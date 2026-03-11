use anchor_lang::prelude::Pubkey;
use crate::constants::{CUMULATIVE_V2_DOMAIN, CUMULATIVE_V3_DOMAIN, GLOBAL_V4_DOMAIN, GLOBAL_V5_DOMAIN};
use sha3::{Digest, Keccak256};

pub fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for p in parts {
        hasher.update(p);
    }
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

pub fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
    // SECURITY: Bounds check to prevent DoS via unbounded proof length
    // Maximum proof depth: 32 levels (2^32 leaves, ~4 billion - more than enough)
    if proof.len() > 32 {
        return false;
    }

    for sibling in proof.iter() {
        let (a, b) = if hash <= *sibling {
            (hash, *sibling)
        } else {
            (*sibling, hash)
        };
        // Pairwise keccak256 over sorted siblings.
        hash = keccak_hashv(&[&a, &b]);
    }
    hash == root
}

/// Computes the cumulative (v2) leaf hash:
/// keccak(domain || channel_cfg || mint || root_seq || wallet || cumulative_total)
pub fn compute_cumulative_leaf(
    channel_config: &Pubkey,
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    let seq = root_seq.to_le_bytes();
    let total = cumulative_total.to_le_bytes();
    keccak_hashv(&[
        CUMULATIVE_V2_DOMAIN,
        channel_config.as_ref(),
        mint.as_ref(),
        &seq,
        wallet.as_ref(),
        &total,
    ])
}

/// Computes the cumulative (v3) leaf hash with stake snapshot binding:
/// keccak(domain || channel_cfg || mint || root_seq || wallet || cumulative_total || stake_snapshot || snapshot_slot)
///
/// V3 adds stake_snapshot and snapshot_slot to prevent "boost gaming" where users:
/// 1. Stake tokens to boost rewards at snapshot time
/// 2. Unstake before claim
/// 3. Claim with boosted proof despite no longer having stake
///
/// This binds the proof to:
/// - The user's stake at snapshot time (prevents unstaking after proof)
/// - The specific slot when stakes were read (enables proof expiry)
///
/// The on-chain claim instruction verifies: user_stake.amount >= stake_snapshot
pub fn compute_cumulative_leaf_v3(
    channel_config: &Pubkey,
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
    stake_snapshot: u64,
    snapshot_slot: u64,
) -> [u8; 32] {
    let seq = root_seq.to_le_bytes();
    let total = cumulative_total.to_le_bytes();
    let stake = stake_snapshot.to_le_bytes();
    let slot = snapshot_slot.to_le_bytes();
    keccak_hashv(&[
        CUMULATIVE_V3_DOMAIN,
        channel_config.as_ref(),
        mint.as_ref(),
        &seq,
        wallet.as_ref(),
        &total,
        &stake,
        &slot,
    ])
}

/// Computes the simple global (v4) leaf hash — per-user totals, no channel scope:
/// keccak(domain || mint || root_seq || wallet || cumulative_total)
///
/// One global root serves all users. No stake snapshot or channel binding.
/// Creator fee split is handled off-chain by the publisher.
pub fn compute_global_leaf(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    let seq = root_seq.to_le_bytes();
    let total = cumulative_total.to_le_bytes();
    keccak_hashv(&[
        GLOBAL_V4_DOMAIN,
        mint.as_ref(),
        &seq,
        wallet.as_ref(),
        &total,
    ])
}

/// Computes the global (v5) leaf hash with yield breakdown:
/// keccak(domain || mint || root_seq || wallet || cumulative_total || base_yield || attention_bonus)
///
/// V5 extends V4 by splitting the cumulative total into two auditable components:
/// - `base_yield`: reward from vault/strategy APR (deposit × rate × time)
/// - `attention_bonus`: reward from attention scoring/multiplier
///
/// The constraint `base_yield + attention_bonus == cumulative_total` is enforced by the publisher.
/// On-chain verification only checks merkle proof validity; the split is informational
/// for off-chain auditability and UI display.
pub fn compute_global_leaf_v5(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
    base_yield: u64,
    attention_bonus: u64,
) -> [u8; 32] {
    let seq = root_seq.to_le_bytes();
    let total = cumulative_total.to_le_bytes();
    let base = base_yield.to_le_bytes();
    let bonus = attention_bonus.to_le_bytes();
    keccak_hashv(&[
        GLOBAL_V5_DOMAIN,
        mint.as_ref(),
        &seq,
        wallet.as_ref(),
        &total,
        &base,
        &bonus,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_global_leaf_v5_deterministic() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let leaf1 = compute_global_leaf_v5(&mint, 1, &wallet, 10_000, 6_000, 4_000);
        let leaf2 = compute_global_leaf_v5(&mint, 1, &wallet, 10_000, 6_000, 4_000);
        assert_eq!(leaf1, leaf2, "Same inputs must produce same leaf");
    }

    #[test]
    fn compute_global_leaf_v5_all_fields_bound() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let baseline = compute_global_leaf_v5(&mint, 1, &wallet, 10_000, 6_000, 4_000);

        let diff_mint = compute_global_leaf_v5(&Pubkey::new_unique(), 1, &wallet, 10_000, 6_000, 4_000);
        let diff_seq = compute_global_leaf_v5(&mint, 2, &wallet, 10_000, 6_000, 4_000);
        let diff_wallet = compute_global_leaf_v5(&mint, 1, &Pubkey::new_unique(), 10_000, 6_000, 4_000);
        let diff_total = compute_global_leaf_v5(&mint, 1, &wallet, 10_001, 6_001, 4_000);
        let diff_base = compute_global_leaf_v5(&mint, 1, &wallet, 10_000, 7_000, 3_000);
        let diff_bonus = compute_global_leaf_v5(&mint, 1, &wallet, 10_000, 5_000, 5_000);

        assert_ne!(baseline, diff_mint, "Different mint must change leaf");
        assert_ne!(baseline, diff_seq, "Different root_seq must change leaf");
        assert_ne!(baseline, diff_wallet, "Different wallet must change leaf");
        assert_ne!(baseline, diff_total, "Different cumulative_total must change leaf");
        assert_ne!(baseline, diff_base, "Different base_yield must change leaf");
        assert_ne!(baseline, diff_bonus, "Different attention_bonus must change leaf");
    }

    #[test]
    fn compute_global_leaf_v5_domain_separation_from_v4() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let total = 10_000_000_000u64;
        let root_seq = 1u64;

        let v4_leaf = compute_global_leaf(&mint, root_seq, &wallet, total);
        let v5_leaf = compute_global_leaf_v5(&mint, root_seq, &wallet, total, total, 0);

        assert_ne!(
            v4_leaf, v5_leaf,
            "V4 and V5 leaves must differ even with same cumulative_total (domain separation)"
        );
    }

    #[test]
    fn compute_global_leaf_v5_manual_keccak_match() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let root_seq = 42u64;
        let total = 123_456_789u64;
        let base = 100_000_000u64;
        let bonus = 23_456_789u64;

        let leaf_from_fn = compute_global_leaf_v5(&mint, root_seq, &wallet, total, base, bonus);

        let mut hasher = Keccak256::new();
        hasher.update(GLOBAL_V5_DOMAIN);
        hasher.update(mint.as_ref());
        hasher.update(&root_seq.to_le_bytes());
        hasher.update(wallet.as_ref());
        hasher.update(&total.to_le_bytes());
        hasher.update(&base.to_le_bytes());
        hasher.update(&bonus.to_le_bytes());
        let expected: [u8; 32] = hasher.finalize().into();

        assert_eq!(leaf_from_fn, expected, "V5 leaf computation must match raw keccak");
    }
}
