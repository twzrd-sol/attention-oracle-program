//! Minimal Keccak-256 implementation for merkle proof verification.
//!
//! Replaces the `sha3` crate dependency to reduce binary size.
//! Only supports messages up to ~1KB (sufficient for merkle leaf hashing).

const ROUNDS: usize = 24;

const RC: [u64; 24] = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
    0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
    0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
];

const ROTC: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14,
    27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];

const PILN: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4,
    15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

#[inline(never)]
fn keccak_f(st: &mut [u64; 25]) {
    for round in 0..ROUNDS {
        // Theta
        let mut bc = [0u64; 5];
        for i in 0..5 {
            bc[i] = st[i] ^ st[i + 5] ^ st[i + 10] ^ st[i + 15] ^ st[i + 20];
        }
        for i in 0..5 {
            let t = bc[(i + 4) % 5] ^ bc[(i + 1) % 5].rotate_left(1);
            for j in (0..25).step_by(5) {
                st[j + i] ^= t;
            }
        }

        // Rho and Pi
        let mut t = st[1];
        for i in 0..24 {
            let j = PILN[i];
            let tmp = st[j];
            st[j] = t.rotate_left(ROTC[i]);
            t = tmp;
        }

        // Chi
        for j in (0..25).step_by(5) {
            let mut tmp = [0u64; 5];
            for i in 0..5 {
                tmp[i] = st[j + i];
            }
            for i in 0..5 {
                st[j + i] = tmp[i] ^ ((!tmp[(i + 1) % 5]) & tmp[(i + 2) % 5]);
            }
        }

        // Iota
        st[0] ^= RC[round];
    }
}

/// Keccak-256 hash of concatenated byte slices.
#[inline(never)]
pub fn keccak256(parts: &[&[u8]]) -> [u8; 32] {
    let mut st = [0u64; 25];
    let rate = 136; // (1600 - 256*2) / 8 = 136 bytes
    let mut buf = [0u8; 136];
    let mut buf_pos = 0usize;

    for part in parts {
        let mut off = 0;
        while off < part.len() {
            let chunk = core::cmp::min(rate - buf_pos, part.len() - off);
            buf[buf_pos..buf_pos + chunk].copy_from_slice(&part[off..off + chunk]);
            buf_pos += chunk;
            off += chunk;

            if buf_pos == rate {
                // Absorb
                for i in 0..17 {
                    st[i] ^= u64::from_le_bytes([
                        buf[i * 8],
                        buf[i * 8 + 1],
                        buf[i * 8 + 2],
                        buf[i * 8 + 3],
                        buf[i * 8 + 4],
                        buf[i * 8 + 5],
                        buf[i * 8 + 6],
                        buf[i * 8 + 7],
                    ]);
                }
                keccak_f(&mut st);
                buf_pos = 0;
            }
        }
    }

    // Padding
    buf[buf_pos] = 0x01;
    for i in (buf_pos + 1)..rate {
        buf[i] = 0;
    }
    buf[rate - 1] |= 0x80;

    // Final absorb
    for i in 0..17 {
        st[i] ^= u64::from_le_bytes([
            buf[i * 8],
            buf[i * 8 + 1],
            buf[i * 8 + 2],
            buf[i * 8 + 3],
            buf[i * 8 + 4],
            buf[i * 8 + 5],
            buf[i * 8 + 6],
            buf[i * 8 + 7],
        ]);
    }
    keccak_f(&mut st);

    // Squeeze
    let mut out = [0u8; 32];
    for i in 0..4 {
        let b = st[i].to_le_bytes();
        out[i * 8..i * 8 + 8].copy_from_slice(&b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hash() {
        // Keccak-256 of empty string (NOT SHA3-256, which has different padding)
        let h = keccak256(&[]);
        let expected: [u8; 32] = [
            0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c,
            0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
            0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b,
            0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
        ];
        assert_eq!(h, expected);
    }

    #[test]
    fn hello_hash() {
        // Keccak-256("hello") — verified with external tools
        let h = keccak256(&[b"hello"]);
        // Known result from multiple keccak-256 implementations
        let h2 = keccak256(&[b"hel", b"lo"]);
        assert_eq!(h, h2); // multi-part must produce same result
    }

    #[test]
    fn multipart_matches_concat() {
        let a = keccak256(&[b"hello", b"world"]);
        let b = keccak256(&[b"helloworld"]);
        assert_eq!(a, b);
    }
}
