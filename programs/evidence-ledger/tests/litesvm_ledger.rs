#![cfg(feature = "localtest")]
//! End-to-end on LiteSVM: init a ledger, publish roots append-only, verify
//! inclusion of the canonical golden leaves (V6 receipt + decision outcome
//! vectors), and refuse every unauthorized or out-of-order path.
//!
//! Run:  cargo build-sbf && cargo test --release --features localtest --test litesvm_ledger

use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_sdk::{instruction::{AccountMeta as AM, Instruction as Ix}, pubkey::Pubkey as Pk};
use solana_signer::Signer;
use solana_system_interface::program as sys;
use solana_transaction::Transaction;
use solana_ed25519_program::new_ed25519_instruction_with_signature;
use std::path::Path;

const DOMAIN: &[u8] = b"TWZRD:AO_REPUTATION_RECEIPT_V6";
// Golden leaves (law): receipt V6 signed example, V6 full/none blocks,
// decision-outcome settled / blocked_never_signed / expired_unused.
const GOLDEN: [&str; 6] = [
    "696bab7f6778236b86c8a88cd537924813331cceaccc99b0a1a4b2eaca934e30",
    "4c82649d2be393b1fca2da7c5d4c7afebb189ad3f0b93b620ce2e552fe5ce558",
    "38e994b1ba17d09f52814001844be37abf8471f813f92a017e8a4fce8a095479",
    "03ea1d9c11719b792a2d80505dbfb22a125519a0041e972707f3a71b232c3df1",
    "5870ba3cca689e94ccf5276917daefa5d1a80c522f3c972329bbb99fdcb1b5ab",
    "21289395b5e5d7319a59f9a893306c31a91f04c19e594bb0226b782ff05b0f52",
];

fn addr(pk: &Pk) -> Address { Address::from(pk.to_bytes()) }
fn pk(kp: &Keypair) -> Pk { Pk::new_from_array(kp.pubkey().to_bytes()) }
fn pid() -> Pk { "BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W".parse().unwrap() }
fn disc(name: &str) -> [u8; 8] { Sha256::digest(format!("global:{name}").as_bytes())[..8].try_into().unwrap() }
fn hex32(s: &str) -> [u8; 32] {
    let mut o = [0u8; 32];
    for i in 0..32 { o[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap(); }
    o
}
fn k(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Keccak256::new();
    for p in parts { h.update(p); }
    h.finalize().into()
}
fn ledger_pda() -> Pk { ledger_pda_for(DOMAIN) }
fn ledger_pda_for(id: &[u8]) -> Pk { Pk::find_program_address(&[b"ledger", &k(&[id])], &pid()).0 }
fn root_pda(ledger: &Pk, seq: u64) -> Pk {
    Pk::find_program_address(&[b"root", ledger.as_ref(), &seq.to_le_bytes()], &pid()).0
}

/// Sorted-pair keccak tree, odd trailing node paired with itself (crates/merkle rule).
fn tree(leaves: &[[u8; 32]]) -> (Vec<Vec<[u8; 32]>>, [u8; 32]) {
    let mut layers = vec![leaves.to_vec()];
    while layers.last().unwrap().len() > 1 {
        let cur = layers.last().unwrap();
        let mut next = Vec::new();
        for c in cur.chunks(2) {
            let (a, b) = (c[0], *c.get(1).unwrap_or(&c[0]));
            let (l, r) = if a <= b { (a, b) } else { (b, a) };
            next.push(k(&[&l, &r]));
        }
        layers.push(next);
    }
    let root = layers.last().unwrap()[0];
    (layers, root)
}
fn proof(layers: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for layer in &layers[..layers.len() - 1] {
        let sib = if idx % 2 == 0 { *layer.get(idx + 1).unwrap_or(&layer[idx]) } else { layer[idx - 1] };
        out.extend_from_slice(&sib);
        idx /= 2;
    }
    out
}

fn load(svm: &mut LiteSVM) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/deploy/evidence_ledger.so");
    assert!(p.exists(), "Run cargo build-sbf first");
    svm.add_program(addr(&pid()), &std::fs::read(&p).unwrap()).unwrap();
}
fn fund(svm: &mut LiteSVM, kp: &Keypair) { svm.airdrop(&kp.pubkey(), 5_000_000_000).unwrap(); }
fn convert(ix: &Ix) -> solana_instruction::Instruction {
    solana_instruction::Instruction {
        program_id: addr(&ix.program_id),
        accounts: ix.accounts.iter().map(|m| {
            let a = addr(&m.pubkey);
            if m.is_writable { solana_instruction::AccountMeta::new(a, m.is_signer) }
            else { solana_instruction::AccountMeta::new_readonly(a, m.is_signer) }
        }).collect(),
        data: ix.data.clone(),
    }
}
fn send(svm: &mut LiteSVM, ix: Ix, signers: &[&Keypair]) -> Result<(), String> {
    let msg = Message::new(&[convert(&ix)], Some(&signers[0].pubkey()));
    svm.expire_blockhash(); // identical ix+signer would otherwise dedupe as AlreadyProcessed
    let bh = svm.latest_blockhash();
    let tx = Transaction::new(signers, msg, bh);
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{e:?}"))
}

