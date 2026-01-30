//! Staking invariant tests for channel staking with Token-2022 transfer fees.
//!
//! Run with: `cargo test --package attention-oracle-token-2022 --test litesvm_staking`
//!
//! Tests cover:
//! - MasterChef reward accumulator math (pure functions)
//! - Fee-deficit accounting invariants (actual_received vs requested amount)
//! - PendingRewardsOnUnstake guard (claimable vs underfunded)
//! - ClaimExceedsAvailableRewards principal protection
//! - Boost multiplier tier correctness

use solana_sdk::pubkey::Pubkey;

// Import program types and functions via the crate's public API
use token_2022::{
    calculate_boost_bps, calculate_pending_rewards, calculate_reward_debt, update_pool_rewards,
    ChannelStakePool, UserChannelStake, BOOST_PRECISION, REWARD_PRECISION, SLOTS_PER_DAY,
};

// ============================================================================
// Helpers
// ============================================================================

fn make_pool(total_staked: u64, total_weighted: u64, reward_per_slot: u64) -> ChannelStakePool {
    ChannelStakePool {
        bump: 0,
        channel: Pubkey::default(),
        mint: Pubkey::default(),
        vault: Pubkey::default(),
        total_staked,
        total_weighted,
        staker_count: 1,
        acc_reward_per_share: 0,
        last_reward_slot: 0,
        reward_per_slot,
        is_shutdown: false,
    }
}

fn make_user_stake(amount: u64, multiplier_bps: u64, reward_debt: u128) -> UserChannelStake {
    UserChannelStake {
        bump: 0,
        user: Pubkey::default(),
        channel: Pubkey::default(),
        amount,
        start_slot: 0,
        lock_end_slot: 0,
        multiplier_bps,
        nft_mint: Pubkey::default(),
        reward_debt,
        pending_rewards: 0,
    }
}

// ============================================================================
// Part 1: Pure Reward Math Tests
// ============================================================================

#[test]
fn test_reward_accumulator_basic() {
    let mut pool = make_pool(
        10_000_000_000, // 10 CCM staked
        10_000_000_000, // 10 CCM weighted (1x multiplier)
        1_000,          // 1000 lamports per slot
    );
    pool.last_reward_slot = 0;

    // Advance 1000 slots
    update_pool_rewards(&mut pool, 1000).unwrap();

    // Expected: acc += (1000 * 1000 * 1e12) / 10_000_000_000
    //         = 1_000_000 * 1e12 / 10e9 = 100_000
    let expected_acc = (1_000u128 * 1000 * REWARD_PRECISION) / 10_000_000_000u128;
    assert_eq!(pool.acc_reward_per_share, expected_acc);
    assert_eq!(pool.last_reward_slot, 1000);
}

#[test]
fn test_reward_accumulator_no_elapsed() {
    let mut pool = make_pool(10_000_000_000, 10_000_000_000, 1_000);
    pool.last_reward_slot = 500;

    // Same slot - no change
    update_pool_rewards(&mut pool, 500).unwrap();
    assert_eq!(pool.acc_reward_per_share, 0);
}

#[test]
fn test_reward_accumulator_no_stakers() {
    let mut pool = make_pool(0, 0, 1_000);
    pool.last_reward_slot = 0;

    // No stakers - just update last_reward_slot
    update_pool_rewards(&mut pool, 1000).unwrap();
    assert_eq!(pool.acc_reward_per_share, 0);
    assert_eq!(pool.last_reward_slot, 1000);
}

