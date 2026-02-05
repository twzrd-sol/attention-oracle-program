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

// =========================================================================
// SLIPPAGE PROTECTION TESTS (complete_withdraw)
// =========================================================================

/// Simulates the slippage check from complete_withdraw.
/// Returns Ok(actual_received) if passes, Err if slippage exceeded.
fn check_slippage(
    ccm_amount_sent: u64,
    transfer_fee_bps: u64,
    min_ccm_amount: u64,
) -> Result<u64, VaultError> {
    // Calculate what user actually receives after transfer fee
    let fee = (ccm_amount_sent as u128)
        .checked_mul(transfer_fee_bps as u128)
        .unwrap()
        .checked_div(BPS_DENOMINATOR as u128)
        .unwrap() as u64;
    let actual_received = ccm_amount_sent.saturating_sub(fee);

    // Slippage check from complete_withdraw (line 476-479 in redeem.rs)
    if actual_received >= min_ccm_amount {
        Ok(actual_received)
    } else {
        Err(VaultError::SlippageExceeded)
    }
}

#[test]
fn test_slippage_protection_fails_when_min_too_high() {
    // CCM has 0.5% transfer fee (50 bps)
    let ccm_amount = 1_000_000_000_000u64; // 1000 CCM sent
    let transfer_fee_bps = 50u64; // 0.5%

    // Expected: user receives 995 CCM (1000 - 0.5%)
    let expected_received = 995_000_000_000u64;

    // User sets min_ccm_amount higher than what they'll receive
    let min_too_high = 996_000_000_000u64; // 996 CCM

    let result = check_slippage(ccm_amount, transfer_fee_bps, min_too_high);
    assert!(
        matches!(result, Err(VaultError::SlippageExceeded)),
        "Should fail when min_ccm_amount ({}) > actual_received ({})",
        min_too_high,
        expected_received
    );
}

#[test]
fn test_slippage_protection_succeeds_when_min_correct() {
    // CCM has 0.5% transfer fee (50 bps)
    let ccm_amount = 1_000_000_000_000u64; // 1000 CCM sent
    let transfer_fee_bps = 50u64; // 0.5%

    // Expected: user receives 995 CCM
    let expected_received = 995_000_000_000u64;

    // User sets min_ccm_amount correctly (accounting for fee)
    let min_correct = 995_000_000_000u64;

    let result = check_slippage(ccm_amount, transfer_fee_bps, min_correct);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_received);
}

#[test]
fn test_slippage_protection_succeeds_with_buffer() {
    // CCM has 0.5% transfer fee (50 bps)
    let ccm_amount = 1_000_000_000_000u64;
    let transfer_fee_bps = 50u64;

    // User sets min with some safety buffer (990 CCM < 995 CCM actual)
    let min_with_buffer = 990_000_000_000u64;

    let result = check_slippage(ccm_amount, transfer_fee_bps, min_with_buffer);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 995_000_000_000u64);
}

#[test]
fn test_slippage_protection_boundary_exactly_equal() {
    // Test boundary: min_ccm_amount == actual_received (should pass)
    let ccm_amount = 1_000_000_000_000u64;
    let transfer_fee_bps = 50u64;
    let min_exactly_equal = 995_000_000_000u64;

    let result = check_slippage(ccm_amount, transfer_fee_bps, min_exactly_equal);
    assert!(result.is_ok(), "Should pass when min == actual");
}

#[test]
fn test_slippage_protection_boundary_one_lamport_over() {
    // Test boundary: min_ccm_amount = actual_received + 1 (should fail)
    let ccm_amount = 1_000_000_000_000u64;
    let transfer_fee_bps = 50u64;
    let min_one_over = 995_000_000_001u64;

    let result = check_slippage(ccm_amount, transfer_fee_bps, min_one_over);
    assert!(
        matches!(result, Err(VaultError::SlippageExceeded)),
        "Should fail when min is 1 lamport over actual"
    );
}

