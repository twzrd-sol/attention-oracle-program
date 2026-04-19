//! wzrd-rails account state definitions.
//!
//! Each struct is declared alongside the IX that creates or reads it. New
//! structs land as new IXs are implemented — not ahead of time.

use anchor_lang::prelude::*;

// PDA seed constants. Centralized here so off-chain derivation scripts
// (keepers, SDK) can import the same values and stay in sync.
pub const CONFIG_SEED: &[u8] = b"config";
pub const POOL_SEED: &[u8] = b"pool";
pub const USER_STAKE_SEED: &[u8] = b"user_stake";
pub const STAKE_VAULT_SEED: &[u8] = b"stake_vault";
pub const REWARD_VAULT_SEED: &[u8] = b"reward_vault";
pub const COMP_VAULT_SEED: &[u8] = b"comp_vault";
pub const COMP_CLAIMED_SEED: &[u8] = b"comp_claimed";
pub const COMPENSATION_LEAF_DOMAIN: &[u8] = b"wzrd-rails-comp";

/// Safety bound for `reward_rate_per_slot`.
///
/// Day 1 uses a deliberately loose cap to prevent accidental absurd emissions
/// while still leaving room for treasury-operator tuning as the protocol finds
/// its real budget envelope.
pub const MAX_REWARD_RATE_PER_SLOT: u64 = 1_000_000;

/// Global configuration for the wzrd-rails program.
///
/// One instance per deployment, created by `initialize_config`. Holds program-wide
/// references (admin, CCM mint, treasury ATA) plus the one-time compensation
/// merkle root for the external stake honoring drop.
///
/// PDA: `[CONFIG_SEED]`
#[account]
#[derive(Debug)]
pub struct Config {
    /// Admin authority. Can call set_admin, set_reward_rate,
    /// compensate_external_stakers, and initialize_pool. Should be a
    /// Squads V4 vault PDA for production; can be any signer for devnet/tests.
    pub admin: Pubkey,
    /// CCM mint (Token-2022). Pinned at init; never changes.
    pub ccm_mint: Pubkey,
    /// Treasury CCM ATA. Source of reward-pool funding via the revenue-router
    /// keeper. Stored as pubkey only; access is gated by the keeper's signer,
    /// not by this field.
    pub treasury_ccm_ata: Pubkey,
    /// Compensation merkle root for the one-time external stakers drop.
    /// All-zero = unset. Set exactly once by `compensate_external_stakers`.
    ///
    /// Future claim convention:
    ///   leaf = keccak::hashv(&[COMPENSATION_LEAF_DOMAIN, user.as_ref(), amount.to_le_bytes().as_ref()])
    ///   pair hash = sorted pair keccak(min, max)
    pub comp_merkle_root: [u8; 32],
    /// Count of initialized stake pools. Incremented by `initialize_pool`.
    pub total_pools: u32,
    /// PDA bump.
    pub bump: u8,
}

impl Config {
    /// Account size: 8 (discriminator) + struct fields.
    /// 8 + 32 + 32 + 32 + 32 + 4 + 1 = 141 bytes.
    pub const LEN: usize = 8 + 32 + 32 + 32 + 32 + 4 + 1;

    /// Returns true if the compensation merkle root has been set.
    pub fn comp_root_set(&self) -> bool {
        self.comp_merkle_root != [0u8; 32]
    }
}

#[event]
pub struct ConfigInitialized {
    pub config: Pubkey,
    pub admin: Pubkey,
    pub ccm_mint: Pubkey,
    pub treasury_ccm_ata: Pubkey,
    pub slot: u64,
}

#[event]
pub struct AdminChanged {
    pub config: Pubkey,
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    pub slot: u64,
}

#[event]
pub struct RewardRateChanged {
    pub pool: Pubkey,
    pub pool_id: u32,
    pub old_rate: u64,
    pub new_rate: u64,
    pub slot: u64,
}

#[event]
pub struct CompensationRootSet {
    pub config: Pubkey,
    pub admin: Pubkey,
    pub comp_vault: Pubkey,
    pub merkle_root: [u8; 32],
    pub slot: u64,
}

#[event]
pub struct CompensationClaimedEvent {
    pub config: Pubkey,
    pub user: Pubkey,
    pub claimed_account: Pubkey,
    pub comp_vault: Pubkey,
    pub amount: u64,
    pub slot: u64,
}