#[test]
fn test_pending_rewards_basic() {
    let mut pool = make_pool(10_000_000_000, 10_000_000_000, 1_000);
    pool.last_reward_slot = 0;

    // Advance 1000 slots to accrue rewards
    update_pool_rewards(&mut pool, 1000).unwrap();

    // User staked 5 CCM at 1x multiplier, debt set at accumulator=0
    let user_stake = make_user_stake(5_000_000_000, BOOST_PRECISION, 0);

    let pending = calculate_pending_rewards(&user_stake, &pool).unwrap();

    // User's weighted stake = 5e9 * 10000 / 10000 = 5e9
    // accumulated = 5e9 * acc / 1e12
    // acc = 1_000_000 * 1e12 / 10e9 = 100_000
    // accumulated = 5e9 * 100_000 / 1e12 = 500_000
    // pending = 500_000 - 0 + 0 = 500_000
    let expected = 500_000u64;
    assert_eq!(pending, expected);
}

#[test]
fn test_pending_rewards_with_debt() {
    let mut pool = make_pool(10_000_000_000, 10_000_000_000, 1_000);
    pool.last_reward_slot = 0;

    // Advance 1000 slots
    update_pool_rewards(&mut pool, 1000).unwrap();

    // User with reward_debt matching accumulated (just staked)
    let debt = calculate_reward_debt(5_000_000_000, BOOST_PRECISION, pool.acc_reward_per_share).unwrap();
    let user_stake = make_user_stake(5_000_000_000, BOOST_PRECISION, debt);

    let pending = calculate_pending_rewards(&user_stake, &pool).unwrap();
    assert_eq!(pending, 0, "Newly staked user should have zero pending rewards");
}

#[test]
fn test_pending_rewards_accrue_after_stake() {
    let mut pool = make_pool(10_000_000_000, 10_000_000_000, 1_000);
    pool.last_reward_slot = 0;

    // First phase: accumulate 1000 slots before user stakes
    update_pool_rewards(&mut pool, 1000).unwrap();

    // User stakes 5 CCM at slot 1000 (debt set to current accumulator)
    let debt = calculate_reward_debt(5_000_000_000, BOOST_PRECISION, pool.acc_reward_per_share).unwrap();

    // Add user to pool
    pool.total_staked += 5_000_000_000;
    pool.total_weighted += 5_000_000_000;

    // Second phase: accumulate 500 more slots
    update_pool_rewards(&mut pool, 1500).unwrap();

    let user_stake = make_user_stake(5_000_000_000, BOOST_PRECISION, debt);
    let pending = calculate_pending_rewards(&user_stake, &pool).unwrap();

    // User should only earn rewards from slots 1000-1500 (500 slots)
    // Total weighted is now 15e9
    // acc_increase = (1000 * 500 * 1e12) / 15e9 = 500_000e12 / 15e9 = 33_333
    // user_accumulated = 5e9 * (old_acc + 33_333) / 1e12
    // user_pending = user_accumulated - debt
    assert!(pending > 0, "User should have accrued rewards after staking");
}

#[test]
fn test_reward_debt_calculation() {
    let amount = 10_000_000_000u64;
    let multiplier = 20_000u64; // 2x boost
    let acc = 500_000u128;

    let debt = calculate_reward_debt(amount, multiplier, acc).unwrap();

    // weighted_stake = 10e9 * 20000 / 10000 = 20e9
    // debt = 20e9 * 500_000 / 1e12 = 10_000
    let weighted = (amount as u128) * (multiplier as u128) / (BOOST_PRECISION as u128);
    let expected = weighted * acc / REWARD_PRECISION;
    assert_eq!(debt, expected);
}

#[test]
fn test_boosted_rewards() {
    // Two users: one at 1x, one at 3x multiplier
    let amount = 10_000_000_000u64; // 10 CCM each

    let weighted_1x = amount; // 10e9 * 10000/10000
    let weighted_3x = (amount as u128 * 30_000 / BOOST_PRECISION as u128) as u64; // 30e9

    let total_weighted = weighted_1x + weighted_3x; // 40e9
    let mut pool = make_pool(amount * 2, total_weighted, 1_000);
    pool.last_reward_slot = 0;

    update_pool_rewards(&mut pool, 1000).unwrap();

    let user_1x = make_user_stake(amount, BOOST_PRECISION, 0);
    let user_3x = make_user_stake(amount, 30_000, 0);

    let pending_1x = calculate_pending_rewards(&user_1x, &pool).unwrap();
    let pending_3x = calculate_pending_rewards(&user_3x, &pool).unwrap();

    // 3x user should earn 3x the rewards of 1x user
    assert_eq!(pending_3x, pending_1x * 3, "3x boost should yield 3x rewards");
}

