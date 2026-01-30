//! Unit-level tests for channel-vault business logic.
//!
//! Tests the vault's share pricing (ERC4626), NAV calculation, reserve mechanics,
//! instant redeem liquidity, and exchange rate invariants.
//! These are pure-logic tests — no CPI or on-chain state required.

use anchor_lang::prelude::Pubkey;
use channel_vault::{ChannelVault, VaultError};

// Re-import constants
const VIRTUAL_SHARES: u64 = 1_000_000_000; // 1e9
const VIRTUAL_ASSETS: u64 = 1_000_000_000; // 1e9
const RESERVE_CAP_BPS: u64 = 500;
const EMERGENCY_PENALTY_BPS: u64 = 2000;
const BPS_DENOMINATOR: u64 = 10_000;

// =========================================================================
// HELPERS
// =========================================================================

fn make_vault() -> ChannelVault {
    ChannelVault {
        bump: 255,
        version: 1,
        channel_config: Pubkey::new_unique(),
        ccm_mint: Pubkey::new_unique(),
        vlofi_mint: Pubkey::new_unique(),
        ccm_buffer: Pubkey::new_unique(),
        total_staked: 0,
        total_shares: 0,
        pending_deposits: 0,
        pending_withdrawals: 0,
        last_compound_slot: 0,
        compound_count: 0,
        admin: Pubkey::new_unique(),
        min_deposit: 1_000_000_000,
        paused: false,
        emergency_reserve: 0,
        lock_duration_slots: 216_000 * 7,
        withdraw_queue_slots: 216_000,
        _reserved: [0u8; 40],
    }
}

/// Simulate a deposit: add actual_received to pending, mint shares.
fn simulate_deposit(vault: &mut ChannelVault, actual_received: u64) -> u64 {
    let shares = vault.calculate_shares(actual_received).unwrap();
    vault.pending_deposits += actual_received;
    vault.total_shares += shares;
    shares
}

/// Simulate a compound: move pending_deposits into total_staked.
fn simulate_compound(vault: &mut ChannelVault) {
    let stakeable = vault.pending_deposits.saturating_sub(vault.pending_withdrawals);
    vault.total_staked += stakeable;
    vault.pending_deposits -= stakeable;
}

// =========================================================================
// SHARE CALCULATION TESTS
// =========================================================================

#[test]
fn test_first_deposit_shares() {
    let vault = make_vault();
    // First deposit: shares = amount * (0 + VIRTUAL_SHARES) / (0 + VIRTUAL_ASSETS)
    // = 1000e9 * 1e9 / 1e9 = 1000e9
    let shares = vault.calculate_shares(1_000_000_000_000).unwrap();
    assert_eq!(shares, 1_000_000_000_000);
}

#[test]
fn test_shares_proportional_to_deposit() {
    let mut vault = make_vault();

    // First deposit: 1000 CCM
    let shares_1 = simulate_deposit(&mut vault, 1_000_000_000_000);

    // Second deposit: same amount should get same shares
    // (NAV hasn't changed, just more deposits + shares)
    let shares_2 = vault.calculate_shares(1_000_000_000_000).unwrap();

    // shares_2 = 1000e9 * (1000e9 + 1e9) / (1000e9 + 1e9)
    // = 1000e9 * 1001e9 / 1001e9 = 1000e9
    // Due to virtual offsets, second deposit gets same shares when no appreciation
    assert_eq!(shares_1, shares_2);
}

#[test]
fn test_shares_diluted_by_appreciation() {
    let mut vault = make_vault();

    // First depositor: 1000 CCM
    let shares_1 = simulate_deposit(&mut vault, 1_000_000_000_000);

    // Simulate appreciation: rewards increase total_staked
    vault.total_staked += 200_000_000_000; // +200 CCM from rewards

    // Second depositor: 1000 CCM — should get fewer shares (vault appreciated)
    let shares_2 = vault.calculate_shares(1_000_000_000_000).unwrap();

    // Second depositor gets fewer shares because NAV/share is higher
    assert!(shares_2 < shares_1, "shares_2 ({}) should be less than shares_1 ({})", shares_2, shares_1);
}