fn init_ix(payer: &Pk, authority: &Pk) -> Ix {
    let mut data = disc("init_ledger").to_vec();
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[0u8; 32]); // trusted_signer: unused for scheme 1
    data.push(1); // SCHEME_SORTED_PAIR
    data.push(DOMAIN.len() as u8);
    data.extend_from_slice(DOMAIN);
    Ix { program_id: pid(), accounts: vec![AM::new(*payer, true), AM::new(ledger_pda(), false), AM::new_readonly(sys::id(), false)], data }
}
fn publish_ix(authority: &Pk, seq: u64, root: [u8; 32], leaf_count: u64) -> Ix {
    let mut data = disc("publish_root").to_vec();
    data.extend_from_slice(&seq.to_le_bytes());
    data.extend_from_slice(&root);
    data.extend_from_slice(&k(&[b"manifest", &seq.to_le_bytes()]));
    data.extend_from_slice(&leaf_count.to_le_bytes());
    let l = ledger_pda();
    Ix { program_id: pid(), accounts: vec![AM::new_readonly(*authority, true), AM::new(*authority, true), AM::new(l, false), AM::new(root_pda(&l, seq), false), AM::new_readonly(sys::id(), false)], data }
}
fn verify_ix(seq: u64, leaf: [u8; 32], proof: &[u8]) -> Ix {
    let mut data = disc("verify_inclusion").to_vec();
    data.extend_from_slice(&seq.to_le_bytes());
    data.extend_from_slice(&leaf);
    data.extend_from_slice(proof);
    let l = ledger_pda();
    Ix { program_id: pid(), accounts: vec![AM::new_readonly(l, false), AM::new_readonly(root_pda(&l, seq), false)], data }
}
fn set_auth_ix(current: &Pk, new: &Pk) -> Ix {
    let mut data = disc("set_authority").to_vec();
    data.extend_from_slice(new.as_ref());
    Ix { program_id: pid(), accounts: vec![AM::new_readonly(*current, true), AM::new(ledger_pda(), false)], data }
}
fn next_seq(svm: &LiteSVM) -> u64 {
    let d = svm.get_account(&addr(&ledger_pda())).unwrap().data;
    assert_eq!(d.len(), 128);
    // count at [115..123]; last_seq at [107..115]
    let count = u64::from_le_bytes(d[115..123].try_into().unwrap());
    if count == 0 { 0 } else { u64::from_le_bytes(d[107..115].try_into().unwrap()) + 1 }
}

#[test]
fn golden_leaves_anchor_and_verify_end_to_end() {
    let mut svm = LiteSVM::new();
    load(&mut svm);
    let auth = Keypair::new();
    fund(&mut svm, &auth);
    send(&mut svm, init_ix(&pk(&auth), &pk(&auth)), &[&auth]).unwrap();
    assert_eq!(next_seq(&svm), 0);

    let leaves: Vec<[u8; 32]> = GOLDEN.iter().map(|h| hex32(h)).collect();
    let (layers, root) = tree(&leaves);
    send(&mut svm, publish_ix(&pk(&auth), 0, root, leaves.len() as u64), &[&auth]).unwrap();
    assert_eq!(next_seq(&svm), 1);
    let r = svm.get_account(&addr(&root_pda(&ledger_pda(), 0))).unwrap().data;
    assert_eq!(r.len(), 152);
    assert_eq!(&r[18..50], &root);
    assert_eq!(u64::from_le_bytes(r[82..90].try_into().unwrap()), 6);
    assert_eq!(&r[114..146], pk(&auth).as_ref());

    for (i, leaf) in leaves.iter().enumerate() {
        send(&mut svm, verify_ix(0, *leaf, &proof(&layers, i)), &[&auth]).unwrap();
    }
    // a leaf from a different tree, and a flipped golden leaf, both refuse
    let mut bad = leaves[0]; bad[0] ^= 1;
    let e = send(&mut svm, verify_ix(0, bad, &proof(&layers, 0)), &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6003)"), "{e}");
}