// ============================================================================
// Part 2: Boost Multiplier Tier Tests
// ============================================================================

#[test]
fn test_boost_multiplier_tiers() {
    // No lock
    assert_eq!(calculate_boost_bps(0), 10_000, "0 days = 1.0x");

    // < 7 days
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 6),
        10_000,
        "6 days = 1.0x"
    );

    // 7-29 days
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 7),
        12_500,
        "7 days = 1.25x"
    );
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 29),
        12_500,
        "29 days = 1.25x"
    );

    // 30-89 days
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 30),
        15_000,
        "30 days = 1.5x"
    );
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 89),
        15_000,
        "89 days = 1.5x"
    );

    // 90-179 days
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 90),
        20_000,
        "90 days = 2.0x"
    );
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 179),
        20_000,
        "179 days = 2.0x"
    );

    // 180-364 days
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 180),
        25_000,
        "180 days = 2.5x"
    );
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 364),
        25_000,
        "364 days = 2.5x"
    );

    // 365+ days
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 365),
        30_000,
        "365 days = 3.0x"
    );
    assert_eq!(
        calculate_boost_bps(SLOTS_PER_DAY * 730),
        30_000,
        "730 days = 3.0x"
    );
}

#[test]
fn test_boost_boundary_slots() {
    // Exactly at day boundary (slots_per_day - 1 vs slots_per_day)
    let just_under_7 = SLOTS_PER_DAY * 7 - 1;
    let exactly_7 = SLOTS_PER_DAY * 7;

    assert_eq!(calculate_boost_bps(just_under_7), 10_000, "6.99 days = 1.0x");
    assert_eq!(calculate_boost_bps(exactly_7), 12_500, "7.00 days = 1.25x");
}

// ============================================================================
// Part 3: Fee-Deficit Invariant Tests
// ============================================================================

#[test]
fn test_fee_deficit_breaks_claim_invariant() {
    // Scenario: total_staked records pre-fee amount (OLD BUG)
    // User stakes 10 CCM, vault receives 9.95 CCM (0.5% fee)
    let fee_bps: u64 = 50;
    let requested = 10_000_000_000u64; // 10 CCM
    let fee = requested * fee_bps / 10_000; // 50_000_000
    let vault_balance = requested - fee; // 9_950_000_000 (what vault actually got)

    // BUG: total_staked = requested (pre-fee)
    let total_staked_buggy = requested;

    // Excess = vault_balance - total_staked (saturates to 0)
    let excess = vault_balance.saturating_sub(total_staked_buggy);
    assert_eq!(excess, 0, "Fee deficit makes excess 0");

    // Even tiny pending rewards fail the invariant
    let pending = 1u64;
    assert!(
        excess < pending,
        "BUG: All claims blocked when total_staked uses pre-fee amount"
    );
}

#[test]
fn test_actual_received_preserves_claim_invariant() {
    // Scenario: total_staked records actual_received (FIXED)
    let fee_bps: u64 = 50;
    let requested = 10_000_000_000u64;
    let fee = requested * fee_bps / 10_000;
    let actual_received = requested - fee; // 9_950_000_000

    // FIXED: total_staked = actual_received
    let vault_balance = actual_received;
    let total_staked = actual_received;

    // No rewards deposited yet -> excess = 0
    let excess = vault_balance.saturating_sub(total_staked);
    assert_eq!(excess, 0, "No excess when no rewards deposited");

    // Deposit rewards to vault
    let rewards = 500_000_000u64; // 0.5 CCM
    let vault_with_rewards = vault_balance + rewards;
    let excess_with_rewards = vault_with_rewards.saturating_sub(total_staked);
    assert_eq!(
        excess_with_rewards, rewards,
        "Excess should equal deposited rewards"
    );

    // Claims succeed
    let pending = 100_000_000u64;
    assert!(
        excess_with_rewards >= pending,
        "Claim should succeed when rewards available"
    );
}