#[test]
fn test_slippage_protection_with_zero_fee() {
    // Edge case: no transfer fee (theoretical)
    let ccm_amount = 1_000_000_000_000u64;
    let transfer_fee_bps = 0u64;
    let min_amount = 1_000_000_000_000u64;

    let result = check_slippage(ccm_amount, transfer_fee_bps, min_amount);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ccm_amount, "No fee means full amount received");
}

#[test]
fn test_slippage_protection_with_high_fee() {
    // Edge case: 5% transfer fee
    let ccm_amount = 1_000_000_000_000u64;
    let transfer_fee_bps = 500u64; // 5%

    // User receives 950 CCM
    let expected = 950_000_000_000u64;

    // Setting min at expected should pass
    let result = check_slippage(ccm_amount, transfer_fee_bps, expected);
    assert!(result.is_ok());

    // Setting min above expected should fail
    let result = check_slippage(ccm_amount, transfer_fee_bps, expected + 1);
    assert!(matches!(result, Err(VaultError::SlippageExceeded)));
}

#[test]
fn test_slippage_protection_small_amount() {
    // Small amount where fee rounding matters
    let ccm_amount = 1_000_000u64; // 0.001 CCM
    let transfer_fee_bps = 50u64;

    // Fee = 1_000_000 * 50 / 10_000 = 5_000 lamports
    let expected = 995_000u64;

    let result = check_slippage(ccm_amount, transfer_fee_bps, expected);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected);
}

// =========================================================================
// COMPOUND BOUNTY PAYMENT TESTS
// =========================================================================

/// Calculate bounty payment (mirrors compound.rs lines 335-340)
fn calculate_bounty(rewards_claimed: u64, bounty_bps: u64) -> u64 {
    (rewards_claimed as u128)
        .checked_mul(bounty_bps as u128)
        .unwrap()
        .checked_div(BPS_DENOMINATOR as u128)
        .unwrap() as u64
}

/// Simulate compound bounty payment and restaking
/// Returns (bounty_paid, amount_restaked)
fn simulate_compound_with_bounty(
    unstaked_received: u64,    // Principal returned from Oracle
    rewards_claimed: u64,      // Rewards claimed from Oracle
    stakeable_pending: u64,    // New deposits waiting to be staked
    bounty_bps: u64,           // Bounty rate in bps (should be 10 = 0.1%)
) -> (u64, u64) {
    let mut amount_to_stake = stakeable_pending
        .checked_add(unstaked_received)
        .unwrap()
        .checked_add(rewards_claimed)
        .unwrap();

    let mut bounty_paid = 0u64;

    // Pay keeper bounty from claimed rewards only (never from principal)
    // This mirrors compound.rs lines 335-370
    if rewards_claimed > 0 && bounty_bps > 0 {
        bounty_paid = calculate_bounty(rewards_claimed, bounty_bps);
        if bounty_paid > 0 {
            amount_to_stake = amount_to_stake.checked_sub(bounty_paid).unwrap();
        }
    }

    (bounty_paid, amount_to_stake)
}

#[test]
fn test_compound_bounty_is_10_bps_of_rewards() {
    // COMPOUND_BOUNTY_BPS = 10 (0.1%) - from constants.rs
    let bounty_bps = 10u64;

    let unstaked_principal = 1_000_000_000_000u64; // 1000 CCM principal
    let rewards_claimed = 100_000_000_000u64;      // 100 CCM rewards
    let new_deposits = 50_000_000_000u64;          // 50 CCM new

    let (bounty_paid, amount_restaked) = simulate_compound_with_bounty(
        unstaked_principal,
        rewards_claimed,
        new_deposits,
        bounty_bps,
    );

    // Bounty = 100 CCM * 10 / 10000 = 0.1 CCM = 100_000_000 lamports
    let expected_bounty = 100_000_000u64;
    assert_eq!(bounty_paid, expected_bounty, "Bounty should be 0.1% of rewards");

    // Amount restaked = 1000 + 100 + 50 - 0.1 = 1149.9 CCM
    let expected_restaked = unstaked_principal + rewards_claimed + new_deposits - bounty_paid;
    assert_eq!(amount_restaked, expected_restaked);
}