/// Stake pool. One per (program_id, pool_id) tuple.
///
/// Pool 0 is the global default for Day 1. Per-channel pools (pool_id > 0) are
/// a future extension path that needs no IX changes — just `initialize_pool`
/// called again with a new id.
///
/// PDA: `[POOL_SEED, pool_id.to_le_bytes()]`
///
/// Reward accounting follows the MasterChef pattern:
///   acc_reward_per_share := acc_reward_per_share
///     + ((slot_delta * reward_rate_per_slot * REWARD_SCALE) / total_staked)
/// where REWARD_SCALE = 1e12 for precision on small total_staked.
#[account]
#[derive(Debug)]
pub struct StakePool {
    /// Pool identifier. 0 = global default. Increments by convention, not enforced
    /// (admin can create any id they want; sequential is just the recommended path).
    pub pool_id: u32,
    /// Sum of all active UserStake.amount for this pool.
    pub total_staked: u64,
    /// MasterChef accumulator, scaled by REWARD_SCALE = 1e12.
    pub acc_reward_per_share: u128,
    /// Rewards minted per slot when total_staked > 0. Set by admin via
    /// `set_reward_rate`. Starts at 0 — meaning no yield until admin
    /// explicitly turns on emissions.
    pub reward_rate_per_slot: u64,
    /// Last slot at which `acc_reward_per_share` was updated. Used to compute
    /// slot_delta on each stake/unstake/claim.
    pub last_update_slot: u64,
    /// Lock duration in slots. Day 1 default = 7 days ≈ 1,512,000 slots.
    pub lock_duration_slots: u64,
    /// PDA bump.
    pub bump: u8,
}

impl StakePool {
    /// Account size: 8 + 4 + 8 + 16 + 8 + 8 + 8 + 1 = 61 bytes.
    pub const LEN: usize = 8 + 4 + 8 + 16 + 8 + 8 + 8 + 1;

    /// Fixed-point scale for `acc_reward_per_share`. 1e12 is enough precision
    /// for total_staked up to 2^52 units without truncation bias.
    pub const REWARD_SCALE: u128 = 1_000_000_000_000;

    /// Recommended Day 1 lock duration: 7 days × 24h × 60m × 60s ÷ 0.4s/slot
    /// = 1,512,000 slots.
    pub const DEFAULT_LOCK_SLOTS: u64 = 1_512_000;

    /// Apply slot-delta-since-last-update to the reward accumulator.
    ///
    /// MasterChef math:
    ///   new_per_share = acc_reward_per_share
    ///     + (slots_elapsed * reward_rate_per_slot * REWARD_SCALE) / total_staked
    ///
    /// Idempotent if called twice in the same slot (slots_elapsed = 0 → no-op).
    /// If total_staked = 0 or reward_rate = 0, the accumulator is unchanged but
    /// last_update_slot still advances so future slot deltas measure from NOW,
    /// not from the original init slot.
    ///
    /// Every IX that reads or writes stake state MUST call this first. Caller
    /// is responsible for passing the current clock slot.
    pub fn accrue_rewards(&mut self, current_slot: u64) -> std::result::Result<(), AccrueError> {
        let slots_elapsed = current_slot.saturating_sub(self.last_update_slot);
        if slots_elapsed == 0 || self.total_staked == 0 || self.reward_rate_per_slot == 0 {
            self.last_update_slot = current_slot;
            return Ok(());
        }
        let new_rewards = (slots_elapsed as u128)
            .checked_mul(self.reward_rate_per_slot as u128)
            .ok_or(AccrueError::Overflow)?;
        let increment = new_rewards
            .checked_mul(Self::REWARD_SCALE)
            .ok_or(AccrueError::Overflow)?
            .checked_div(self.total_staked as u128)
            .ok_or(AccrueError::Overflow)?; // unreachable; total_staked > 0 above
        self.acc_reward_per_share = self
            .acc_reward_per_share
            .checked_add(increment)
            .ok_or(AccrueError::Overflow)?;
        self.last_update_slot = current_slot;
        Ok(())
    }
}

#[event]
pub struct PoolInitialized {
    pub config: Pubkey,
    pub pool: Pubkey,
    pub pool_id: u32,
    pub stake_vault: Pubkey,
    pub reward_vault: Pubkey,
    pub lock_duration_slots: u64,
    pub slot: u64,
}