#[test]
fn test_multiple_stakers_fee_accounting() {
    // Multiple users stake with transfer fees
    let fee_bps: u64 = 50;

    let stake_1 = 10_000_000_000u64;
    let received_1 = stake_1 - (stake_1 * fee_bps / 10_000); // 9_950_000_000

    let stake_2 = 5_000_000_000u64;
    let received_2 = stake_2 - (stake_2 * fee_bps / 10_000); // 4_975_000_000

    let vault_balance = received_1 + received_2; // 14_925_000_000
    let total_staked = received_1 + received_2; // 14_925_000_000 (FIXED)

    // Invariant holds
    let excess = vault_balance.saturating_sub(total_staked);
    assert_eq!(excess, 0);

    // Add rewards
    let rewards = 1_000_000_000u64;
    let vault_with_rewards = vault_balance + rewards;
    let excess_with_rewards = vault_with_rewards.saturating_sub(total_staked);
    assert_eq!(excess_with_rewards, rewards);

    // Buggy accounting would have:
    let total_staked_buggy = stake_1 + stake_2; // 15_000_000_000
    let excess_buggy = vault_with_rewards.saturating_sub(total_staked_buggy);
    // vault_with_rewards = 15_925_000_000, total_staked_buggy = 15_000_000_000
    // excess_buggy = 925_000_000 (WRONG - understates available by 75_000_000)
    // But more critically, without rewards: vault=14_925_000_000 vs staked=15_000_000_000
    let excess_buggy_no_rewards = vault_balance.saturating_sub(total_staked_buggy);
    assert_eq!(
        excess_buggy_no_rewards, 0,
        "BUG: fee deficit eats into reward space"
    );
    assert!(
        excess_buggy < rewards,
        "BUG: fee deficit understates available rewards"
    );
}

// ============================================================================
// Part 4: Unstake Guard Invariant Tests
// ============================================================================

/// Simulates the unstake guard logic from unstake_channel
fn unstake_guard(
    pending: u64,
    is_shutdown: bool,
    vault_balance: u64,
    total_staked: u64,
) -> Result<(), &'static str> {
    if pending > 0 && !is_shutdown {
        let excess = vault_balance.saturating_sub(total_staked);
        if excess >= pending {
            // Rewards are claimable - block unstake
            return Err("PendingRewardsOnUnstake");
        }
        // Underfunded - allow forfeit
    }
    Ok(())
}

#[test]
fn test_unstake_blocked_when_rewards_claimable() {
    // User has 100 pending, vault has 100 excess -> must claim first
    let result = unstake_guard(
        100_000_000,      // pending
        false,            // not shutdown
        10_100_000_000,   // vault_balance (staked + rewards)
        10_000_000_000,   // total_staked
    );
    assert_eq!(result, Err("PendingRewardsOnUnstake"));
}

#[test]
fn test_unstake_allowed_when_no_pending() {
    // No pending rewards -> unstake proceeds
    let result = unstake_guard(
        0,                // no pending
        false,            // not shutdown
        10_000_000_000,   // vault_balance
        10_000_000_000,   // total_staked
    );
    assert!(result.is_ok());
}

#[test]
fn test_unstake_allowed_when_shutdown() {
    // Pool shutdown -> unstake always proceeds regardless of pending
    let result = unstake_guard(
        100_000_000,      // has pending
        true,             // SHUTDOWN
        10_100_000_000,   // vault with rewards
        10_000_000_000,   // total_staked
    );
    assert!(result.is_ok(), "Shutdown should bypass pending check");
}