#[test]
fn test_virtual_offset_prevents_inflation_attack() {
    let mut vault = make_vault();

    // Attacker deposits 1 token (minimum)
    let attacker_shares = simulate_deposit(&mut vault, 1_000_000_000);

    // Attacker "donates" a huge amount to inflate exchange rate
    // In real attack, this would be a direct token transfer to the vault
    vault.pending_deposits += 1_000_000_000_000_000; // 1M CCM donated

    // Victim deposits 100 CCM
    let victim_shares = vault.calculate_shares(100_000_000_000).unwrap();

    // With virtual offsets, victim should still get reasonable shares
    // Without virtual offsets, victim would get 0 shares (rounding exploit)
    assert!(victim_shares > 0, "Victim got 0 shares — inflation attack succeeded!");

    // Verify victim gets meaningful shares relative to their deposit
    let redeem = vault.calculate_redeem_amount(victim_shares).unwrap();
    // Victim should get back close to what they put in
    // Some loss is expected due to dilution from donation, but not total loss
    assert!(redeem > 50_000_000_000, "Victim lost >50% to inflation attack: redeems only {}", redeem);
}

#[test]
fn test_zero_deposit_returns_zero_shares() {
    let vault = make_vault();
    let shares = vault.calculate_shares(0).unwrap();
    assert_eq!(shares, 0);
}

// =========================================================================
// REDEEM AMOUNT TESTS
// =========================================================================

#[test]
fn test_redeem_round_trip() {
    let mut vault = make_vault();

    // Deposit 1000 CCM
    let shares = simulate_deposit(&mut vault, 1_000_000_000_000);

    // Redeem all shares — should get back original deposit (minus virtual offset rounding)
    let redeem = vault.calculate_redeem_amount(shares).unwrap();

    // Due to virtual offsets, there's minor rounding loss on first deposit
    // Difference should be < 1 token (1e9 lamports)
    let diff = if redeem > 1_000_000_000_000 {
        redeem - 1_000_000_000_000
    } else {
        1_000_000_000_000 - redeem
    };
    assert!(diff <= 1_000_000_000, "Round-trip loss {} exceeds 1 token", diff);
}

#[test]
fn test_redeem_with_appreciation() {
    let mut vault = make_vault();

    // Deposit 1000 CCM → shares
    let shares = simulate_deposit(&mut vault, 1_000_000_000_000);

    // Simulate 100 CCM appreciation (from staking rewards)
    vault.total_staked += 100_000_000_000;

    // Redeem should return > 1000 CCM (depositor captures appreciation)
    let redeem = vault.calculate_redeem_amount(shares).unwrap();
    assert!(redeem > 1_000_000_000_000, "Redeem {} should exceed 1000 CCM", redeem);
}

#[test]
fn test_partial_redeem() {
    let mut vault = make_vault();

    let shares = simulate_deposit(&mut vault, 1_000_000_000_000);
    let half_shares = shares / 2;

    let full_redeem = vault.calculate_redeem_amount(shares).unwrap();
    let half_redeem = vault.calculate_redeem_amount(half_shares).unwrap();

    // Half the shares should redeem approximately half the CCM
    let diff = if full_redeem / 2 > half_redeem {
        full_redeem / 2 - half_redeem
    } else {
        half_redeem - full_redeem / 2
    };
    // Allow 1 token rounding tolerance
    assert!(diff <= 1_000_000_000, "Half-redeem not proportional: diff={}", diff);
}

// =========================================================================
// NAV AND EXCHANGE RATE TESTS
// =========================================================================

#[test]
fn test_exchange_rate_initial() {
    let vault = make_vault();
    // Empty vault: 1:1 ratio → 1e9
    let rate = vault.exchange_rate().unwrap();
    assert_eq!(rate, 1_000_000_000);
}

#[test]
fn test_exchange_rate_after_deposit() {
    let mut vault = make_vault();
    simulate_deposit(&mut vault, 1_000_000_000_000);

    // After deposit with no appreciation, rate should be ~1e9
    let rate = vault.exchange_rate().unwrap();
    // With virtual offsets, rate is: net_assets * 1e9 / total_shares
    // net_assets = 0 + 1000e9 + 0 - 0 = 1000e9
    // rate = 1000e9 * 1e9 / 1000e9 = 1e9
    assert_eq!(rate, 1_000_000_000);
}

#[test]
fn test_exchange_rate_increases_with_rewards() {
    let mut vault = make_vault();
    simulate_deposit(&mut vault, 1_000_000_000_000);

    let rate_before = vault.exchange_rate().unwrap();

    // Simulate reward accrual
    vault.total_staked += 100_000_000_000;

    let rate_after = vault.exchange_rate().unwrap();
    assert!(rate_after > rate_before, "Rate didn't increase with rewards");
}