#[test]
fn test_bounty_comes_from_rewards_not_principal() {
    // Critical test: bounty is calculated from rewards_claimed only,
    // NOT from the unstaked principal or pending deposits

    let bounty_bps = 10u64;
    let unstaked_principal = 1_000_000_000_000u64; // 1000 CCM principal
    let rewards_claimed = 10_000_000_000u64;       // 10 CCM rewards (small)
    let new_deposits = 500_000_000_000u64;         // 500 CCM new deposits

    let (bounty_paid, amount_restaked) = simulate_compound_with_bounty(
        unstaked_principal,
        rewards_claimed,
        new_deposits,
        bounty_bps,
    );

    // Bounty should only be 0.1% of 10 CCM rewards = 0.01 CCM
    // NOT 0.1% of (1000 + 10 + 500) = 1.51 CCM
    let expected_bounty = 10_000_000u64; // 0.01 CCM in lamports
    assert_eq!(
        bounty_paid, expected_bounty,
        "Bounty must come from rewards only, not principal or deposits"
    );

    // Total going back to stake
    let total_input = unstaked_principal + rewards_claimed + new_deposits;
    assert_eq!(
        amount_restaked,
        total_input - bounty_paid,
        "Restaked = total input - bounty"
    );
}

#[test]
fn test_no_bounty_when_no_rewards() {
    let bounty_bps = 10u64;
    let unstaked_principal = 1_000_000_000_000u64;
    let rewards_claimed = 0u64; // No rewards
    let new_deposits = 100_000_000_000u64;

    let (bounty_paid, amount_restaked) = simulate_compound_with_bounty(
        unstaked_principal,
        rewards_claimed,
        new_deposits,
        bounty_bps,
    );

    // No rewards = no bounty
    assert_eq!(bounty_paid, 0, "No bounty when no rewards claimed");

    // All goes to restaking
    assert_eq!(
        amount_restaked,
        unstaked_principal + new_deposits,
        "All principal + deposits should be restaked"
    );
}

#[test]
fn test_no_bounty_when_zero_bps() {
    let bounty_bps = 0u64; // Bounty disabled
    let rewards_claimed = 100_000_000_000u64;

    let (bounty_paid, _) = simulate_compound_with_bounty(
        1_000_000_000_000,
        rewards_claimed,
        0,
        bounty_bps,
    );

    assert_eq!(bounty_paid, 0, "No bounty when bps is 0");
}

#[test]
fn test_bounty_small_rewards_rounding() {
    // Test with small rewards where bounty might round to 0
    let bounty_bps = 10u64;

    // 1000 lamports rewards * 10 / 10000 = 1 lamport bounty
    let small_rewards = 1_000u64;

    let (bounty_paid, _) = simulate_compound_with_bounty(
        1_000_000_000_000,
        small_rewards,
        0,
        bounty_bps,
    );

    assert_eq!(bounty_paid, 1, "Very small bounty should still be paid (1 lamport)");
}

#[test]
fn test_bounty_very_small_rewards_rounds_to_zero() {
    // Test with extremely small rewards that round to 0
    let bounty_bps = 10u64;

    // 99 lamports * 10 / 10000 = 0 (integer division)
    let tiny_rewards = 99u64;

    let bounty = calculate_bounty(tiny_rewards, bounty_bps);
    assert_eq!(bounty, 0, "Sub-threshold rewards should round bounty to 0");
}