#[test]
fn test_unstake_allowed_when_underfunded() {
    // Pending = 100, but vault has no excess (underfunded) -> forfeit + proceed
    let result = unstake_guard(
        100_000_000,      // pending
        false,            // not shutdown
        10_000_000_000,   // vault_balance == total_staked (no excess)
        10_000_000_000,   // total_staked
    );
    assert!(
        result.is_ok(),
        "Underfunded rewards should allow unstake with forfeit"
    );
}

#[test]
fn test_unstake_allowed_when_partially_funded() {
    // Pending = 100M, but only 50M available -> underfunded, allow exit
    let result = unstake_guard(
        100_000_000,      // pending 100M lamports
        false,            // not shutdown
        10_050_000_000,   // vault = staked + 50M
        10_000_000_000,   // total_staked
    );
    assert!(
        result.is_ok(),
        "Partially funded rewards (50M < 100M pending) should allow unstake"
    );
}

#[test]
fn test_unstake_blocked_when_exactly_funded() {
    // Pending = exactly available excess -> claimable, block unstake
    let result = unstake_guard(
        100_000_000,      // pending
        false,            // not shutdown
        10_100_000_000,   // vault = staked + exactly pending
        10_000_000_000,   // total_staked
    );
    assert_eq!(
        result,
        Err("PendingRewardsOnUnstake"),
        "Exactly funded rewards should still require claim"
    );
}

// ============================================================================
// Part 5: Claim Invariant Tests
// ============================================================================

/// Simulates the claim invariant check from claim_channel_rewards
fn claim_invariant(pending: u64, vault_balance: u64, total_staked: u64) -> Result<(), &'static str> {
    if pending == 0 {
        return Err("NoRewardsToClaim");
    }
    let excess = vault_balance.saturating_sub(total_staked);
    if excess < pending {
        return Err("ClaimExceedsAvailableRewards");
    }
    Ok(())
}

#[test]
fn test_claim_succeeds_when_funded() {
    let result = claim_invariant(
        100_000_000,      // pending 0.1 CCM
        10_500_000_000,   // vault has 0.5 CCM excess
        10_000_000_000,   // total_staked
    );
    assert!(result.is_ok());
}

#[test]
fn test_claim_blocked_when_no_rewards() {
    let result = claim_invariant(
        0,                // no pending
        10_500_000_000,
        10_000_000_000,
    );
    assert_eq!(result, Err("NoRewardsToClaim"));
}

#[test]
fn test_claim_blocked_when_underfunded() {
    let result = claim_invariant(
        500_000_000,      // pending 0.5 CCM
        10_100_000_000,   // only 0.1 CCM excess
        10_000_000_000,   // total_staked
    );
    assert_eq!(result, Err("ClaimExceedsAvailableRewards"));
}

#[test]
fn test_claim_blocked_when_fee_deficit() {
    // Fee deficit: vault < total_staked (buggy pre-fee accounting)
    let result = claim_invariant(
        1,                // even 1 lamport fails
        9_950_000_000,    // vault (post-fee)
        10_000_000_000,   // total_staked (pre-fee BUG)
    );
    assert_eq!(
        result,
        Err("ClaimExceedsAvailableRewards"),
        "Fee deficit should block all claims"
    );
}

#[test]
fn test_claim_exactly_available() {
    // Claim exactly the available excess
    let result = claim_invariant(
        500_000_000,      // pending = excess
        10_500_000_000,   // vault
        10_000_000_000,   // staked
    );
    assert!(result.is_ok(), "Claim should succeed when pending == excess");
}

// ============================================================================
// Part 6: End-to-End Reward Scenario Tests
// ============================================================================