#[test]
fn test_nav_components() {
    let mut vault = make_vault();

    vault.total_staked = 500_000_000_000;
    vault.pending_deposits = 200_000_000_000;
    vault.emergency_reserve = 50_000_000_000;
    vault.pending_withdrawals = 100_000_000_000;
    vault.total_shares = 650_000_000_000;

    // NAV = staked + pending + reserve - withdrawals
    // = 500 + 200 + 50 - 100 = 650 (all in 1e9 units)
    let rate = vault.exchange_rate().unwrap();
    // rate = 650e9 * 1e9 / 650e9 = 1e9
    assert_eq!(rate, 1_000_000_000);
}

#[test]
fn test_nav_insolvency_detection() {
    let mut vault = make_vault();
    vault.total_shares = 100;
    vault.pending_withdrawals = 1_000;
    // gross assets = 0, pending_withdrawals = 1000 → insolvent

    let result = vault.exchange_rate();
    assert!(result.is_err(), "Should detect insolvency");
}

// =========================================================================
// RESERVE MECHANICS TESTS
// =========================================================================

#[test]
fn test_reserve_cap_at_5_percent() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000; // 1000 CCM NAV
    vault.total_shares = 1_000_000_000_000;

    // Reserve cap = 5% of NAV = 50 CCM
    let cap = vault.reserve_cap().unwrap();
    let expected = 1_000_000_000_000 * RESERVE_CAP_BPS / BPS_DENOMINATOR;
    assert_eq!(cap, expected);
    assert_eq!(cap, 50_000_000_000); // 50 CCM
}

#[test]
fn test_add_to_reserve_respects_cap() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000; // NAV = 1000 CCM
    vault.total_shares = 1_000_000_000_000;

    // Try to add 100 CCM to reserve (cap is 50 CCM)
    let added = vault.add_to_reserve(100_000_000_000).unwrap();
    assert_eq!(added, 50_000_000_000); // Only 50 fits
    assert_eq!(vault.emergency_reserve, 50_000_000_000);
}

#[test]
fn test_add_to_reserve_partial_fill() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000;
    vault.total_shares = 1_000_000_000_000;

    // First add: 30 CCM (under initial cap of 50 CCM = 5% of 1000)
    let added1 = vault.add_to_reserve(30_000_000_000).unwrap();
    assert_eq!(added1, 30_000_000_000);
    assert_eq!(vault.emergency_reserve, 30_000_000_000);

    // After adding 30 to reserve, NAV = 1000 + 30 = 1030, new cap = 51.5
    // Space = 51.5 - 30 = 21.5 CCM
    let added2 = vault.add_to_reserve(30_000_000_000).unwrap();
    assert_eq!(added2, 21_500_000_000);
    assert_eq!(vault.emergency_reserve, 51_500_000_000);
}

#[test]
fn test_reserve_included_in_nav() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000;
    vault.total_shares = 1_000_000_000_000;

    let rate_before = vault.exchange_rate().unwrap();

    // Add reserve (reserve IS included in NAV, so rate changes)
    vault.emergency_reserve = 50_000_000_000;
    let rate_after = vault.exchange_rate().unwrap();

    assert!(rate_after > rate_before, "Reserve should increase NAV and rate");
}

// =========================================================================
// INSTANT REDEEM LIQUIDITY TESTS
// =========================================================================

#[test]
fn test_available_for_instant_redeem() {
    let mut vault = make_vault();
    vault.pending_withdrawals = 200_000_000_000;

    // Buffer has 500 CCM, 200 reserved for queue
    let available = vault.available_for_instant_redeem(500_000_000_000).unwrap();
    assert_eq!(available, 300_000_000_000);
}

#[test]
fn test_available_for_instant_redeem_insufficient() {
    let mut vault = make_vault();
    vault.pending_withdrawals = 500_000_000_000;

    // Buffer has only 200, but 500 reserved → error
    let result = vault.available_for_instant_redeem(200_000_000_000);
    assert!(result.is_err(), "Should fail when buffer < pending_withdrawals");
}

#[test]
fn test_available_for_instant_redeem_exact() {
    let mut vault = make_vault();
    vault.pending_withdrawals = 500_000_000_000;

    // Buffer equals reserved → 0 available
    let available = vault.available_for_instant_redeem(500_000_000_000).unwrap();
    assert_eq!(available, 0);
}

// =========================================================================
// INSTANT REDEEM PENALTY MATH
// =========================================================================