#[test]
fn test_bounty_large_rewards() {
    let bounty_bps = 10u64;

    // 1 billion CCM in rewards (extreme case)
    let large_rewards = 1_000_000_000_000_000_000u64;

    let (bounty_paid, amount_restaked) = simulate_compound_with_bounty(
        0,             // No principal (first stake scenario)
        large_rewards,
        0,             // No pending
        bounty_bps,
    );

    // 0.1% of 1B CCM = 1M CCM
    let expected_bounty = 1_000_000_000_000_000u64;
    assert_eq!(bounty_paid, expected_bounty);
    assert_eq!(amount_restaked, large_rewards - bounty_paid);
}

#[test]
fn test_bounty_preserves_principal_invariant() {
    // Invariant: principal + new deposits should never be touched for bounty

    let bounty_bps = 10u64;
    let principal = 1_000_000_000_000u64;
    let new_deposits = 200_000_000_000u64;
    let rewards = 50_000_000_000u64;

    let (bounty_paid, amount_restaked) = simulate_compound_with_bounty(
        principal,
        rewards,
        new_deposits,
        bounty_bps,
    );

    // The sum going to Oracle should be:
    // principal + deposits + rewards - bounty
    let expected = principal + new_deposits + rewards - bounty_paid;
    assert_eq!(amount_restaked, expected);

    // Critically, the principal + deposits portion is fully preserved
    // Only the rewards portion is reduced by bounty
    let principal_and_deposits = principal + new_deposits;
    let rewards_after_bounty = rewards - bounty_paid;
    assert_eq!(
        amount_restaked,
        principal_and_deposits + rewards_after_bounty,
        "Principal and deposits must be fully preserved"
    );
}

#[test]
fn test_bounty_calculation_matches_constants() {
    // Verify our test uses the actual constant value
    // COMPOUND_BOUNTY_BPS = 10 (from constants.rs line 77)
    let bounty_bps = 10u64;

    let rewards = 10_000_000_000_000u64; // 10,000 CCM

    let bounty = calculate_bounty(rewards, bounty_bps);

    // 10,000 CCM * 0.001 = 10 CCM
    assert_eq!(bounty, 10_000_000_000u64, "10 bps = 0.1% of rewards");
}

// =========================================================================
// REWARD CLAIM PRECHECK (UNDERFUNDED GUARD)
// =========================================================================

/// Mirrors the new precheck gate for reward claims: only attempt a CPI claim
/// when the vault has enough excess (vault_balance - total_staked).
fn claim_precheck(pending: u64, vault_balance: u64, total_staked: u64) -> bool {
    if pending == 0 {
        return false;
    }
    let excess = vault_balance.saturating_sub(total_staked);
    pending <= excess
}

#[test]
fn test_claim_precheck_fails_when_underfunded() {
    let should_claim = claim_precheck(
        500_000_000,    // pending
        10_100_000_000, // vault
        10_000_000_000, // total_staked (excess = 0.1 CCM)
    );
    assert!(!should_claim, "Underfunded rewards should skip claim CPI");
}

#[test]
fn test_claim_precheck_succeeds_when_funded() {
    let should_claim = claim_precheck(
        500_000_000,    // pending
        10_600_000_000, // vault (excess = 0.6 CCM)
        10_000_000_000, // total_staked
    );
    assert!(should_claim, "Sufficient excess should allow claim CPI");
}

#[test]
fn test_claim_precheck_skips_when_no_pending() {
    let should_claim = claim_precheck(
        0,
        10_600_000_000,
        10_000_000_000,
    );
    assert!(!should_claim, "Zero pending rewards should skip claim CPI");
}

// =========================================================================
// CAPITAL INJECTION (INSOLVENCY RECOVERY) TESTS
// =========================================================================

/// Check if vault is solvent (can honor all pending withdrawals).
fn is_solvent(vault: &ChannelVault) -> bool {
    let gross = vault.total_staked
        .saturating_add(vault.pending_deposits)
        .saturating_add(vault.emergency_reserve);
    gross >= vault.pending_withdrawals
}