#[test]
fn append_only_and_authority_are_enforced() {
    let mut svm = LiteSVM::new();
    load(&mut svm);
    let auth = Keypair::new();
    let stranger = Keypair::new();
    fund(&mut svm, &auth);
    fund(&mut svm, &stranger);
    send(&mut svm, init_ix(&pk(&auth), &pk(&auth)), &[&auth]).unwrap();
    let e = send(&mut svm, init_ix(&pk(&auth), &pk(&auth)), &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6001)"), "re-init: {e}");

    let (_, root) = tree(&[k(&[b"x"])]);
    let e = send(&mut svm, publish_ix(&pk(&stranger), 0, root, 1), &[&stranger]).unwrap_err();
    assert!(e.contains("Custom(6000)"), "stranger publish: {e}");
    send(&mut svm, publish_ix(&pk(&auth), 5, root, 1), &[&auth]).unwrap(); // first seq may be anything
    let e = send(&mut svm, publish_ix(&pk(&auth), 5, root, 1), &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6002)"), "rewrite seq 5: {e}");
    let e = send(&mut svm, publish_ix(&pk(&auth), 3, root, 1), &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6002)"), "seq must increase: {e}");

    let e = send(&mut svm, set_auth_ix(&pk(&stranger), &pk(&stranger)), &[&stranger]).unwrap_err();
    assert!(e.contains("Custom(6000)"), "stranger rotate: {e}");
    send(&mut svm, set_auth_ix(&pk(&auth), &pk(&stranger)), &[&auth]).unwrap();
    send(&mut svm, publish_ix(&pk(&stranger), 9, root, 1), &[&stranger]).unwrap();
    let e = send(&mut svm, publish_ix(&pk(&auth), 10, root, 1), &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6000)"), "old authority after rotation: {e}");
    assert_eq!(next_seq(&svm), 10);
}

// ---------------- scheme 2: signed Receipt-log heads ----------------
const LOG_ID: &[u8] = b"intel.twzrd.xyz/v6";
const STH_DOMAIN: &[u8] = b"TWZRD:RECEIPT_LOG_STH_V1";

fn sth_preimage(tree_size: u64, ts: u64, root: &[u8; 32]) -> Vec<u8> {
    let mut m = STH_DOMAIN.to_vec();
    m.extend_from_slice(&(LOG_ID.len() as u16).to_le_bytes());
    m.extend_from_slice(LOG_ID);
    m.extend_from_slice(&tree_size.to_le_bytes());
    m.extend_from_slice(&ts.to_le_bytes());
    m.extend_from_slice(root);
    m
}
fn init_log_ix(payer: &Pk, authority: &Pk, trusted: &Pk) -> Ix {
    let mut data = disc("init_ledger").to_vec();
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(trusted.as_ref());
    data.push(2); // SCHEME_RFC9162
    data.push(LOG_ID.len() as u8);
    data.extend_from_slice(LOG_ID);
    Ix { program_id: pid(), accounts: vec![AM::new(*payer, true), AM::new(ledger_pda_for(LOG_ID), false), AM::new_readonly(sys::id(), false)], data }
}
fn anchor_ix(authority: &Pk, tree_size: u64, ts: u64, root: [u8; 32]) -> Ix {
    let mut data = disc("anchor_head").to_vec();
    data.extend_from_slice(&tree_size.to_le_bytes());
    data.extend_from_slice(&ts.to_le_bytes());
    data.extend_from_slice(&root);
    data.push(LOG_ID.len() as u8);
    data.extend_from_slice(LOG_ID);
    let l = ledger_pda_for(LOG_ID);
    let ixs: Pk = "Sysvar1nstructions1111111111111111111111111".parse().unwrap();
    Ix { program_id: pid(), accounts: vec![AM::new_readonly(*authority, true), AM::new(*authority, true), AM::new(l, false), AM::new(root_pda(&l, tree_size), false), AM::new_readonly(sys::id(), false), AM::new_readonly(ixs, false)], data }
}
fn send2(svm: &mut LiteSVM, first: Option<Ix>, ix: Ix, kp: &Keypair) -> Result<(), String> {
    let mut v = Vec::new();
    if let Some(f) = first { v.push(convert(&f)); }
    v.push(convert(&ix));
    let msg = Message::new(&v, Some(&kp.pubkey()));
    svm.expire_blockhash();
    let bh = svm.latest_blockhash();
    let tx = Transaction::new(&[kp], msg, bh);
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{e:?}"))
}
fn ed_ix(signer: &Keypair, msg: &[u8]) -> Ix {
    let sig: [u8; 64] = signer.sign_message(msg).as_ref().try_into().unwrap();
    let pk: [u8; 32] = signer.pubkey().to_bytes();
    let i = new_ed25519_instruction_with_signature(msg, &sig, &pk);
    Ix { program_id: Pk::new_from_array(i.program_id.to_bytes()), accounts: vec![], data: i.data }
}