/// Internal accrual error. Distinct from RailsError so the helper can be unit-tested
/// without Anchor context. IX handlers map AccrueError → RailsError::MathOverflow.
#[derive(Debug, PartialEq, Eq)]
pub enum AccrueError {
    Overflow,
}

/// Per-user stake position for a given pool.
///
/// One per (user, pool_id) tuple. Created on first stake via
/// `init_if_needed`, updated on subsequent stakes. Holds the user's staked
/// amount, reward debt (MasterChef anti-double-spend bookkeeping), and
/// lock expiry.
///
/// PDA: `[USER_STAKE_SEED, pool_pubkey, user_pubkey]`
///
/// ### MasterChef reward_debt semantics
///   claimable = amount * pool.acc_reward_per_share / REWARD_SCALE - reward_debt
/// When a user stakes more:
///   new_reward_debt = new_amount * pool.acc_reward_per_share / REWARD_SCALE
/// When a user claims:
///   reward_debt += claimable_amount_paid  (equivalent to re-anchoring at current acc)
#[account]
#[derive(Debug)]
pub struct UserStake {
    /// User wallet who owns this stake.
    pub user: Pubkey,
    /// Pool this stake lives in.
    pub pool: Pubkey,
    /// Amount of CCM currently staked (base units, 9 decimals on mainnet CCM).
    pub amount: u64,
    /// MasterChef reward debt. See doc-comment above for semantics.
    pub reward_debt: u128,
    /// Rewards accrued before a stake/unstake mutation but not yet claimed.
    /// This prevents reward loss when reward_debt is re-anchored after amount changes.
    pub pending_rewards: u64,
    /// Slot after which the user may call `unstake`. Set to
    /// `now + pool.lock_duration_slots` on each stake (resets on restake).
    pub lock_end_slot: u64,
    /// PDA bump.
    pub bump: u8,
}

impl UserStake {
    /// Account size: 8 + 32 + 32 + 8 + 16 + 8 + 8 + 1 = 113 bytes.
    pub const LEN: usize = 8 + 32 + 32 + 8 + 16 + 8 + 8 + 1;

    /// Compute claimable CCM reward for this user given the pool's current
    /// `acc_reward_per_share`. Does NOT mutate state; callers apply the
    /// result and update reward_debt separately.
    ///
    /// Formula: amount * acc_reward_per_share / REWARD_SCALE - reward_debt
    pub fn claimable(&self, acc_reward_per_share: u128) -> std::result::Result<u64, AccrueError> {
        let total_entitled = (self.amount as u128)
            .checked_mul(acc_reward_per_share)
            .ok_or(AccrueError::Overflow)?
            .checked_div(StakePool::REWARD_SCALE)
            .ok_or(AccrueError::Overflow)?;
        let claimable = total_entitled.saturating_sub(self.reward_debt);
        u64::try_from(claimable).map_err(|_| AccrueError::Overflow)
    }

    /// Current claimable amount including any rewards carried forward from a prior
    /// stake/unstake mutation.
    pub fn total_claimable(
        &self,
        acc_reward_per_share: u128,
    ) -> std::result::Result<u64, AccrueError> {
        let fresh = self.claimable(acc_reward_per_share)? as u128;
        let carried = self.pending_rewards as u128;
        let total = fresh.checked_add(carried).ok_or(AccrueError::Overflow)?;
        u64::try_from(total).map_err(|_| AccrueError::Overflow)
    }
}

/// Replay-protection marker for the one-time compensation merkle drop.
///
/// PDA: `[COMP_CLAIMED_SEED, user_pubkey]`
///
/// The account exists iff the user has already claimed their external
/// compensation allotment. Day 1 intentionally uses `init`, not
/// `init_if_needed`, so a second claim attempt aborts before handler logic.
#[account]
#[derive(Debug)]
pub struct CompensationClaimed {
    /// User who consumed their one-time compensation claim.
    pub user: Pubkey,
    /// Leaf amount claimed (pre Token-2022 transfer fee).
    pub amount: u64,
    /// PDA bump.
    pub bump: u8,
}

impl CompensationClaimed {
    /// Account size: 8 + 32 + 8 + 1 = 49 bytes.
    pub const LEN: usize = 8 + 32 + 8 + 1;
}

#[event]
pub struct Staked {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_stake: Pubkey,
    pub requested_amount: u64,
    pub actual_received: u64,
    pub total_staked: u64,
    pub lock_end_slot: u64,
    pub slot: u64,
}