/// Simulate capital injection: add actual_received to pending_deposits.
fn simulate_inject_capital(vault: &mut ChannelVault, actual_received: u64) {
    vault.pending_deposits = vault.pending_deposits
        .saturating_add(actual_received);
}

#[test]
fn test_insolvency_detection() {
    let mut vault = make_vault();
    vault.total_staked = 500_000_000_000;    // 500 CCM staked
    vault.pending_deposits = 100_000_000_000; // 100 CCM pending
    vault.emergency_reserve = 50_000_000_000; // 50 CCM reserve
    vault.pending_withdrawals = 800_000_000_000; // 800 CCM queued!

    // Gross assets = 500 + 100 + 50 = 650 CCM
    // Pending withdrawals = 800 CCM
    // 650 < 800 → insolvent
    assert!(!is_solvent(&vault), "Vault should be insolvent");

    // Exchange rate should error when insolvent
    let rate_result = vault.exchange_rate();
    assert!(rate_result.is_err(), "Exchange rate should error when insolvent");
}

#[test]
fn test_capital_injection_restores_solvency() {
    let mut vault = make_vault();
    vault.total_staked = 500_000_000_000;    // 500 CCM staked
    vault.pending_deposits = 100_000_000_000; // 100 CCM pending
    vault.emergency_reserve = 50_000_000_000; // 50 CCM reserve
    vault.pending_withdrawals = 800_000_000_000; // 800 CCM queued
    vault.total_shares = 700_000_000_000;    // For rate calculation

    // Insolvent: gross 650, pending_withdrawals 800
    assert!(!is_solvent(&vault));

    // Inject 200 CCM (covers shortfall + buffer)
    // Assuming 0.5% transfer fee, actual_received = 199 CCM
    let actual_received = 199_000_000_000;
    simulate_inject_capital(&mut vault, actual_received);

    // Now: gross = 500 + 299 + 50 = 849 CCM
    // pending_withdrawals = 800 CCM
    // 849 >= 800 → solvent!
    assert!(is_solvent(&vault), "Vault should be solvent after injection");

    // Exchange rate should work now
    let rate = vault.exchange_rate().unwrap();
    assert!(rate > 0, "Exchange rate should be positive after recovery");
}

#[test]
fn test_capital_injection_does_not_mint_shares() {
    let mut vault = make_vault();
    vault.total_staked = 1_000_000_000_000;
    vault.pending_deposits = 0;
    vault.total_shares = 1_000_000_000_000;
    vault.pending_withdrawals = 1_500_000_000_000; // Insolvent!

    let shares_before = vault.total_shares;

    // Inject 600 CCM (restores solvency)
    simulate_inject_capital(&mut vault, 600_000_000_000);

    // Shares should NOT increase - injection is a gift to existing shareholders
    assert_eq!(vault.total_shares, shares_before,
        "Capital injection must not mint new shares");
}

#[test]
fn test_capital_injection_increases_share_value() {
    let mut vault = make_vault();
    vault.total_staked = 1_000_000_000_000;  // 1000 CCM
    vault.pending_deposits = 0;
    vault.total_shares = 1_000_000_000_000;  // 1000 shares
    // Solvent: rate = 1e9 (1:1)

    let rate_before = vault.exchange_rate().unwrap();
    assert_eq!(rate_before, 1_000_000_000);

    // Admin injects 100 CCM capital (no new shares minted)
    simulate_inject_capital(&mut vault, 100_000_000_000);

    // Now: NAV = 1100 CCM, shares = 1000
    // Rate = 1100/1000 = 1.1 CCM per share
    let rate_after = vault.exchange_rate().unwrap();
    assert_eq!(rate_after, 1_100_000_000, "Share value should increase after injection");
}

