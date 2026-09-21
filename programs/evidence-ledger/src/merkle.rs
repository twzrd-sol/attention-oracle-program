//! Sorted-pair keccak merkle inclusion, byte-for-byte the rule in
//! `crates/merkle` (`verify_proof`, `MerkleTree::new`): at each level the
//! smaller 32-byte hash goes first, and an odd trailing node is paired with
//! itself. Off-chain publishers build roots with that crate; this is the only
//! verifier the chain trusts, so the two must never drift.

use crate::keccak::keccak256;

/// Longest proof accepted (2^32 leaves). Mirrors the off-chain bound.
pub const MAX_PROOF_LEN: usize = 32;

#[inline(never)]
pub fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    if a <= b {
        keccak256(&[a, b])
    } else {
        keccak256(&[b, a])
    }
}

/// `proof` is the concatenation of sibling hashes from leaf to root.
/// Returns false on any malformed input; never panics.
#[inline(never)]
pub fn verify_proof(proof: &[u8], leaf: &[u8; 32], root: &[u8; 32]) -> bool {
    if proof.len() % 32 != 0 || proof.len() / 32 > MAX_PROOF_LEN {
        return false;
    }
    let mut hash = *leaf;
    let mut off = 0;
    while off < proof.len() {
        let mut sib = [0u8; 32];
        sib.copy_from_slice(&proof[off..off + 32]);
        hash = hash_pair(&hash, &sib);
        off += 32;
    }
    hash == *root
}