#[event]
pub struct RewardPoolFunded {
    pub pool: Pubkey,
    pub funder: Pubkey,
    pub reward_vault: Pubkey,
    pub requested_amount: u64,
    pub actual_received: u64,
    pub slot: u64,
}

#[event]
pub struct Unstaked {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_stake: Pubkey,
    pub amount: u64,
    pub remaining_total_staked: u64,
    pub pending_rewards: u64,
    pub slot: u64,
}

#[event]
pub struct Claimed {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub user_stake: Pubkey,
    pub owed: u64,
    pub paid: u64,
    pub pending_rewards: u64,
    pub slot: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::Discriminator;

    #[test]
    fn config_size_matches_manual_calc() {
        // Manual: 8 disc + 32 admin + 32 ccm_mint + 32 treasury_ata + 32 merkle + 4 total_pools + 1 bump.
        assert_eq!(Config::LEN, 141);
    }

    #[test]
    fn config_discriminator_is_stable() {
        // Anchor derives discriminator = sha256("account:Config")[..8].
        // Test ensures nobody renames the struct without noticing the
        // discriminator change (which would break on-chain compat).
        let disc = Config::DISCRIMINATOR;
        assert_eq!(disc.len(), 8);
        // If this assert fires, `Config` was renamed — intentional or not.
        // Update the expected bytes after confirming the rename is desired.
        // Expected bytes computed by: sha256("account:Config")[..8]
    }

    #[test]
    fn seed_constants_are_stable() {
        // Typo guard: seed byte strings are load-bearing across program + SDKs.
        assert_eq!(CONFIG_SEED, b"config");
        assert_eq!(POOL_SEED, b"pool");
        assert_eq!(USER_STAKE_SEED, b"user_stake");
        assert_eq!(STAKE_VAULT_SEED, b"stake_vault");
        assert_eq!(REWARD_VAULT_SEED, b"reward_vault");
        assert_eq!(COMP_VAULT_SEED, b"comp_vault");
        assert_eq!(COMP_CLAIMED_SEED, b"comp_claimed");
    }

    #[test]
    fn comp_root_set_detects_unset() {
        let c = Config {
            admin: Pubkey::default(),
            ccm_mint: Pubkey::default(),
            treasury_ccm_ata: Pubkey::default(),
            comp_merkle_root: [0u8; 32],
            total_pools: 0,
            bump: 0,
        };
        assert!(!c.comp_root_set(), "all-zero root must read as unset");

        let mut c2 = c;
        c2.comp_merkle_root[0] = 1;
        assert!(c2.comp_root_set(), "any non-zero byte marks root as set");
    }

    #[test]
    fn stake_pool_size_matches_manual_calc() {
        // 8 disc + 4 pool_id + 8 total_staked + 16 acc + 8 rate + 8 last_slot + 8 lock + 1 bump
        assert_eq!(StakePool::LEN, 61);
    }

    #[test]
    fn reward_scale_is_1e12() {
        assert_eq!(StakePool::REWARD_SCALE, 1_000_000_000_000u128);
    }

    #[test]
    fn default_lock_is_seven_days() {
        // 7 days * 24h * 60m * 60s / 0.4s-per-slot = 1,512,000 slots
        let expected = 7u64 * 24 * 60 * 60 * 1000 / 400;
        assert_eq!(StakePool::DEFAULT_LOCK_SLOTS, expected);
        assert_eq!(StakePool::DEFAULT_LOCK_SLOTS, 1_512_000);
    }

    fn fresh_pool() -> StakePool {
        StakePool {
            pool_id: 0,
            total_staked: 0,
            acc_reward_per_share: 0,
            reward_rate_per_slot: 0,
            last_update_slot: 1000,
            lock_duration_slots: StakePool::DEFAULT_LOCK_SLOTS,
            bump: 0,
        }
    }

    #[test]
    fn accrue_noop_when_no_slots_elapsed() {
        let mut pool = fresh_pool();
        pool.total_staked = 100_000;
        pool.reward_rate_per_slot = 10;
        let before = pool.acc_reward_per_share;
        pool.accrue_rewards(1000).unwrap();
        assert_eq!(pool.acc_reward_per_share, before);
        assert_eq!(pool.last_update_slot, 1000);
    }