#[test]
fn test_capital_injection_minimum_to_restore_solvency() {
    let mut vault = make_vault();
    vault.total_staked = 400_000_000_000;    // 400 CCM
    vault.pending_deposits = 50_000_000_000;  // 50 CCM
    vault.emergency_reserve = 0;
    vault.pending_withdrawals = 500_000_000_000; // 500 CCM queued
    vault.total_shares = 450_000_000_000;

    // Shortfall: 500 - 450 = 50 CCM
    assert!(!is_solvent(&vault));

    // Inject exactly enough (50 CCM, minus 0.5% fee → 49.75 CCM received)
    // Still not enough! Need 50.25 CCM sent to receive 50 CCM
    simulate_inject_capital(&mut vault, 49_750_000_000);
    assert!(!is_solvent(&vault), "49.75 CCM not enough");

    // Inject 0.25 CCM more
    simulate_inject_capital(&mut vault, 250_000_000);
    assert!(is_solvent(&vault), "Exactly at solvency threshold");
}

#[test]
fn test_injected_capital_available_for_withdrawals() {
    let mut vault = make_vault();
    vault.total_staked = 0;                   // Nothing staked
    vault.pending_deposits = 100_000_000_000; // 100 CCM in buffer
    vault.pending_withdrawals = 150_000_000_000; // 150 CCM queued

    // Currently: 50 CCM shortfall
    assert!(!is_solvent(&vault));

    // Inject 60 CCM (covers shortfall + 10 buffer)
    simulate_inject_capital(&mut vault, 60_000_000_000);

    // Now: pending_deposits = 160 CCM, pending_withdrawals = 150 CCM
    assert!(is_solvent(&vault));

    // The 160 CCM in pending_deposits can honor the 150 CCM withdrawals
    // with 10 CCM remaining for next compound cycle
    assert_eq!(vault.pending_deposits, 160_000_000_000);
}

// =========================================================================
// EMERGENCY TIMEOUT TESTS (Oracle Unresponsive)
// =========================================================================

/// ~7 days in slots at 400ms/slot = 1,500,000 slots
const EMERGENCY_TIMEOUT_SLOTS: u64 = 1_500_000;

/// Check if Oracle is stale (no compound in EMERGENCY_TIMEOUT_SLOTS)
fn is_oracle_stale(vault: &ChannelVault, current_slot: u64) -> bool {
    let slots_since_compound = current_slot.saturating_sub(vault.last_compound_slot);
    slots_since_compound >= EMERGENCY_TIMEOUT_SLOTS
}

/// Calculate emergency withdraw payout (80% after 20% penalty)
fn calculate_emergency_payout(ccm_requested: u64) -> u64 {
    let return_bps = BPS_DENOMINATOR - EMERGENCY_PENALTY_BPS;
    (ccm_requested as u128 * return_bps as u128 / BPS_DENOMINATOR as u128) as u64
}

#[test]
fn test_oracle_staleness_detection_fresh() {
    let mut vault = make_vault();
    vault.last_compound_slot = 100_000_000;

    // Current slot is 500k slots later (~2.3 days)
    let current_slot = 100_500_000;
    assert!(!is_oracle_stale(&vault, current_slot), "Oracle should not be stale after 2.3 days");
}

#[test]
fn test_oracle_staleness_detection_stale() {
    let mut vault = make_vault();
    vault.last_compound_slot = 100_000_000;

    // Current slot is 1.5M slots later (~7 days)
    let current_slot = 100_000_000 + EMERGENCY_TIMEOUT_SLOTS;
    assert!(is_oracle_stale(&vault, current_slot), "Oracle should be stale after 7 days");
}

#[test]
fn test_oracle_staleness_detection_just_under() {
    let mut vault = make_vault();
    vault.last_compound_slot = 100_000_000;

    // Current slot is 1 slot under timeout
    let current_slot = 100_000_000 + EMERGENCY_TIMEOUT_SLOTS - 1;
    assert!(!is_oracle_stale(&vault, current_slot), "Oracle should not be stale 1 slot before timeout");
}