#[test]
fn test_instant_redeem_penalty_20_percent() {
    // Verify penalty math used in instant_redeem instruction
    let ccm_gross: u64 = 1_000_000_000_000;
    let return_bps = BPS_DENOMINATOR - EMERGENCY_PENALTY_BPS;
    let ccm_returned = (ccm_gross as u128 * return_bps as u128 / BPS_DENOMINATOR as u128) as u64;
    let penalty = ccm_gross - ccm_returned;

    assert_eq!(ccm_returned, 800_000_000_000); // 80%
    assert_eq!(penalty, 200_000_000_000); // 20%
}

#[test]
fn test_instant_redeem_penalty_small_amount() {
    // Verify no rounding issues on small amounts
    let ccm_gross: u64 = 7; // 7 lamports
    let return_bps = BPS_DENOMINATOR - EMERGENCY_PENALTY_BPS;
    let ccm_returned = (ccm_gross as u128 * return_bps as u128 / BPS_DENOMINATOR as u128) as u64;
    let penalty = ccm_gross - ccm_returned;

    // 7 * 8000 / 10000 = 56000 / 10000 = 5 (integer division)
    assert_eq!(ccm_returned, 5);
    assert_eq!(penalty, 2);
}

// =========================================================================
// COMPOUND STATE TRANSITIONS
// =========================================================================

#[test]
fn test_compound_moves_pending_to_staked() {
    let mut vault = make_vault();
    simulate_deposit(&mut vault, 1_000_000_000_000);

    assert_eq!(vault.pending_deposits, 1_000_000_000_000);
    assert_eq!(vault.total_staked, 0);

    simulate_compound(&mut vault);

    assert_eq!(vault.pending_deposits, 0);
    assert_eq!(vault.total_staked, 1_000_000_000_000);
}

#[test]
fn test_compound_preserves_nav() {
    let mut vault = make_vault();
    let shares = simulate_deposit(&mut vault, 1_000_000_000_000);

    let rate_before = vault.exchange_rate().unwrap();
    simulate_compound(&mut vault);
    let rate_after = vault.exchange_rate().unwrap();

    // Compound shouldn't change exchange rate (just moves accounting buckets)
    assert_eq!(rate_before, rate_after);
}

#[test]
fn test_compound_respects_pending_withdrawals() {
    let mut vault = make_vault();
    simulate_deposit(&mut vault, 1_000_000_000_000);

    // Reserve 300 CCM for pending withdrawals
    vault.pending_withdrawals = 300_000_000_000;

    simulate_compound(&mut vault);

    // Only 700 should move to staked (1000 - 300 reserved)
    assert_eq!(vault.total_staked, 700_000_000_000);
    assert_eq!(vault.pending_deposits, 300_000_000_000); // reserved amount stays
}

#[test]
fn test_compound_with_rewards_increases_rate() {
    let mut vault = make_vault();
    let _shares = simulate_deposit(&mut vault, 1_000_000_000_000);

    simulate_compound(&mut vault);
    let rate_before = vault.exchange_rate().unwrap();

    // Simulate reward accrual (e.g., from Oracle staking rewards)
    vault.total_staked += 100_000_000_000;

    let rate_after = vault.exchange_rate().unwrap();
    assert!(rate_after > rate_before, "Rate should increase after rewards");
}

// =========================================================================
// MULTI-DEPOSITOR FAIRNESS TESTS
// =========================================================================

#[test]
fn test_two_depositors_equal_shares_at_parity() {
    let mut vault = make_vault();

    let shares_1 = simulate_deposit(&mut vault, 1_000_000_000_000);
    let shares_2 = simulate_deposit(&mut vault, 1_000_000_000_000);

    assert_eq!(shares_1, shares_2, "Equal deposits at same rate should get equal shares");
}

#[test]
fn test_late_depositor_shares_diluted_by_appreciation() {
    let mut vault = make_vault();

    // Depositor A: 1000 CCM
    let shares_a = simulate_deposit(&mut vault, 1_000_000_000_000);
    simulate_compound(&mut vault);

    // Vault appreciates 50% from rewards
    vault.total_staked = vault.total_staked * 3 / 2;

    // Depositor B: 1000 CCM — gets fewer shares (higher price)
    let shares_b = simulate_deposit(&mut vault, 1_000_000_000_000);

    assert!(shares_b < shares_a, "Late depositor should get fewer shares");

    // But both depositors' share values should reflect their fair portion
    let redeem_a = vault.calculate_redeem_amount(shares_a).unwrap();
    let redeem_b = vault.calculate_redeem_amount(shares_b).unwrap();

    // A deposited 1000, vault went to 1500, so A's share = ~1500
    // B deposited 1000 when NAV was 2500 total, gets share proportional to 1000/2500
    assert!(redeem_a > 1_000_000_000_000, "A should have appreciated");
    // B should get roughly their deposit back (just joined, no appreciation for them)
    assert!(redeem_b <= 1_050_000_000_000, "B shouldn't have significant appreciation");
    assert!(redeem_b >= 950_000_000_000, "B shouldn't lose value on entry");
}