/// RFC 9162 (Certificate Transparency) inclusion over KECCAK-256, the exact
/// rule in `twzrd_agent_intel.log_rfc6962`: `LeafHash = keccak(0x00 || e)`,
/// `NodeHash = keccak(0x01 || l || r)`, audit path walked by (index, tree_size).
/// `path` is the concatenation of 32-byte nodes leaf-to-root.
#[inline(never)]
pub fn verify_inclusion_rfc9162(
    entry: &[u8; 32],
    index: u64,
    tree_size: u64,
    path: &[u8],
    root: &[u8; 32],
) -> bool {
    if tree_size < 1 || index >= tree_size {
        return false;
    }
    if path.len() % 32 != 0 || path.len() / 32 > MAX_PROOF_LEN {
        return false;
    }
    let (mut fnode, mut snode) = (index, tree_size - 1);
    let mut r = keccak256(&[&[0x00u8], entry]);
    let mut off = 0;
    while off < path.len() {
        if snode == 0 {
            return false;
        }
        let mut p = [0u8; 32];
        p.copy_from_slice(&path[off..off + 32]);
        if fnode % 2 == 1 || fnode == snode {
            r = keccak256(&[&[0x01u8], &p, &r]);
            while fnode % 2 == 0 && fnode != 0 {
                fnode /= 2;
                snode /= 2;
            }
        } else {
            r = keccak256(&[&[0x01u8], &r, &p]);
        }
        fnode /= 2;
        snode /= 2;
        off += 32;
    }
    snode == 0 && r == *root
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha3::{Digest, Keccak256};

    fn k(parts: &[&[u8]]) -> [u8; 32] {
        let mut h = Keccak256::new();
        for p in parts {
            h.update(p);
        }
        h.finalize().into()
    }

    /// Reference tree, transcribed from crates/merkle::MerkleTree::new.
    fn build(leaves: &[[u8; 32]]) -> (Vec<Vec<[u8; 32]>>, [u8; 32]) {
        let mut layers = vec![leaves.to_vec()];
        while layers.last().unwrap().len() > 1 {
            let cur = layers.last().unwrap();
            let mut next = Vec::new();
            let mut i = 0;
            while i < cur.len() {
                let a = cur[i];
                let b = if i + 1 < cur.len() { cur[i + 1] } else { cur[i] };
                let (l, r) = if a <= b { (a, b) } else { (b, a) };
                next.push(k(&[&l, &r]));
                i += 2;
            }
            layers.push(next);
        }
        let root = layers.last().unwrap()[0];
        (layers, root)
    }

    fn proof_for(layers: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for layer in &layers[..layers.len() - 1] {
            let sib = if idx % 2 == 0 {
                if idx + 1 < layer.len() { layer[idx + 1] } else { layer[idx] }
            } else {
                layer[idx - 1]
            };
            out.extend_from_slice(&sib);
            idx /= 2;
        }
        out
    }

    fn leaves(n: usize) -> Vec<[u8; 32]> {
        (0..n).map(|i| k(&[b"leaf", &(i as u64).to_le_bytes()])).collect()
    }

    #[test]
    fn hash_pair_is_sorted_and_matches_sha3() {
        let a = k(&[b"a"]);
        let b = k(&[b"b"]);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        assert_eq!(hash_pair(&a, &b), k(&[&lo, &hi]));
        assert_eq!(hash_pair(&a, &b), hash_pair(&b, &a));
    }

    #[test]
    fn every_leaf_verifies_including_odd_trees() {
        for n in [1usize, 2, 3, 5, 8, 13] {
            let lv = leaves(n);
            let (layers, root) = build(&lv);
            for (i, leaf) in lv.iter().enumerate() {
                assert!(verify_proof(&proof_for(&layers, i), leaf, &root), "n={n} i={i}");
            }
        }
    }

    #[test]
    fn single_leaf_root_is_the_leaf() {
        let lv = leaves(1);
        assert!(verify_proof(&[], &lv[0], &lv[0]));
    }

    #[test]
    fn rejects_tampered_leaf_root_and_malformed_proof() {
        let lv = leaves(5);
        let (layers, root) = build(&lv);
        let p = proof_for(&layers, 2);
        let mut bad_leaf = lv[2];
        bad_leaf[0] ^= 1;
        assert!(!verify_proof(&p, &bad_leaf, &root));
        let mut bad_root = root;
        bad_root[31] ^= 1;
        assert!(!verify_proof(&p, &lv[2], &bad_root));
        assert!(!verify_proof(&p[..p.len() - 1], &lv[2], &root)); // not a multiple of 32
        assert!(!verify_proof(&vec![0u8; 32 * (MAX_PROOF_LEN + 1)], &lv[2], &root));
    }

    // ---- RFC 9162 vectors generated by twzrd_agent_intel.log_rfc6962 (law) ----
    const RFC_ENTRIES: [&str; 7] = ["bfa6e9db6e3028aee847c101d694a6650d3228bb0a15d61a314d15519df84092", "c9c234e81d7adaeaf06a14d8491939615fc6a2d274f22dbef59a9936963f8042", "179abcf64241a8a8a4fab33e6fc1b41d1316dac6d1f2968e8d5be2e1b8ab0cb4", "138397b786bdf628b32490b0deebb6d64e36769a3bc07debae3c645d5d4ec684", "1a486827057fa9510c14609c7f0a15496727381676f07bd240d6b13b6045f149", "292a198e4d28ddc57ea0d5aa061b31d6cdf44f818235bb5f5bce60bae151d961", "c7ec410c7dfda5df684546d86b9c460ff652088699b9220bdf855c4fde4e27bc"];
    const RFC_ROOT: &str = "c038f111c37786a5a278066e2ee900615e130751b92fbdf856fd0eeb00907063";
    const RFC_PROOF_0: &[&str] = &["85c6efa74a3f8df92067bee4dc044583028702e36c56c0b922a0878989feae90", "79c05a6bd16b979b34a22af095395d17bbcce9af6ef1175773fc8d806f29cfa7", "389bcd7937d4198b25379234e06f75e1a7e31c4439d029b53966717a5d6eb742"];
    const RFC_PROOF_3: &[&str] = &["4ba08d6d42a1d406a150952b2db948908c86dbafb486805f46302e1d05f997e0", "6595bde26347325f385f51b0a5beacc80ab9f248cfa59c6a2fb76a66da3de6ce", "389bcd7937d4198b25379234e06f75e1a7e31c4439d029b53966717a5d6eb742"];
    const RFC_PROOF_6: &[&str] = &["ade14d197e79dd19c4d533bf3935388da3f7ebd13691cd956a1403629d8e2be3", "da4611d74d0d361299318656555fff41c556fb801f1e8184555032c3d4534559"];
    const RFC_SINGLE_ROOT: &str = "0464adcacce1e8c142e5f5f2ab4d099780344a53eaa89a3c888e2f6cdaaa784e";

    fn hx(s: &str) -> [u8; 32] {
        let mut o = [0u8; 32];
        for i in 0..32 {
            o[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        }
        o
    }
    fn path(p: &[&str]) -> Vec<u8> {
        p.iter().flat_map(|s| hx(s)).collect()
    }

    #[test]
    fn rfc9162_matches_python_reference_vectors() {
        let root = hx(RFC_ROOT);
        assert!(verify_inclusion_rfc9162(&hx(RFC_ENTRIES[0]), 0, 7, &path(RFC_PROOF_0), &root));
        assert!(verify_inclusion_rfc9162(&hx(RFC_ENTRIES[3]), 3, 7, &path(RFC_PROOF_3), &root));
        assert!(verify_inclusion_rfc9162(&hx(RFC_ENTRIES[6]), 6, 7, &path(RFC_PROOF_6), &root));
        // single-entry tree: root == LeafHash(entry), empty path
        assert!(verify_inclusion_rfc9162(&hx(RFC_ENTRIES[0]), 0, 1, &[], &hx(RFC_SINGLE_ROOT)));
    }

    #[test]
    fn rfc9162_rejects_wrong_index_size_and_tamper() {
        let root = hx(RFC_ROOT);
        let e3 = hx(RFC_ENTRIES[3]);
        let p3 = path(RFC_PROOF_3);
        assert!(!verify_inclusion_rfc9162(&e3, 2, 7, &p3, &root)); // wrong index
        // Not a rejection: for index 3 the audit-path walk has the same shape at
        // sizes 5..=8, so the 7-tree proof verifies at size 8 too. Byte-faithful
        // to the Python reference; tree_size is bound by the signed STH, not the
        // root. Index 6 IS size-sensitive (only 7 verifies), pinned below.
        assert!(verify_inclusion_rfc9162(&e3, 3, 8, &p3, &root));
        let (e6, p6) = (hx(RFC_ENTRIES[6]), path(RFC_PROOF_6));
        assert!(!verify_inclusion_rfc9162(&e6, 6, 8, &p6, &root)); // wrong tree_size
        assert!(!verify_inclusion_rfc9162(&e3, 7, 7, &p3, &root)); // index >= size
        assert!(!verify_inclusion_rfc9162(&e3, 3, 0, &p3, &root)); // empty tree
        assert!(!verify_inclusion_rfc9162(&hx(RFC_ENTRIES[0]), 3, 7, &p3, &root)); // wrong entry
        assert!(!verify_inclusion_rfc9162(&e3, 3, 7, &p3[..p3.len() - 1], &root)); // malformed
        let mut bad = root;
        bad[0] ^= 1;
        assert!(!verify_inclusion_rfc9162(&e3, 3, 7, &p3, &bad));
    }
}