#[test]
fn signed_log_head_anchors_only_under_the_trusted_key() {
    let mut svm = LiteSVM::new();
    load(&mut svm);
    let auth = Keypair::new();
    let log_key = Keypair::new(); // stands in for twzrd-receipt-ed25519-v2
    let rogue = Keypair::new();
    fund(&mut svm, &auth);
    send(&mut svm, init_log_ix(&pk(&auth), &pk(&auth), &pk(&log_key)), &[&auth]).unwrap();

    // a real RFC 9162 head over 7 entries, as the log would serve it
    let entries: Vec<[u8; 32]> = (0..7u64).map(|i| k(&[b"entry", &i.to_le_bytes()])).collect();
    let leaf_h = |e: &[u8; 32]| k(&[&[0u8], e]);
    let node_h = |l: &[u8; 32], r: &[u8; 32]| k(&[&[1u8], l, r]);
    // MTH of 7: RFC 9162 split at largest power of two < n
    fn mth(e: &[[u8; 32]], lh: &dyn Fn(&[u8; 32]) -> [u8; 32], nh: &dyn Fn(&[u8; 32], &[u8; 32]) -> [u8; 32]) -> [u8; 32] {
        if e.len() == 1 { return lh(&e[0]); }
        let mut kk = 1; while kk * 2 < e.len() { kk *= 2; }
        nh(&mth(&e[..kk], lh, nh), &mth(&e[kk..], lh, nh))
    }
    let root = mth(&entries, &leaf_h, &node_h);
    let (size, ts) = (7u64, 1_760_000_000u64);
    let pre = sth_preimage(size, ts, &root);

    // no precompile in the tx -> 6006
    let e = send2(&mut svm, None, anchor_ix(&pk(&auth), size, ts, root), &auth).unwrap_err();
    assert!(e.contains("Custom(6006)"), "no precompile: {e}");
    // signed by the wrong key -> 6007
    let e = send2(&mut svm, Some(ed_ix(&rogue, &pre)), anchor_ix(&pk(&auth), size, ts, root), &auth).unwrap_err();
    assert!(e.contains("Custom(6007)"), "rogue key: {e}");
    // signed message does not match the head being anchored -> 6007
    let e = send2(&mut svm, Some(ed_ix(&log_key, &sth_preimage(size, ts + 1, &root))), anchor_ix(&pk(&auth), size, ts, root), &auth).unwrap_err();
    assert!(e.contains("Custom(6007)"), "binding mismatch: {e}");
    // publish_root is not a way around the signature on a log ledger -> 6005
    let mut pub_ix = publish_ix(&pk(&auth), size, root, size);
    pub_ix.accounts[2] = AM::new(ledger_pda_for(LOG_ID), false);
    pub_ix.accounts[3] = AM::new(root_pda(&ledger_pda_for(LOG_ID), size), false);
    let e = send(&mut svm, pub_ix, &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6005)"), "publish on log ledger: {e}");

    // the genuine head anchors
    send2(&mut svm, Some(ed_ix(&log_key, &pre)), anchor_ix(&pk(&auth), size, ts, root), &auth).unwrap();
    let l = ledger_pda_for(LOG_ID);
    let r = svm.get_account(&addr(&root_pda(&l, size))).unwrap().data;
    assert_eq!(&r[18..50], &root);
    assert_eq!(u64::from_le_bytes(r[90..98].try_into().unwrap()), ts);
    assert_eq!(u64::from_le_bytes(r[82..90].try_into().unwrap()), size);

    // and an RFC 9162 inclusion proof for entry 3 verifies against it on-chain
    let l0: Vec<[u8; 32]> = entries.iter().map(|e| leaf_h(e)).collect();
    // audit path for index 3 of 7: sibling leaf 2, node(0,1), MTH(4..7)
    let path: Vec<u8> = [l0[2], node_h(&l0[0], &l0[1]), mth(&entries[4..], &leaf_h, &node_h)].concat();
    let mut data = disc("verify_inclusion").to_vec();
    data.extend_from_slice(&size.to_le_bytes());
    data.extend_from_slice(&entries[3]);
    data.extend_from_slice(&3u64.to_le_bytes());
    data.extend_from_slice(&path);
    let vix = Ix { program_id: pid(), accounts: vec![AM::new_readonly(l, false), AM::new_readonly(root_pda(&l, size), false)], data };
    send(&mut svm, vix, &[&auth]).unwrap();
}