#[test]
fn test_emergency_timeout_payout_calculation() {
    // 1000 CCM withdrawal request
    let ccm_requested = 1_000_000_000_000u64;

    // User should receive 80% = 800 CCM
    let ccm_returned = calculate_emergency_payout(ccm_requested);
    assert_eq!(ccm_returned, 800_000_000_000u64);

    // 20% penalty = 200 CCM stays in vault buffer
    let penalty = ccm_requested - ccm_returned;
    assert_eq!(penalty, 200_000_000_000u64);
}

#[test]
fn test_emergency_timeout_small_amount() {
    // Small amount: 10 CCM
    let ccm_requested = 10_000_000_000u64;
    let ccm_returned = calculate_emergency_payout(ccm_requested);

    // 10 * 0.8 = 8 CCM
    assert_eq!(ccm_returned, 8_000_000_000u64);
}

#[test]
fn test_emergency_timeout_minimum_viable() {
    // 1 lamport request (edge case)
    let ccm_requested = 1u64;
    let ccm_returned = calculate_emergency_payout(ccm_requested);

    // 1 * 8000 / 10000 = 0 (integer division)
    assert_eq!(ccm_returned, 0u64, "Sub-lamport amounts round to 0");
}

#[test]
fn test_emergency_timeout_does_not_require_oracle() {
    // Key property: emergency timeout withdraw doesn't check Oracle state at all
    // It only checks vault.last_compound_slot vs current_slot

    let mut vault = make_vault();
    vault.last_compound_slot = 100_000_000;
    vault.total_staked = 1_000_000_000_000;  // 1000 CCM "in Oracle"
    vault.pending_deposits = 500_000_000_000; // 500 CCM in buffer
    vault.pending_withdrawals = 200_000_000_000; // 200 CCM queued
    vault.total_shares = 1_500_000_000_000;

    // Oracle goes stale
    let current_slot = 100_000_000 + EMERGENCY_TIMEOUT_SLOTS;
    assert!(is_oracle_stale(&vault, current_slot));

    // Even though total_staked is 1000 CCM, emergency withdraw only uses buffer
    // User's 200 CCM request gets 160 CCM (80%) from the 500 CCM buffer
    let withdrawal_request_amount = 200_000_000_000u64;
    let payout = calculate_emergency_payout(withdrawal_request_amount);
    assert_eq!(payout, 160_000_000_000u64);

    // Buffer (500) is enough to cover payout (160)
    assert!(vault.pending_deposits >= payout);
}

#[test]
fn test_emergency_timeout_penalty_stays_in_vault() {
    // Verify penalty accounting: penalty stays in buffer, benefits remaining shareholders

    let mut vault = make_vault();
    vault.last_compound_slot = 0;
    vault.pending_deposits = 1_000_000_000_000; // 1000 CCM in buffer
    vault.pending_withdrawals = 500_000_000_000; // 500 CCM queued
    vault.total_shares = 1_000_000_000_000;

    let current_slot = EMERGENCY_TIMEOUT_SLOTS; // Oracle is stale

    // User with 500 CCM pending withdrawal does emergency exit
    let ccm_requested = 500_000_000_000u64;
    let ccm_returned = calculate_emergency_payout(ccm_requested);
    assert_eq!(ccm_returned, 400_000_000_000u64); // 80%

    let penalty = ccm_requested - ccm_returned; // 100 CCM

    // Simulate the vault state update:
    // pending_withdrawals decreases by FULL requested (500)
    // pending_deposits decreases by RETURNED (400)
    // The 100 CCM penalty implicitly stays in buffer

    vault.pending_withdrawals -= ccm_requested;
    vault.pending_deposits -= ccm_returned;

    // Buffer started at 1000, paid out 400, left with 600
    // (penalty 100 stays, plus the remaining 500)
    assert_eq!(vault.pending_deposits, 600_000_000_000);
    assert_eq!(vault.pending_withdrawals, 0);
}