#[test]
fn test_many_small_deposits_equal_one_large() {
    let mut vault_small = make_vault();
    let mut vault_large = make_vault();

    // 10 deposits of 100 CCM each
    let mut total_small_shares = 0u64;
    for _ in 0..10 {
        total_small_shares += simulate_deposit(&mut vault_small, 100_000_000_000);
    }

    // 1 deposit of 1000 CCM
    let large_shares = simulate_deposit(&mut vault_large, 1_000_000_000_000);

    // Should get same total shares (virtual offset makes them equivalent)
    assert_eq!(total_small_shares, large_shares);
}

// =========================================================================
// EDGE CASES
// =========================================================================

#[test]
fn test_minimum_deposit_gets_shares() {
    let vault = make_vault();
    // 1 CCM (9 decimals)
    let shares = vault.calculate_shares(1_000_000_000).unwrap();
    assert!(shares > 0, "Minimum deposit should get non-zero shares");
}

#[test]
fn test_very_large_deposit() {
    let mut vault = make_vault();
    // 1 billion CCM
    let shares = vault.calculate_shares(1_000_000_000_000_000_000).unwrap();
    assert!(shares > 0);

    // Verify redeem returns approximately the same
    vault.pending_deposits = 1_000_000_000_000_000_000;
    vault.total_shares = shares;
    let redeem = vault.calculate_redeem_amount(shares).unwrap();
    let diff = if redeem > 1_000_000_000_000_000_000 {
        redeem - 1_000_000_000_000_000_000
    } else {
        1_000_000_000_000_000_000 - redeem
    };
    // Rounding error should be < 1 CCM on a billion-CCM deposit
    assert!(diff < 1_000_000_000, "Round-trip error {} too large", diff);
}

#[test]
fn test_exchange_rate_precision_with_small_shares() {
    let mut vault = make_vault();
    vault.total_shares = 1; // 1 lamport of shares
    vault.pending_deposits = 1_000_000_000_000; // 1000 CCM backing 1 share

    // Rate should be very high but not overflow
    let rate = vault.exchange_rate().unwrap();
    assert!(rate > 0, "Rate should be calculable even with tiny share count");
}

#[test]
fn test_pending_withdrawals_reduce_nav() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000;
    vault.total_shares = 1_000_000_000_000;

    let rate_before = vault.exchange_rate().unwrap();

    vault.pending_withdrawals = 200_000_000_000;

    let rate_after = vault.exchange_rate().unwrap();
    assert!(rate_after < rate_before, "Pending withdrawals should reduce NAV and rate");
}

#[test]
fn test_reserve_cap_zero_when_empty_vault() {
    let vault = make_vault();
    // Empty vault: NAV = 0
    let cap = vault.reserve_cap().unwrap();
    assert_eq!(cap, 0);
}

#[test]
fn test_add_to_reserve_negligible_when_near_cap() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000;
    vault.total_shares = 1_000_000_000_000;

    // Set reserve to 50 CCM. NAV = 1000 + 50 = 1050, cap = 52.5 CCM.
    // Space = 52.5 - 50 = 2.5 CCM (reserve in NAV creates small headroom)
    vault.emergency_reserve = 50_000_000_000;

    let added = vault.add_to_reserve(10_000_000_000).unwrap();
    assert_eq!(added, 2_500_000_000); // Only 2.5 CCM fits
    assert_eq!(vault.emergency_reserve, 52_500_000_000);
}

#[test]
fn test_add_to_reserve_truly_zero_when_saturated() {
    let mut vault = make_vault();
    vault.pending_deposits = 1_000_000_000_000;
    vault.total_shares = 1_000_000_000_000;

    // Iteratively fill to saturation
    // The fixed point is: reserve = 5% * (1000 + reserve)
    // → reserve = 50 + 0.05*reserve → 0.95*reserve = 50 → reserve ≈ 52.63 CCM
    // After a few rounds of add_to_reserve, converges:
    for _ in 0..10 {
        let added = vault.add_to_reserve(100_000_000_000).unwrap();
        if added == 0 {
            break;
        }
    }

    // Now truly at cap — adding more returns 0
    let added = vault.add_to_reserve(10_000_000_000).unwrap();
    assert_eq!(added, 0);
}