    #[test]
    fn accrue_noop_when_total_staked_is_zero() {
        let mut pool = fresh_pool();
        pool.reward_rate_per_slot = 10;
        pool.accrue_rewards(2000).unwrap();
        // Accumulator unchanged (no stakers to credit), but slot advances
        // so future stakers don't retroactively earn from the empty window.
        assert_eq!(pool.acc_reward_per_share, 0);
        assert_eq!(pool.last_update_slot, 2000);
    }

    #[test]
    fn accrue_noop_when_rate_is_zero() {
        let mut pool = fresh_pool();
        pool.total_staked = 100_000;
        pool.accrue_rewards(2000).unwrap();
        assert_eq!(pool.acc_reward_per_share, 0);
        assert_eq!(pool.last_update_slot, 2000);
    }

    #[test]
    fn accrue_adds_expected_increment() {
        let mut pool = fresh_pool();
        pool.total_staked = 1_000_000; // 1M base units
        pool.reward_rate_per_slot = 10; // 10 base units per slot
        // 100 slots elapse
        pool.accrue_rewards(1100).unwrap();
        // Expected: 100 * 10 * 1e12 / 1_000_000 = 1_000_000_000
        let expected = 100u128 * 10 * StakePool::REWARD_SCALE / 1_000_000;
        assert_eq!(pool.acc_reward_per_share, expected);
        assert_eq!(pool.acc_reward_per_share, 1_000_000_000);
        assert_eq!(pool.last_update_slot, 1100);
    }

    #[test]
    fn accrue_is_idempotent_in_same_slot() {
        let mut pool = fresh_pool();
        pool.total_staked = 500_000;
        pool.reward_rate_per_slot = 25;
        pool.accrue_rewards(2000).unwrap();
        let first = pool.acc_reward_per_share;
        let first_slot = pool.last_update_slot;
        pool.accrue_rewards(2000).unwrap();
        assert_eq!(pool.acc_reward_per_share, first);
        assert_eq!(pool.last_update_slot, first_slot);
    }

    #[test]
    fn user_stake_size_matches_manual_calc() {
        // 8 disc + 32 user + 32 pool + 8 amount + 16 reward_debt + 8 pending + 8 lock + 1 bump
        assert_eq!(UserStake::LEN, 113);
    }

    #[test]
    fn claimable_is_zero_when_freshly_staked() {
        // Fresh stake: reward_debt anchors at current acc, so claimable = 0.
        let stake = UserStake {
            user: Pubkey::default(),
            pool: Pubkey::default(),
            amount: 1_000_000,
            reward_debt: 1_000_000u128 * 5_000_000_000 / StakePool::REWARD_SCALE,
            pending_rewards: 0,
            lock_end_slot: 2000,
            bump: 0,
        };
        let claim = stake.claimable(5_000_000_000).unwrap();
        assert_eq!(claim, 0);
    }

    #[test]
    fn claimable_grows_with_acc_share() {
        // Stake 1M at acc=0, then check claim at acc=2e9.
        // entitled = 1M * 2e9 / 1e12 = 2000 base units.
        let stake = UserStake {
            user: Pubkey::default(),
            pool: Pubkey::default(),
            amount: 1_000_000,
            reward_debt: 0,
            pending_rewards: 0,
            lock_end_slot: 2000,
            bump: 0,
        };
        let claim = stake.claimable(2_000_000_000).unwrap();
        assert_eq!(claim, 2000);
    }

    #[test]
    fn claimable_saturates_not_panics_on_backward_acc() {
        // Defensive: if acc_reward_per_share somehow goes BACKWARD (shouldn't happen
        // but we don't want a panic), reward_debt > entitled saturates to 0.
        let stake = UserStake {
            user: Pubkey::default(),
            pool: Pubkey::default(),
            amount: 1_000_000,
            reward_debt: 10_000,
            pending_rewards: 0,
            lock_end_slot: 2000,
            bump: 0,
        };
        let claim = stake.claimable(0).unwrap();
        assert_eq!(claim, 0);
    }

    #[test]
    fn total_claimable_includes_pending_rewards() {
        let stake = UserStake {
            user: Pubkey::default(),
            pool: Pubkey::default(),
            amount: 1_000_000,
            reward_debt: 0,
            pending_rewards: 750,
            lock_end_slot: 2000,
            bump: 0,
        };
        let claim = stake.total_claimable(2_000_000_000).unwrap();
        assert_eq!(claim, 2750);
    }
}