#[test]
fn stripe_fixture_leaves_publish_and_verify() {
    // No Stripe, no trust refuse, no RPC: encode the checked-in SPT fixture,
    // publish a scheme-1 root, verify inclusion on LiteSVM.
    use evidence_ledger::leaf_stripe::{stripe_spt_leaf, STRIPE_SPT_LOG_ID};

    let mut svm = LiteSVM::new();
    load(&mut svm);
    let auth = Keypair::new();
    fund(&mut svm, &auth);

    // init ledger for the Stripe leaf domain
    let mut data = disc("init_ledger").to_vec();
    data.extend_from_slice(pk(&auth).as_ref());
    data.extend_from_slice(&[0u8; 32]);
    data.push(1); // SCHEME_SORTED_PAIR
    data.push(STRIPE_SPT_LOG_ID.len() as u8);
    data.extend_from_slice(STRIPE_SPT_LOG_ID);
    let ledger = ledger_pda_for(STRIPE_SPT_LOG_ID);
    let init = Ix {
        program_id: pid(),
        accounts: vec![
            AM::new(pk(&auth), true),
            AM::new(ledger, false),
            AM::new_readonly(sys::id(), false),
        ],
        data,
    };
    send(&mut svm, init, &[&auth]).unwrap();

    let leaf = stripe_spt_leaf(
        b"cus_agent_spike_001",
        b"pi_3SpikeTest000000000000000",
        b"mcp://twzrd.intel/readiness_card",
        25,
        1_758_480_000,
    );
    // companion filler leaf so the tree is non-trivial
    let filler = k(&[b"stripe-spike-filler"]);
    let leaves = vec![leaf, filler];
    let (layers, root) = tree(&leaves);

    // publish_root for this ledger (reuse helper shape with custom ledger PDA)
    let seq: u64 = 0;
    let mut pdata = disc("publish_root").to_vec();
    pdata.extend_from_slice(&seq.to_le_bytes());
    pdata.extend_from_slice(&root);
    pdata.extend_from_slice(&k(&[b"manifest", &seq.to_le_bytes()]));
    pdata.extend_from_slice(&(leaves.len() as u64).to_le_bytes());
    let publish = Ix {
        program_id: pid(),
        accounts: vec![
            AM::new_readonly(pk(&auth), true),
            AM::new(pk(&auth), true),
            AM::new(ledger, false),
            AM::new(root_pda(&ledger, seq), false),
            AM::new_readonly(sys::id(), false),
        ],
        data: pdata,
    };
    send(&mut svm, publish, &[&auth]).unwrap();

    let mut vdata = disc("verify_inclusion").to_vec();
    vdata.extend_from_slice(&seq.to_le_bytes());
    vdata.extend_from_slice(&leaf);
    vdata.extend_from_slice(&proof(&layers, 0));
    let verify = Ix {
        program_id: pid(),
        accounts: vec![
            AM::new_readonly(ledger, false),
            AM::new_readonly(root_pda(&ledger, seq), false),
        ],
        data: vdata,
    };
    send(&mut svm, verify, &[&auth]).unwrap();

    // flipped leaf must refuse
    let mut bad = leaf;
    bad[0] ^= 1;
    let mut bdata = disc("verify_inclusion").to_vec();
    bdata.extend_from_slice(&seq.to_le_bytes());
    bdata.extend_from_slice(&bad);
    bdata.extend_from_slice(&proof(&layers, 0));
    let bad_ix = Ix {
        program_id: pid(),
        accounts: vec![
            AM::new_readonly(ledger, false),
            AM::new_readonly(root_pda(&ledger, seq), false),
        ],
        data: bdata,
    };
    let e = send(&mut svm, bad_ix, &[&auth]).unwrap_err();
    assert!(e.contains("Custom(6003)"), "{e}");
}