#[test]
fn test_full_reward_lifecycle() {
    // Simulate: stake -> accrue -> claim -> verify debt reset

    // 1. Initialize pool with 100 CCM staked at 1x
    let staked = 100_000_000_000u64; // 100 CCM
    let mut pool = make_pool(staked, staked, 10_000); // 10k lamports/slot
    pool.last_reward_slot = 0;

    // 2. User stakes 10 CCM (actual_received after 0.5% fee)
    let user_requested = 10_000_000_000u64;
    let user_received = user_requested - (user_requested * 50 / 10_000); // 9_950_000_000

    // Update pool before adding user
    update_pool_rewards(&mut pool, 0).unwrap();
    let debt = calculate_reward_debt(user_received, BOOST_PRECISION, pool.acc_reward_per_share).unwrap();

    pool.total_staked += user_received;
    pool.total_weighted += user_received;

    let user = make_user_stake(user_received, BOOST_PRECISION, debt);

    // 3. Advance 10000 slots
    update_pool_rewards(&mut pool, 10_000).unwrap();

    // 4. Calculate pending
    let pending = calculate_pending_rewards(&user, &pool).unwrap();
    assert!(pending > 0, "User should have pending rewards after 10000 slots");

    // 5. Verify claim invariant holds
    let vault_balance = pool.total_staked + pending + 1_000_000; // some buffer
    let excess = vault_balance.saturating_sub(pool.total_staked);
    assert!(excess >= pending, "Claim should succeed");

    // 6. After claim: debt resets to current accumulator
    let new_debt =
        calculate_reward_debt(user_received, BOOST_PRECISION, pool.acc_reward_per_share).unwrap();
    let post_claim_user = make_user_stake(user_received, BOOST_PRECISION, new_debt);
    let post_claim_pending = calculate_pending_rewards(&post_claim_user, &pool).unwrap();
    assert_eq!(
        post_claim_pending, 0,
        "Pending should be 0 after claim resets debt"
    );
}

#[test]
fn test_weighted_rewards_proportional() {
    // Three users with different boosts, verify proportional distribution
    let base = 10_000_000_000u64; // 10 CCM each

    let w_1x = base; // 1.0x
    let w_2x = base * 2; // 2.0x
    let w_3x = base * 3; // 3.0x
    let total_weighted = w_1x + w_2x + w_3x; // 60 CCM weighted

    let mut pool = make_pool(base * 3, total_weighted, 6_000); // 6k/slot for clean math
    pool.last_reward_slot = 0;

    update_pool_rewards(&mut pool, 1000).unwrap();

    // Total rewards = 6000 * 1000 = 6_000_000
    let user_1x = make_user_stake(base, BOOST_PRECISION, 0);
    let user_2x = make_user_stake(base, 20_000, 0);
    let user_3x = make_user_stake(base, 30_000, 0);

    let p1 = calculate_pending_rewards(&user_1x, &pool).unwrap();
    let p2 = calculate_pending_rewards(&user_2x, &pool).unwrap();
    let p3 = calculate_pending_rewards(&user_3x, &pool).unwrap();

    // p1 : p2 : p3 should be 1 : 2 : 3
    assert_eq!(p2, p1 * 2, "2x user gets 2x rewards");
    assert_eq!(p3, p1 * 3, "3x user gets 3x rewards");

    // Total distributed should equal total accrued
    let total_distributed = p1 + p2 + p3;
    let total_accrued = 6_000u64 * 1000;
    assert_eq!(
        total_distributed, total_accrued,
        "Sum of rewards should equal total accrued"
    );
}

#[test]
fn test_fee_accounting_preserves_proportional_rewards() {
    // With transfer fees, rewards are still proportional
    let fee_bps = 50u64; // 0.5%

    let requested_1 = 10_000_000_000u64;
    let received_1 = requested_1 - (requested_1 * fee_bps / 10_000);

    let requested_2 = 20_000_000_000u64;
    let received_2 = requested_2 - (requested_2 * fee_bps / 10_000);

    // Both at 1x multiplier
    let total_staked = received_1 + received_2;
    let total_weighted = total_staked;

    let mut pool = make_pool(total_staked, total_weighted, 1_000);
    pool.last_reward_slot = 0;
    update_pool_rewards(&mut pool, 1000).unwrap();

    let debt_1 = 0u128; // Both staked at slot 0
    let debt_2 = 0u128;

    let user_1 = make_user_stake(received_1, BOOST_PRECISION, debt_1);
    let user_2 = make_user_stake(received_2, BOOST_PRECISION, debt_2);

    let p1 = calculate_pending_rewards(&user_1, &pool).unwrap();
    let p2 = calculate_pending_rewards(&user_2, &pool).unwrap();

    // User 2 staked ~2x user 1, should get ~2x rewards
    // received_2 / received_1 = 19_900_000_000 / 9_950_000_000 = 2.0
    assert_eq!(
        p2,
        p1 * 2,
        "Double stake should yield double rewards (even with fees)"
    );
}

// ============================================================================
// Part 7: Edge Cases
// ============================================================================

#[test]
fn test_zero_reward_rate() {
    let mut pool = make_pool(10_000_000_000, 10_000_000_000, 0); // zero rate
    pool.last_reward_slot = 0;

    update_pool_rewards(&mut pool, 10_000).unwrap();
    assert_eq!(
        pool.acc_reward_per_share, 0,
        "Zero reward rate should not accrue"
    );

    let user = make_user_stake(5_000_000_000, BOOST_PRECISION, 0);
    let pending = calculate_pending_rewards(&user, &pool).unwrap();
    assert_eq!(pending, 0);
}

#[test]
fn test_very_small_stake_large_pool() {
    // 1 lamport staked in a pool with 1B CCM
    let mut pool = make_pool(
        1_000_000_000_000_000_000, // 1B CCM
        1_000_000_000_000_000_000,
        1_000_000, // 1M lamports/slot
    );
    pool.last_reward_slot = 0;

    update_pool_rewards(&mut pool, 1000).unwrap();

    let user = make_user_stake(1, BOOST_PRECISION, 0); // 1 lamport
    let pending = calculate_pending_rewards(&user, &pool).unwrap();

    // Reward for 1 lamport in 1B pool over 1000 slots = essentially 0
    // 1 * acc / 1e12, where acc = 1e6 * 1000 * 1e12 / 1e18 = 1
    // pending = 1 * 1 / 1e12 = 0 (integer truncation)
    assert_eq!(pending, 0, "Dust stake in large pool rounds to 0");
}

#[test]
fn test_max_boost_multiplier_rewards() {
    // 365-day lock = 3x boost
    let amount = 10_000_000_000u64;
    let multiplier = calculate_boost_bps(SLOTS_PER_DAY * 365);
    assert_eq!(multiplier, 30_000);

    let weighted = (amount as u128 * multiplier as u128 / BOOST_PRECISION as u128) as u64;
    assert_eq!(weighted, amount * 3, "3x multiplier triples weighted stake");

    let mut pool = make_pool(amount, weighted, 1_000);
    pool.last_reward_slot = 0;
    update_pool_rewards(&mut pool, 1000).unwrap();

    let user = make_user_stake(amount, multiplier, 0);
    let pending = calculate_pending_rewards(&user, &pool).unwrap();

    // Solo staker gets all rewards (±1 lamport for integer division truncation)
    let total_rewards = 1_000u64 * 1000;
    assert!(
        total_rewards - pending <= 1,
        "Solo 3x staker should get all rewards (got {} of {})",
        pending,
        total_rewards
    );
}

#[test]
fn test_shutdown_stops_accrual() {
    let mut pool = make_pool(10_000_000_000, 10_000_000_000, 1_000);
    pool.last_reward_slot = 0;

    // Accrue for 1000 slots
    update_pool_rewards(&mut pool, 1000).unwrap();
    let acc_before_shutdown = pool.acc_reward_per_share;

    // Shutdown: set rate to 0
    pool.reward_per_slot = 0;
    pool.is_shutdown = true;

    // Advance another 1000 slots
    update_pool_rewards(&mut pool, 2000).unwrap();

    assert_eq!(
        pool.acc_reward_per_share, acc_before_shutdown,
        "No rewards should accrue after shutdown"
    );
}
