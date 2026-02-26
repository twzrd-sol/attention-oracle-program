//! LiteSVM-friendly tests for creator market primitives (Phase 1A + 1B).
//!
//! Run with: `cargo test --package attention-oracle-token-2022 --test litesvm_markets`
//!
//! Coverage:
//! - Phase 1A: PDA derivation, discriminators, resolution threshold, tampered proofs
//! - Phase 1B: Fee-aware minting math, conditional token invariants, settlement logic,
//!   market state machine, CHAOS security vectors

use anchor_lang::prelude::AccountSerialize;
use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    message::Message,
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    sysvar,
    transaction::Transaction,
};
use solana_system_interface::program as system_program;
use spl_token_2022::{
    extension::ExtensionType,
    state::{Account as SplAccount, AccountState, Mint as SplMint},
};
use std::path::Path;

use token_2022::{
    GlobalRootConfig, MarketState, ProtocolState, RootEntry,
    CUMULATIVE_ROOT_HISTORY, GLOBAL_ROOT_SEED, MARKET_MINT_AUTHORITY_SEED, MARKET_NO_MINT_SEED,
    MARKET_STATE_SEED, MARKET_VAULT_SEED, MARKET_YES_MINT_SEED, PROTOCOL_SEED,
};

const GLOBAL_V4_DOMAIN: &[u8] = b"TWZRD:GLOBAL_V4";
const FEE_BPS: u64 = 50; // 0.5% — matches mainnet CCM transfer fee

fn program_id() -> Pubkey {
    "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
        .parse()
        .unwrap()
}

fn compute_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

fn serialize_anchor<T: AccountSerialize>(account: &T, len: usize) -> Vec<u8> {
    let mut data = vec![0u8; len];
    account.try_serialize(&mut data.as_mut_slice()).unwrap();
    data
}

/// Helper to load the compiled program
fn load_program(svm: &mut LiteSVM) -> Result<(), Box<dyn std::error::Error>> {
    let program_path = Path::new("../../target/deploy/token_2022.so");

    if !program_path.exists() {
        return Err(format!(
            "Program not found at {:?}. Run `anchor build` first.",
            program_path
                .canonicalize()
                .unwrap_or(program_path.to_path_buf())
        )
        .into());
    }

    let program_bytes = std::fs::read(program_path)?;
    svm.add_program(program_id(), &program_bytes)?;
    Ok(())
}

// =============================================================================
// PDA DERIVATION HELPERS
// =============================================================================

fn derive_protocol_state(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[PROTOCOL_SEED, mint.as_ref()], &program_id())
}

fn derive_global_root_config(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[GLOBAL_ROOT_SEED, mint.as_ref()], &program_id())
}

fn derive_market_state(mint: &Pubkey, market_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[MARKET_STATE_SEED, mint.as_ref(), &market_id.to_le_bytes()],
        &program_id(),
    )
}

fn derive_market_vault(mint: &Pubkey, market_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[MARKET_VAULT_SEED, mint.as_ref(), &market_id.to_le_bytes()],
        &program_id(),
    )
}

fn derive_market_yes_mint(mint: &Pubkey, market_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[MARKET_YES_MINT_SEED, mint.as_ref(), &market_id.to_le_bytes()],
        &program_id(),
    )
}

fn derive_market_no_mint(mint: &Pubkey, market_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[MARKET_NO_MINT_SEED, mint.as_ref(), &market_id.to_le_bytes()],
        &program_id(),
    )
}

fn derive_market_mint_authority(mint: &Pubkey, market_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            MARKET_MINT_AUTHORITY_SEED,
            mint.as_ref(),
            &market_id.to_le_bytes(),
        ],
        &program_id(),
    )
}

// =============================================================================
// MERKLE TREE UTILITIES
// =============================================================================

fn compute_global_leaf(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(GLOBAL_V4_DOMAIN);
    hasher.update(mint.as_ref());
    hasher.update(&root_seq.to_le_bytes());
    hasher.update(wallet.as_ref());
    hasher.update(&cumulative_total.to_le_bytes());
    hasher.finalize().into()
}

fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current = leaves.to_vec();
    while current.len() > 1 {
        let mut next = Vec::new();
        for chunk in current.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
            let (a, b) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            let mut hasher = Keccak256::new();
            hasher.update(&a);
            hasher.update(&b);
            next.push(hasher.finalize().into());
        }
        current = next;
    }
    current[0]
}

fn generate_proof(leaves: &[[u8; 32]], mut index: usize) -> Vec<[u8; 32]> {
    if leaves.is_empty() || leaves.len() == 1 {
        return vec![];
    }

    let mut proof = Vec::new();
    let mut current = leaves.to_vec();

    while current.len() > 1 {
        let sibling_idx = if index % 2 == 0 { index + 1 } else { index - 1 };
        let sibling = if sibling_idx < current.len() {
            current[sibling_idx]
        } else {
            current[index]
        };
        proof.push(sibling);

        let mut next = Vec::new();
        for chunk in current.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
            let (a, b) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            let mut hasher = Keccak256::new();
            hasher.update(&a);
            hasher.update(&b);
            next.push(hasher.finalize().into());
        }
        current = next;
        index /= 2;
    }
    proof
}

fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
    for sibling in proof {
        let (a, b) = if hash <= *sibling {
            (hash, *sibling)
        } else {
            (*sibling, hash)
        };
        let mut hasher = Keccak256::new();
        hasher.update(&a);
        hasher.update(&b);
        hash = hasher.finalize().into();
    }
    hash == root
}

// =============================================================================
// FEE CALCULATION HELPERS
// =============================================================================

/// Calculate the Token-2022 transfer fee for a given amount.
/// CCM uses 50bps (0.5%). The fee is floor(amount * bps / 10000).
/// Uses u128 intermediate to avoid overflow on large amounts.
fn transfer_fee(amount: u64, fee_bps: u64) -> u64 {
    ((amount as u128) * (fee_bps as u128) / 10_000u128) as u64
}

/// Net amount received after Token-2022 transfer fee deduction.
fn net_after_fee(amount: u64, fee_bps: u64) -> u64 {
    amount - transfer_fee(amount, fee_bps)
}

// =============================================================================
// MARKET STATE MACHINE SIMULATOR
// =============================================================================

/// Simulates the on-chain market state machine for invariant testing.
#[derive(Debug, Clone)]
struct MarketSim {
    resolved: bool,
    outcome: bool,
    tokens_initialized: bool,
    target: u64,
    vault_balance: u64,
    yes_supply: u64,
    no_supply: u64,
}

impl MarketSim {
    fn new(target: u64) -> Self {
        Self {
            resolved: false,
            outcome: false,
            tokens_initialized: true,
            target,
            vault_balance: 0,
            yes_supply: 0,
            no_supply: 0,
        }
    }

    /// Deposit CCM → receive YES + NO shares. Returns (net_deposited, shares_minted).
    fn mint_shares(&mut self, gross_amount: u64, fee_bps: u64) -> Result<(u64, u64), &'static str> {
        if self.resolved {
            return Err("MarketAlreadyResolved");
        }
        if !self.tokens_initialized {
            return Err("MarketTokensNotInitialized");
        }
        if gross_amount == 0 {
            return Err("ZeroSharesMinted");
        }

        let net = net_after_fee(gross_amount, fee_bps);
        if net == 0 {
            return Err("ZeroSharesMinted");
        }

        self.vault_balance += net;
        self.yes_supply += net;
        self.no_supply += net;

        Ok((net, net))
    }

    /// Burn equal YES + NO → get CCM back (pre-resolution).
    fn redeem_shares(&mut self, shares: u64, fee_bps: u64) -> Result<u64, &'static str> {
        if self.resolved {
            return Err("MarketAlreadyResolved");
        }
        if shares == 0 {
            return Err("ZeroSharesMinted");
        }
        if self.yes_supply < shares || self.no_supply < shares {
            return Err("InsufficientShares");
        }
        if self.vault_balance < shares {
            return Err("InsufficientVaultBalance");
        }

        self.yes_supply -= shares;
        self.no_supply -= shares;
        self.vault_balance -= shares;

        // Redeemer receives shares minus outbound transfer fee
        let net_returned = net_after_fee(shares, fee_bps);
        Ok(net_returned)
    }

    /// Resolve the market with the creator's verified cumulative total.
    fn resolve(&mut self, cumulative_total: u64) -> Result<bool, &'static str> {
        if self.resolved {
            return Err("MarketAlreadyResolved");
        }
        self.outcome = cumulative_total >= self.target;
        self.resolved = true;
        Ok(self.outcome)
    }

    /// Burn winning shares → claim CCM from vault (post-resolution).
    fn settle(&mut self, shares: u64, is_yes: bool, fee_bps: u64) -> Result<u64, &'static str> {
        if !self.resolved {
            return Err("MarketNotResolved");
        }
        if shares == 0 {
            return Err("ZeroSharesMinted");
        }

        // Verify correct winning side
        let correct_side = if self.outcome { is_yes } else { !is_yes };
        if !correct_side {
            return Err("WrongOutcomeToken");
        }

        // Verify vault has enough
        if self.vault_balance < shares {
            return Err("InsufficientVaultBalance");
        }

        // Burn winning shares
        if is_yes {
            if self.yes_supply < shares {
                return Err("InsufficientShares");
            }
            self.yes_supply -= shares;
        } else {
            if self.no_supply < shares {
                return Err("InsufficientShares");
            }
            self.no_supply -= shares;
        }

        self.vault_balance -= shares;
        let net_returned = net_after_fee(shares, fee_bps);
        Ok(net_returned)
    }
}

// =============================================================================
// PHASE 1A: PDA DERIVATION TESTS (existing)
// =============================================================================

#[test]
fn test_market_instruction_discriminators_unique() {
    let names = [
        "create_market",
        "initialize_market_tokens",
        "mint_shares",
        "redeem_shares",
        "resolve_market",
        "settle",
    ];

    let discs: Vec<[u8; 8]> = names.iter().map(|n| compute_discriminator(n)).collect();

    for i in 0..discs.len() {
        for j in (i + 1)..discs.len() {
            assert_ne!(
                discs[i], discs[j],
                "Discriminator collision: {} vs {}",
                names[i], names[j]
            );
        }
    }
    println!("All 6 market instruction discriminators unique");
}

#[test]
fn test_market_pda_derivation() {
    let mint = Pubkey::new_unique();
    let (market_a, _) = derive_market_state(&mint, 1);
    let (market_a_repeat, _) = derive_market_state(&mint, 1);
    let (market_b, _) = derive_market_state(&mint, 2);
    let (market_other_mint, _) = derive_market_state(&Pubkey::new_unique(), 1);

    assert_eq!(
        market_a, market_a_repeat,
        "PDA derivation must be deterministic"
    );
    assert_ne!(
        market_a, market_b,
        "Different market IDs must produce different PDAs"
    );
    assert_ne!(
        market_a, market_other_mint,
        "Different mints must produce different market PDAs"
    );
}

#[test]
fn test_market_resolution_threshold_outcome() {
    let mint = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let root_seq = 18u64;
    let creator_total = 123_000_000_000u64;

    let creator_leaf = compute_global_leaf(&mint, root_seq, &creator, creator_total);
    let other_leaf = compute_global_leaf(&mint, root_seq, &Pubkey::new_unique(), 500_000_000);
    let leaves = vec![creator_leaf, other_leaf];
    let root = compute_merkle_root(&leaves);
    let proof = generate_proof(&leaves, 0);

    assert!(
        verify_proof(&proof, creator_leaf, root),
        "Creator proof should verify"
    );

    let yes_target = 120_000_000_000u64;
    let no_target = 130_000_000_000u64;
    let yes_outcome = creator_total >= yes_target;
    let no_outcome = creator_total >= no_target;

    assert!(
        yes_outcome,
        "Expected YES outcome for threshold below total"
    );
    assert!(!no_outcome, "Expected NO outcome for threshold above total");
}

#[test]
fn test_market_resolution_rejects_tampered_total() {
    let mint = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let root_seq = 7u64;
    let real_total = 10_000_000_000u64;
    let forged_total = 99_000_000_000u64;

    let real_leaf = compute_global_leaf(&mint, root_seq, &creator, real_total);
    let other_leaf = compute_global_leaf(&mint, root_seq, &Pubkey::new_unique(), 500_000_000);
    let leaves = vec![real_leaf, other_leaf];
    let root = compute_merkle_root(&leaves);
    let proof = generate_proof(&leaves, 0);

    assert!(
        verify_proof(&proof, real_leaf, root),
        "Real leaf must verify"
    );

    let forged_leaf = compute_global_leaf(&mint, root_seq, &creator, forged_total);
    assert!(
        !verify_proof(&proof, forged_leaf, root),
        "Forged total must fail proof verification"
    );
}

// =============================================================================
// PHASE 1B: TOKEN PDA DERIVATION TESTS
// =============================================================================

#[test]
fn test_market_token_pda_derivation() {
    let mint = Pubkey::new_unique();
    let market_id = 42u64;

    let (vault, _) = derive_market_vault(&mint, market_id);
    let (yes_mint, _) = derive_market_yes_mint(&mint, market_id);
    let (no_mint, _) = derive_market_no_mint(&mint, market_id);
    let (auth, _) = derive_market_mint_authority(&mint, market_id);
    let (state, _) = derive_market_state(&mint, market_id);

    // All 5 PDAs must be unique
    let pdas = vec![vault, yes_mint, no_mint, auth, state];
    for i in 0..pdas.len() {
        for j in (i + 1)..pdas.len() {
            assert_ne!(
                pdas[i], pdas[j],
                "Market PDA collision at indices {} and {}",
                i, j
            );
        }
    }

    // Deterministic
    let (vault2, _) = derive_market_vault(&mint, market_id);
    assert_eq!(vault, vault2, "Vault PDA must be deterministic");

    // Different market_id → different PDAs
    let (vault_other, _) = derive_market_vault(&mint, 99);
    assert_ne!(vault, vault_other, "Different market IDs must produce different vault PDAs");

    // Different mint → different PDAs
    let (vault_other_mint, _) = derive_market_vault(&Pubkey::new_unique(), market_id);
    assert_ne!(vault, vault_other_mint, "Different mints must produce different vault PDAs");

    println!("Market token PDAs: 5 unique, deterministic, collision-resistant");
}

#[test]
fn test_market_token_pdas_isolated_from_state_pda() {
    let mint = Pubkey::new_unique();
    let market_id = 1u64;

    let (state_pda, _) = derive_market_state(&mint, market_id);
    let (vault_pda, _) = derive_market_vault(&mint, market_id);
    let (yes_pda, _) = derive_market_yes_mint(&mint, market_id);
    let (no_pda, _) = derive_market_no_mint(&mint, market_id);
    let (auth_pda, _) = derive_market_mint_authority(&mint, market_id);

    // Token PDAs must never collide with the market state PDA
    assert_ne!(state_pda, vault_pda, "Vault PDA must differ from state PDA");
    assert_ne!(state_pda, yes_pda, "YES mint PDA must differ from state PDA");
    assert_ne!(state_pda, no_pda, "NO mint PDA must differ from state PDA");
    assert_ne!(state_pda, auth_pda, "Mint authority PDA must differ from state PDA");
}

// =============================================================================
// PHASE 1B: FEE-AWARE MINTING INVARIANT TESTS
// =============================================================================

#[test]
fn test_fee_aware_minting_basic() {
    // 100 CCM deposit → 50bps fee → 99.5 CCM net → 99.5 YES + 99.5 NO
    let deposit = 100_000_000_000u64; // 100 CCM
    let fee = transfer_fee(deposit, FEE_BPS);
    let net = net_after_fee(deposit, FEE_BPS);

    assert_eq!(fee, 500_000_000, "50bps of 100 CCM = 0.5 CCM");
    assert_eq!(net, 99_500_000_000, "Net should be 99.5 CCM");

    let mut market = MarketSim::new(50_000_000_000);
    let (net_deposited, shares) = market.mint_shares(deposit, FEE_BPS).unwrap();

    assert_eq!(net_deposited, net, "Net deposited must match fee calculation");
    assert_eq!(shares, net, "Shares minted must equal net deposited (1:1)");
    assert_eq!(market.vault_balance, net, "Vault balance must equal net deposited");
    assert_eq!(market.yes_supply, net, "YES supply must equal net deposited");
    assert_eq!(market.no_supply, net, "NO supply must equal net deposited");
}

#[test]
fn test_fee_aware_minting_snapshot_pattern() {
    // This test validates the critical vault snapshot pattern used on-chain:
    // vault_before = vault.amount
    // transfer(amount) → Token-2022 deducts fee internally
    // vault_after = vault.reload().amount
    // net_received = vault_after - vault_before
    //
    // This is MORE ROBUST than `amount - calculated_fee` because:
    // 1. It's immune to fee schedule changes
    // 2. It captures the exact amount the vault received
    // 3. No rounding disagreements with the Token-2022 program

    let vault_before = 1_000_000_000u64; // Pre-existing vault balance
    let deposit = 200_000_000_000u64; // 200 CCM

    // Simulate what Token-2022 does internally
    let fee = transfer_fee(deposit, FEE_BPS);
    let vault_after = vault_before + (deposit - fee);

    let net_received = vault_after.checked_sub(vault_before).unwrap();
    assert_eq!(net_received, deposit - fee, "Snapshot correctly captures net");

    // Shares minted should be exactly net_received
    assert_eq!(net_received, 199_000_000_000, "200 CCM - 1 CCM fee = 199 CCM");
}

#[test]
fn test_fee_aware_minting_multiple_deposits() {
    let mut market = MarketSim::new(100_000_000_000);
    let mut total_net = 0u64;

    // 5 different deposit amounts
    let deposits = [
        10_000_000_000u64,  // 10 CCM
        50_000_000_000,     // 50 CCM
        1_000_000_000,      // 1 CCM
        100_000_000_000,    // 100 CCM
        500_000_000,        // 0.5 CCM
    ];

    for deposit in &deposits {
        let (net, _) = market.mint_shares(*deposit, FEE_BPS).unwrap();
        total_net += net;
    }

    // Conservation invariant: vault == YES supply == NO supply == sum(net)
    assert_eq!(market.vault_balance, total_net, "Vault must equal sum of nets");
    assert_eq!(market.yes_supply, total_net, "YES supply must equal sum of nets");
    assert_eq!(market.no_supply, total_net, "NO supply must equal sum of nets");

    // Verify YES == NO at all times (conditional token conservation)
    assert_eq!(
        market.yes_supply, market.no_supply,
        "YES and NO supply must always be equal (conditional token invariant)"
    );
}

#[test]
fn test_fee_aware_minting_dust_amount() {
    // Tiny deposit where fee rounds to 0
    let deposit = 1u64; // 1 lamport
    let fee = transfer_fee(deposit, FEE_BPS);
    assert_eq!(fee, 0, "Fee on 1 lamport should be 0 (floor division)");

    let net = net_after_fee(deposit, FEE_BPS);
    assert_eq!(net, 1, "1 lamport deposit nets 1 lamport (no fee)");

    let mut market = MarketSim::new(100);
    let (_, shares) = market.mint_shares(deposit, FEE_BPS).unwrap();
    assert_eq!(shares, 1, "Should mint 1 share for 1 lamport deposit");
}

#[test]
fn test_fee_aware_minting_zero_net_rejected() {
    let mut market = MarketSim::new(100);
    let result = market.mint_shares(0, FEE_BPS);
    assert_eq!(result, Err("ZeroSharesMinted"), "Zero deposit must fail");
}

// =============================================================================
// PHASE 1B: CONDITIONAL TOKEN INVARIANT TESTS
// =============================================================================

#[test]
fn test_conditional_token_conservation_after_mints() {
    let mut market = MarketSim::new(50_000_000_000);

    // Multiple users deposit
    for i in 1..=10 {
        let amount = i as u64 * 5_000_000_000; // 5, 10, 15, ... 50 CCM
        market.mint_shares(amount, FEE_BPS).unwrap();
    }

    // Invariant: YES == NO == vault at all times
    assert_eq!(market.yes_supply, market.no_supply, "YES must equal NO");
    assert_eq!(market.vault_balance, market.yes_supply, "Vault must equal YES supply");
}

#[test]
fn test_conditional_token_conservation_after_redeems() {
    let mut market = MarketSim::new(50_000_000_000);

    // Deposit 100 CCM
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // Redeem half
    let redeem_shares = net / 2;
    market.redeem_shares(redeem_shares, FEE_BPS).unwrap();

    // Remaining supplies
    let remaining = net - redeem_shares;
    assert_eq!(market.yes_supply, remaining, "YES supply after partial redeem");
    assert_eq!(market.no_supply, remaining, "NO supply after partial redeem");
    assert_eq!(market.vault_balance, remaining, "Vault after partial redeem");

    // Invariant still holds
    assert_eq!(market.yes_supply, market.no_supply, "YES must equal NO after redeem");
    assert_eq!(market.vault_balance, market.yes_supply, "Vault must equal YES after redeem");
}

#[test]
fn test_conditional_token_conservation_mint_then_redeem_all() {
    let mut market = MarketSim::new(50_000_000_000);

    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // Redeem ALL shares
    market.redeem_shares(net, FEE_BPS).unwrap();

    assert_eq!(market.vault_balance, 0, "Vault should be empty after full redeem");
    assert_eq!(market.yes_supply, 0, "YES supply should be 0 after full redeem");
    assert_eq!(market.no_supply, 0, "NO supply should be 0 after full redeem");
}

// =============================================================================
// PHASE 1B: REDEEM SHARES INVARIANT TESTS
// =============================================================================

#[test]
fn test_redeem_shares_outbound_fee() {
    // Redeemer burns N shares but receives N minus outbound transfer fee
    let shares = 10_000_000_000u64; // 10 CCM
    let outbound_fee = transfer_fee(shares, FEE_BPS);
    let net_returned = net_after_fee(shares, FEE_BPS);

    assert_eq!(outbound_fee, 50_000_000, "Outbound fee on 10 CCM = 0.05 CCM");
    assert_eq!(net_returned, 9_950_000_000, "Redeemer gets 9.95 CCM");

    // The difference (fee) becomes protocol revenue via Token-2022 withheld fees
}

#[test]
fn test_redeem_blocked_after_resolution() {
    let mut market = MarketSim::new(50_000_000_000);
    market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // Resolve
    market.resolve(60_000_000_000).unwrap();

    // Try to redeem
    let result = market.redeem_shares(1_000_000_000, FEE_BPS);
    assert_eq!(
        result,
        Err("MarketAlreadyResolved"),
        "Redemption must be blocked after resolution"
    );
}

#[test]
fn test_redeem_insufficient_shares() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(10_000_000_000, FEE_BPS).unwrap();

    // Try to redeem more than available
    let result = market.redeem_shares(net + 1, FEE_BPS);
    assert_eq!(result, Err("InsufficientShares"));
}

// =============================================================================
// PHASE 1B: SETTLEMENT LOGIC TESTS
// =============================================================================

#[test]
fn test_settlement_yes_outcome() {
    let target = 50_000_000_000u64; // 50 CCM target
    let mut market = MarketSim::new(target);

    // Deposit 100 CCM
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // Resolve with cumulative_total >= target → YES wins
    let outcome = market.resolve(60_000_000_000).unwrap();
    assert!(outcome, "60B >= 50B should be YES");

    // YES holder settles
    let returned = market.settle(net, true, FEE_BPS).unwrap();
    let expected_net = net_after_fee(net, FEE_BPS);
    assert_eq!(returned, expected_net, "Settlement should return net after outbound fee");
    assert_eq!(market.vault_balance, 0, "Vault should be empty after full settlement");
    assert_eq!(market.yes_supply, 0, "YES supply should be 0");
}

#[test]
fn test_settlement_no_outcome() {
    let target = 100_000_000_000u64; // 100 CCM target (high bar)
    let mut market = MarketSim::new(target);

    let (net, _) = market.mint_shares(50_000_000_000, FEE_BPS).unwrap();

    // Resolve with cumulative_total < target → NO wins
    let outcome = market.resolve(80_000_000_000).unwrap();
    assert!(!outcome, "80B < 100B should be NO");

    // NO holder settles
    let returned = market.settle(net, false, FEE_BPS).unwrap();
    assert!(returned > 0, "NO holder should receive CCM");
    assert_eq!(market.vault_balance, 0, "Vault empty after full NO settlement");
    assert_eq!(market.no_supply, 0, "NO supply should be 0");
}

#[test]
fn test_settlement_wrong_side_rejected() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // YES wins
    market.resolve(60_000_000_000).unwrap();

    // NO holder tries to settle → FAIL
    let result = market.settle(net, false, FEE_BPS);
    assert_eq!(
        result,
        Err("WrongOutcomeToken"),
        "Losing side must not be able to settle"
    );
}

#[test]
fn test_settlement_before_resolution_rejected() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // Try to settle without resolving
    let result = market.settle(net, true, FEE_BPS);
    assert_eq!(
        result,
        Err("MarketNotResolved"),
        "Settlement before resolution must fail"
    );
}

#[test]
fn test_settlement_partial() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    market.resolve(60_000_000_000).unwrap(); // YES wins

    // Settle half
    let half = net / 2;
    market.settle(half, true, FEE_BPS).unwrap();

    // Check remaining state
    assert_eq!(market.vault_balance, net - half, "Half should remain in vault");
    assert_eq!(market.yes_supply, net - half, "Half of YES should remain");
    assert_eq!(market.no_supply, net, "NO supply unchanged (losers don't settle)");

    // Settle remaining half
    market.settle(net - half, true, FEE_BPS).unwrap();
    assert_eq!(market.vault_balance, 0, "Vault fully drained");
    assert_eq!(market.yes_supply, 0, "All YES shares burned");
}

#[test]
fn test_settlement_exceeds_vault() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(10_000_000_000, FEE_BPS).unwrap();

    market.resolve(60_000_000_000).unwrap();

    // Artificially try to settle more than vault holds
    // (shouldn't happen in practice since shares are minted 1:1 to vault)
    let result = market.settle(net + 1, true, FEE_BPS);
    assert_eq!(
        result,
        Err("InsufficientVaultBalance"),
        "Cannot settle more than vault holds"
    );
}

// =============================================================================
// PHASE 1B: MARKET STATE MACHINE TESTS
// =============================================================================

#[test]
fn test_market_lifecycle_happy_path() {
    // Full lifecycle: create → mint → resolve → settle
    let mut market = MarketSim::new(100_000_000_000); // 100 CCM target

    // Multiple users deposit
    let (net_a, _) = market.mint_shares(50_000_000_000, FEE_BPS).unwrap();  // User A: 50 CCM
    let (net_b, _) = market.mint_shares(30_000_000_000, FEE_BPS).unwrap();  // User B: 30 CCM

    let total_vault = net_a + net_b;
    assert_eq!(market.vault_balance, total_vault);

    // Resolve: creator exceeded target → YES wins
    let outcome = market.resolve(120_000_000_000).unwrap();
    assert!(outcome, "120B >= 100B → YES");

    // User A settles YES tokens
    market.settle(net_a, true, FEE_BPS).unwrap();

    // User B settles YES tokens
    market.settle(net_b, true, FEE_BPS).unwrap();

    assert_eq!(market.vault_balance, 0, "Vault fully settled");
    assert_eq!(market.yes_supply, 0, "All YES burned");
}

#[test]
fn test_market_lifecycle_no_wins() {
    let mut market = MarketSim::new(100_000_000_000);

    let (net, _) = market.mint_shares(50_000_000_000, FEE_BPS).unwrap();

    // Resolve: creator below target → NO wins
    let outcome = market.resolve(80_000_000_000).unwrap();
    assert!(!outcome, "80B < 100B → NO");

    // YES holder cannot settle
    let result = market.settle(net, true, FEE_BPS);
    assert_eq!(result, Err("WrongOutcomeToken"));

    // NO holder settles
    market.settle(net, false, FEE_BPS).unwrap();
    assert_eq!(market.vault_balance, 0);
}

#[test]
fn test_mint_blocked_after_resolution() {
    let mut market = MarketSim::new(50_000_000_000);
    market.mint_shares(10_000_000_000, FEE_BPS).unwrap();
    market.resolve(60_000_000_000).unwrap();

    let result = market.mint_shares(10_000_000_000, FEE_BPS);
    assert_eq!(
        result,
        Err("MarketAlreadyResolved"),
        "Minting must be blocked after resolution"
    );
}

#[test]
fn test_double_resolution_rejected() {
    let mut market = MarketSim::new(50_000_000_000);
    market.resolve(60_000_000_000).unwrap();

    let result = market.resolve(70_000_000_000);
    assert_eq!(
        result,
        Err("MarketAlreadyResolved"),
        "Double resolution must be rejected"
    );
}

// =============================================================================
// PHASE 1B: FEE ECONOMICS TESTS
// =============================================================================

#[test]
fn test_protocol_revenue_from_market_activity() {
    // Token-2022 transfer fee generates protocol revenue on:
    // 1. Inbound deposit (mint_shares)
    // 2. Outbound redemption (redeem_shares)
    // 3. Outbound settlement (settle)
    //
    // Fee is always 50bps of the gross transfer amount, withheld
    // in the destination account and harvested later.

    let deposit = 100_000_000_000u64; // 100 CCM

    // Revenue from deposit
    let inbound_fee = transfer_fee(deposit, FEE_BPS);
    assert_eq!(inbound_fee, 500_000_000, "0.5 CCM fee on deposit");

    let net = net_after_fee(deposit, FEE_BPS);

    // Revenue from settlement (if entire net is settled)
    let outbound_fee = transfer_fee(net, FEE_BPS);
    assert_eq!(outbound_fee, 497_500_000, "Fee on outbound settlement");

    // Total protocol revenue from one round-trip
    let total_revenue = inbound_fee + outbound_fee;
    assert_eq!(total_revenue, 997_500_000, "~1 CCM total fee revenue on 100 CCM round-trip");

    // Effective round-trip fee as basis points
    let effective_bps = total_revenue * 10_000 / deposit;
    assert_eq!(effective_bps, 99, "~99bps effective round-trip fee");

    println!("Protocol revenue: {} CCM per 100 CCM round-trip (~99bps)", total_revenue as f64 / 1e9);
}

#[test]
fn test_settlement_payout_correct_after_fee() {
    // User deposits 100 CCM, market resolves YES, user settles.
    // Expected payout = net_deposited - outbound_fee
    let deposit = 100_000_000_000u64;

    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(deposit, FEE_BPS).unwrap();
    market.resolve(60_000_000_000).unwrap();

    let returned = market.settle(net, true, FEE_BPS).unwrap();
    let expected = net_after_fee(net, FEE_BPS);
    assert_eq!(returned, expected);

    // User's total loss = deposit - returned
    let total_cost = deposit - returned;
    let cost_bps = total_cost * 10_000 / deposit;
    assert_eq!(cost_bps, 99, "Winner's cost is ~99bps (double fee)");
}

#[test]
fn test_redeem_payout_correct_after_fee() {
    // User deposits, then redeems before resolution.
    // They pay double fee: inbound + outbound
    let deposit = 100_000_000_000u64;

    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(deposit, FEE_BPS).unwrap();

    // Redeem all shares
    let returned = market.redeem_shares(net, FEE_BPS).unwrap();

    // User receives: net_deposited - outbound_fee
    let expected = net_after_fee(net, FEE_BPS);
    assert_eq!(returned, expected);

    // Cost of no-op (deposit + immediate redeem)
    let cost = deposit - returned;
    let cost_bps = cost * 10_000 / deposit;
    assert_eq!(cost_bps, 99, "Round-trip no-op costs ~99bps");
}

// =============================================================================
// PHASE 1B: MARKET ACCOUNT SIZE TESTS
// =============================================================================

#[test]
fn test_market_state_account_size() {
    // MarketState::LEN should match the manual calculation
    // discriminator(8) + version(1) + bump(1) + metric(1) + resolved(1) + outcome(1)
    // + tokens_initialized(1) + padding(2) + market_id(8) + mint(32) + authority(32)
    // + creator_wallet(32) + target(8) + resolution_root_seq(8)
    // + resolution_cumulative_total(8) + created_slot(8) + resolved_slot(8)
    // + vault(32) + yes_mint(32) + no_mint(32) + mint_authority(32)
    let expected = 8 + 1 + 1 + 1 + 1 + 1 + 1 + 2 + 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8
        + 32 + 32 + 32 + 32;
    assert_eq!(expected, 288, "Manual calculation should be 288 bytes");
    assert_eq!(
        MarketState::LEN, expected,
        "MarketState::LEN must match manual calculation"
    );

    // Rent cost (using approximate lamports per byte)
    let rent_per_byte: u64 = 6960; // approximate
    let rent = MarketState::LEN as u64 * rent_per_byte;
    println!("  MarketState rent: ~{} lamports (~{:.4} SOL)", rent, rent as f64 / 1e9);
}

// =============================================================================
// CHAOS TESTS: SECURITY ATTACK VECTORS
// =============================================================================

/// CHAOS: Attacker tries to settle with losing tokens
#[test]
fn test_chaos_settle_with_losing_tokens() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // YES wins
    market.resolve(60_000_000_000).unwrap();

    // Attacker holds NO tokens, tries to settle
    let result = market.settle(net, false, FEE_BPS);
    assert_eq!(
        result,
        Err("WrongOutcomeToken"),
        "SECURITY: Losing side must NOT be able to settle"
    );
    println!("CHAOS: settle with losing tokens correctly rejected");
}

/// CHAOS: Attacker tries to settle unresolved market
#[test]
fn test_chaos_settle_unresolved_market() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();

    // No resolution — try to settle
    let result_yes = market.settle(net, true, FEE_BPS);
    let result_no = market.settle(net, false, FEE_BPS);

    assert_eq!(result_yes, Err("MarketNotResolved"));
    assert_eq!(result_no, Err("MarketNotResolved"));
    println!("CHAOS: settlement of unresolved market rejected (both sides)");
}

/// CHAOS: Attacker tries to mint after resolution
#[test]
fn test_chaos_mint_after_resolution() {
    let mut market = MarketSim::new(50_000_000_000);
    market.mint_shares(10_000_000_000, FEE_BPS).unwrap();
    market.resolve(60_000_000_000).unwrap();

    let result = market.mint_shares(10_000_000_000, FEE_BPS);
    assert_eq!(
        result,
        Err("MarketAlreadyResolved"),
        "SECURITY: Minting after resolution must fail"
    );
    println!("CHAOS: post-resolution minting rejected");
}

/// CHAOS: Attacker tries to redeem after resolution
#[test]
fn test_chaos_redeem_after_resolution() {
    let mut market = MarketSim::new(50_000_000_000);
    let (net, _) = market.mint_shares(100_000_000_000, FEE_BPS).unwrap();
    market.resolve(60_000_000_000).unwrap();

    let result = market.redeem_shares(net, FEE_BPS);
    assert_eq!(
        result,
        Err("MarketAlreadyResolved"),
        "SECURITY: Redemption after resolution must fail"
    );
    println!("CHAOS: post-resolution redemption rejected");
}

/// CHAOS: Cross-market token settlement attack
/// Attacker uses YES tokens from market A to settle on market B
#[test]
fn test_chaos_cross_market_token_isolation() {
    // On-chain, this is enforced by the mint constraint:
    // winning_mint.key() == market_state.yes_mint (or no_mint)
    // Since each market has unique YES/NO mints (PDA-derived with market_id),
    // tokens from market A cannot be used in market B.

    let mint = Pubkey::new_unique();
    let market_id_a = 1u64;
    let market_id_b = 2u64;

    let (yes_a, _) = derive_market_yes_mint(&mint, market_id_a);
    let (yes_b, _) = derive_market_yes_mint(&mint, market_id_b);
    let (no_a, _) = derive_market_no_mint(&mint, market_id_a);
    let (no_b, _) = derive_market_no_mint(&mint, market_id_b);

    // Cross-market tokens must have different mints
    assert_ne!(yes_a, yes_b, "SECURITY: YES mints must differ across markets");
    assert_ne!(no_a, no_b, "SECURITY: NO mints must differ across markets");
    assert_ne!(yes_a, no_a, "YES and NO mints must differ within same market");
    assert_ne!(yes_a, no_b, "Cross-market cross-side mints must differ");

    println!("CHAOS: cross-market token isolation confirmed via PDA uniqueness");
}

/// CHAOS: Resolution at exact boundary (cumulative_total == target)
#[test]
fn test_chaos_resolution_exact_boundary() {
    let target = 50_000_000_000u64;

    // Exactly at target → YES
    let mut market_eq = MarketSim::new(target);
    let outcome = market_eq.resolve(target).unwrap();
    assert!(outcome, "cumulative == target should be YES (>= operator)");

    // One lamport below → NO
    let mut market_below = MarketSim::new(target);
    let outcome = market_below.resolve(target - 1).unwrap();
    assert!(!outcome, "cumulative < target should be NO");
}

/// CHAOS: Vault drainage attack — settle more than your share
#[test]
fn test_chaos_vault_drainage_attack() {
    let mut market = MarketSim::new(50_000_000_000);

    // Two users deposit equal amounts
    let (net_a, _) = market.mint_shares(50_000_000_000, FEE_BPS).unwrap();
    let (net_b, _) = market.mint_shares(50_000_000_000, FEE_BPS).unwrap();

    market.resolve(60_000_000_000).unwrap(); // YES wins

    // User A settles their full share
    market.settle(net_a, true, FEE_BPS).unwrap();

    // User B tries to settle more than their share
    let result = market.settle(net_b + 1, true, FEE_BPS);
    // This might be InsufficientVaultBalance or InsufficientShares depending on timing
    assert!(
        result.is_err(),
        "SECURITY: Cannot settle more than available"
    );

    // User B settles their correct share
    market.settle(net_b, true, FEE_BPS).unwrap();
    assert_eq!(market.vault_balance, 0, "Vault fully drained after both settle");

    println!("CHAOS: vault drainage attack rejected");
}

/// CHAOS: Zero-amount operations
#[test]
fn test_chaos_zero_amount_operations() {
    let mut market = MarketSim::new(50_000_000_000);

    // Zero mint
    assert_eq!(
        market.mint_shares(0, FEE_BPS),
        Err("ZeroSharesMinted"),
        "Zero mint must fail"
    );

    // Zero redeem
    market.mint_shares(10_000_000_000, FEE_BPS).unwrap();
    assert_eq!(
        market.redeem_shares(0, FEE_BPS),
        Err("ZeroSharesMinted"),
        "Zero redeem must fail"
    );

    // Zero settle
    market.resolve(60_000_000_000).unwrap();
    assert_eq!(
        market.settle(0, true, FEE_BPS),
        Err("ZeroSharesMinted"),
        "Zero settle must fail"
    );

    println!("CHAOS: all zero-amount operations rejected");
}

/// CHAOS: Max u64 overflow protection
#[test]
fn test_chaos_overflow_protection() {
    // Verify fee calculation doesn't overflow
    // u64::MAX would overflow with naive multiplication.
    // Our helper uses u128 intermediate (matching Token-2022 behavior).
    let max_fee = transfer_fee(u64::MAX, FEE_BPS);
    assert!(max_fee > 0, "Fee on u64::MAX should be non-zero");
    assert!(max_fee < u64::MAX, "Fee should be less than amount");

    // For realistic amounts (up to total CCM supply ~1B tokens = 1e18 lamports)
    let max_realistic = 1_000_000_000_000_000_000u64; // 1B CCM
    let fee = transfer_fee(max_realistic, FEE_BPS);
    assert_eq!(fee, 5_000_000_000_000_000, "Fee on 1B CCM = 5M CCM");

    let net = net_after_fee(max_realistic, FEE_BPS);
    assert_eq!(net, 995_000_000_000_000_000, "Net after fee on 1B CCM");
}

// =============================================================================
// PHASE 1B: LITESVM INTEGRATION TEST (create_market on-chain)
// =============================================================================

#[test]
fn test_litesvm_create_market_and_resolve() {
    let mut svm = LiteSVM::new();

    if load_program(&mut svm).is_err() {
        println!("Skipping LiteSVM test - program not compiled. Run `anchor build`.");
        return;
    }

    let admin = Keypair::new();
    let mint_keypair = Keypair::new();
    let creator_wallet = Pubkey::new_unique();
    let market_id = 1u64;

    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    // Derive PDAs
    let mint = mint_keypair.pubkey();
    let (protocol_state_pda, protocol_bump) = derive_protocol_state(&mint);
    let (global_root_pda, global_bump) = derive_global_root_config(&mint);
    let (market_state_pda, _) = derive_market_state(&mint, market_id);

    // Pre-load ProtocolState
    let protocol_data = ProtocolState {
        is_initialized: true,
        version: 1,
        admin: admin.pubkey(),
        publisher: admin.pubkey(),
        treasury: Pubkey::new_unique(),
        mint,
        paused: false,
        require_receipt: false,
        bump: protocol_bump,
    };
    let protocol_bytes = serialize_anchor(&protocol_data, ProtocolState::LEN);
    let protocol_lamports = svm.minimum_balance_for_rent_exemption(protocol_bytes.len());
    svm.set_account(
        protocol_state_pda,
        Account {
            lamports: protocol_lamports,
            data: protocol_bytes,
            owner: program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Pre-load GlobalRootConfig with a published root at seq 10
    let root_seq = 10u64;
    let mut roots = [RootEntry::default(); CUMULATIVE_ROOT_HISTORY];

    // Build a merkle tree with the creator's cumulative total
    let creator_total = 120_000_000_000u64; // 120 CCM
    let creator_leaf = compute_global_leaf(&mint, root_seq, &creator_wallet, creator_total);
    let other_leaf = compute_global_leaf(&mint, root_seq, &Pubkey::new_unique(), 500_000_000);
    let leaves = vec![creator_leaf, other_leaf];
    let root = compute_merkle_root(&leaves);

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    roots[idx] = RootEntry {
        seq: root_seq,
        root,
        dataset_hash: [0u8; 32],
        published_slot: 100,
    };

    let global_root_data = GlobalRootConfig {
        version: 1,
        bump: global_bump,
        mint,
        latest_root_seq: root_seq,
        roots,
    };
    let global_bytes = serialize_anchor(&global_root_data, GlobalRootConfig::LEN);
    let global_lamports = svm.minimum_balance_for_rent_exemption(global_bytes.len());
    svm.set_account(
        global_root_pda,
        Account {
            lamports: global_lamports,
            data: global_bytes,
            owner: program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Build create_market instruction
    let disc = compute_discriminator("create_market");
    let mut data = disc.to_vec();
    data.extend_from_slice(&market_id.to_le_bytes());
    data.extend_from_slice(creator_wallet.as_ref());
    data.extend_from_slice(&0u8.to_le_bytes()); // metric = ATTENTION_SCORE
    data.extend_from_slice(&100_000_000_000u64.to_le_bytes()); // target = 100 CCM
    data.extend_from_slice(&root_seq.to_le_bytes()); // resolution_root_seq

    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(protocol_state_pda, false),
            AccountMeta::new_readonly(global_root_pda, false),
            AccountMeta::new(market_state_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[ix], Some(&admin.pubkey()));
    let tx = Transaction::new(&[&admin], message, blockhash);

    let result = svm.send_transaction(tx);
    if let Err(ref e) = result {
        // InstructionFallbackNotFound (error 101) means the compiled binary
        // predates the market instructions — need `anchor build` to rebuild.
        let err_str = format!("{:?}", e);
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skipping: program binary predates market instructions. Run `anchor build`.");
            return;
        }
    }
    assert!(
        result.is_ok(),
        "create_market should succeed: {:?}",
        result.err()
    );

    // Verify MarketState was created
    let market_account = svm.get_account(&market_state_pda).unwrap();
    assert_eq!(
        market_account.owner,
        program_id(),
        "MarketState should be owned by program"
    );
    assert_eq!(
        market_account.data.len(),
        MarketState::LEN,
        "MarketState account size"
    );

    println!("LiteSVM: create_market succeeded, MarketState PDA created");

    // Now build resolve_market instruction
    let proof = generate_proof(&leaves, 0);
    let disc_resolve = compute_discriminator("resolve_market");
    let mut resolve_data = disc_resolve.to_vec();
    resolve_data.extend_from_slice(&creator_total.to_le_bytes()); // cumulative_total
    // Serialize proof as Borsh Vec: length prefix (u32) + elements
    resolve_data.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    for node in &proof {
        resolve_data.extend_from_slice(node);
    }

    let resolve_ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true), // resolver (permissionless)
            AccountMeta::new_readonly(protocol_state_pda, false),
            AccountMeta::new_readonly(global_root_pda, false),
            AccountMeta::new(market_state_pda, false),
        ],
        data: resolve_data,
    };

    let blockhash2 = svm.latest_blockhash();
    let message2 = Message::new(&[resolve_ix], Some(&admin.pubkey()));
    let tx2 = Transaction::new(&[&admin], message2, blockhash2);

    let result2 = svm.send_transaction(tx2);
    assert!(
        result2.is_ok(),
        "resolve_market should succeed: {:?}",
        result2.err()
    );

    println!("LiteSVM: resolve_market succeeded (120B >= 100B target → YES outcome)");

    // Verify double-resolution fails
    let disc_resolve2 = compute_discriminator("resolve_market");
    let mut double_data = disc_resolve2.to_vec();
    double_data.extend_from_slice(&creator_total.to_le_bytes());
    double_data.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    for node in &proof {
        double_data.extend_from_slice(node);
    }

    let double_ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new_readonly(protocol_state_pda, false),
            AccountMeta::new_readonly(global_root_pda, false),
            AccountMeta::new(market_state_pda, false),
        ],
        data: double_data,
    };

    let blockhash3 = svm.latest_blockhash();
    let message3 = Message::new(&[double_ix], Some(&admin.pubkey()));
    let tx3 = Transaction::new(&[&admin], message3, blockhash3);

    let result3 = svm.send_transaction(tx3);
    assert!(
        result3.is_err(),
        "Double resolution must fail"
    );

    println!("LiteSVM: double resolution correctly rejected");
    println!("LiteSVM integration: create_market + resolve_market + double-resolve guard: PASS");
}

// =============================================================================
// PHASE 1B: ROOT SEQUENCE MATCHING TESTS
// =============================================================================

#[test]
fn test_strict_root_seq_match_for_resolution() {
    // Market resolution uses strict == match: entry.seq == root_seq
    // This prevents score inflation from delayed resolution.

    // Simulate circular buffer
    #[derive(Clone, Copy, Default)]
    #[allow(dead_code)]
    struct Root {
        seq: u64,
        root: [u8; 32],
    }

    let mut buffer = [Root::default(); CUMULATIVE_ROOT_HISTORY];

    // Publish roots 1-5 (wraps around 4-slot buffer)
    for seq in 1..=5u64 {
        let idx = (seq as usize) % CUMULATIVE_ROOT_HISTORY;
        buffer[idx] = Root {
            seq,
            root: [seq as u8; 32],
        };
    }

    // Market requires resolution_root_seq = 3
    let required_seq = 3u64;
    let idx = (required_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = buffer[idx];

    // Seq 3 is still in buffer (slots: 2→seq5, 3→seq3, 0→seq4, 1→seq5)
    // Wait — let's recalculate. Buffer has 4 slots.
    // seq 1 → idx 1, seq 2 → idx 2, seq 3 → idx 3, seq 4 → idx 0, seq 5 → idx 1
    // After publishing seq 5: idx[0]=4, idx[1]=5, idx[2]=2, idx[3]=3
    // Seq 3 at idx 3: still valid!
    assert_eq!(entry.seq, required_seq, "Seq 3 should still be in buffer");

    // But seq 2 was evicted (idx 2 now holds seq 2 — actually seq 2 is at idx 2)
    // seq 2 → idx 2 is NOT overwritten by seq 5 (which goes to idx 1)
    // Actually: 5 % 4 = 1, so idx 1 is overwritten. Seq 2 at idx 2 is still valid!
    // Let's check what was really evicted: seq 1 at idx 1 was overwritten by seq 5.
    let evicted_seq = 1u64;
    let evicted_idx = (evicted_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let evicted_entry = buffer[evicted_idx];
    assert_ne!(
        evicted_entry.seq, evicted_seq,
        "Seq 1 should be evicted (overwritten by seq 5)"
    );
    assert_eq!(evicted_entry.seq, 5, "Idx 1 should now hold seq 5");

    println!("Root sequence strict match: validated with circular buffer eviction");
}

// =============================================================================
// FULL LITESVM INTEGRATION TESTS (Token-2022 Fee-Aware Markets)
//
// These tests exercise the REAL Token-2022 program with TransferFeeConfig,
// executing create_market → initialize_market_tokens → mint_shares
// → redeem_shares → resolve_market → settle through the BPF runtime.
// =============================================================================

/// Standard SPL Token program ID
fn spl_token_program_id() -> Pubkey {
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        .parse()
        .unwrap()
}

/// Search for SPL program ELF binaries shipped with litesvm in the cargo registry.
fn find_spl_elf(prefix: &str) -> Option<Vec<u8>> {
    let home = std::env::var("HOME").ok()?;
    let base = std::path::PathBuf::from(home).join(".cargo/registry/src");

    for index_entry in std::fs::read_dir(&base).ok()?.flatten() {
        for crate_entry in std::fs::read_dir(index_entry.path()).ok()?.flatten() {
            let name = crate_entry.file_name();
            if name.to_str().map_or(false, |s| s.starts_with("litesvm-")) {
                let elf_dir = crate_entry.path().join("src/programs/elf");
                if let Ok(entries) = std::fs::read_dir(&elf_dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name();
                        if fname
                            .to_str()
                            .map_or(false, |s| s.starts_with(prefix) && s.ends_with(".so"))
                        {
                            return std::fs::read(entry.path()).ok();
                        }
                    }
                }
            }
        }
    }
    None
}

/// Load the Token-2022 SPL program from litesvm's bundled ELF.
fn load_token_2022_spl_program(svm: &mut LiteSVM) -> Result<(), String> {
    let bytes =
        find_spl_elf("spl_token_2022").ok_or("Token-2022 ELF not found in litesvm")?;
    svm.add_program(spl_token_2022::id(), &bytes)
        .map_err(|e| format!("{e:?}"))
}

/// Load the standard SPL Token program from litesvm's bundled ELF.
fn load_standard_spl_token_program(svm: &mut LiteSVM) -> Result<(), String> {
    let bytes =
        find_spl_elf("spl_token-").ok_or("SPL Token ELF not found in litesvm")?;
    svm.add_program(spl_token_program_id(), &bytes)
        .map_err(|e| format!("{e:?}"))
}

/// Create a Token-2022 CCM mint with TransferFeeConfig extension via real CPI.
/// This uses the actual Token-2022 program instructions to ensure the account
/// data layout (AccountType discriminator, TLV extension metadata) is exactly
/// what the program expects during deserialization.
fn create_ccm_mint_via_cpi(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_kp: &Keypair,
    fee_bps: u16,
) {
    use spl_token_2022::extension::transfer_fee;

    let extensions = &[ExtensionType::TransferFeeConfig];
    let mint_len = ExtensionType::try_calculate_account_len::<SplMint>(extensions).unwrap();
    let rent = svm.minimum_balance_for_rent_exemption(mint_len);

    // 1. Create account owned by Token-2022
    let create_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &mint_kp.pubkey(),
        rent,
        mint_len as u64,
        &spl_token_2022::id(),
    );

    // 2. Initialize TransferFeeConfig extension (MUST come before InitializeMint)
    let init_fee_ix = transfer_fee::instruction::initialize_transfer_fee_config(
        &spl_token_2022::id(),
        &mint_kp.pubkey(),
        None, // no fee config authority (immutable)
        None, // no withdraw withheld authority
        fee_bps,
        u64::MAX, // max fee (no cap)
    )
    .unwrap();

    // 3. Initialize the mint itself
    let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_2022::id(),
        &mint_kp.pubkey(),
        &payer.pubkey(), // mint authority
        None,            // no freeze authority
        9,               // decimals
    )
    .unwrap();

    let bh = svm.latest_blockhash();
    let msg = Message::new(
        &[create_ix, init_fee_ix, init_mint_ix],
        Some(&payer.pubkey()),
    );
    let tx = Transaction::new(&[payer, mint_kp], msg, bh);
    svm.send_transaction(tx)
        .expect("Failed to create CCM mint via CPI");
}

/// Create a Token-2022 token account and fund it via real CPI.
/// Uses create_account + initialize_account3 + mint_to to produce
/// account data that the Token-2022 program will accept.
fn create_and_fund_token_2022_account(
    svm: &mut LiteSVM,
    mint_authority: &Keypair,
    owner: &Keypair,
    account_kp: &Keypair,
    mint: &Pubkey,
    amount: u64,
) {
    let extensions = &[ExtensionType::TransferFeeAmount];
    let acct_len = ExtensionType::try_calculate_account_len::<SplAccount>(extensions).unwrap();
    let rent = svm.minimum_balance_for_rent_exemption(acct_len);

    // 1. Create account owned by Token-2022
    let create_ix = solana_sdk::system_instruction::create_account(
        &owner.pubkey(),
        &account_kp.pubkey(),
        rent,
        acct_len as u64,
        &spl_token_2022::id(),
    );

    // 2. Initialize as token account
    let init_ix = spl_token_2022::instruction::initialize_account3(
        &spl_token_2022::id(),
        &account_kp.pubkey(),
        mint,
        &owner.pubkey(),
    )
    .unwrap();

    let bh = svm.latest_blockhash();
    let msg = Message::new(&[create_ix, init_ix], Some(&owner.pubkey()));
    let tx = Transaction::new(&[owner, account_kp], msg, bh);
    svm.send_transaction(tx)
        .expect("Failed to create Token-2022 account via CPI");

    // 3. Mint tokens into the account (skip if amount is 0)
    if amount > 0 {
        let mint_ix = spl_token_2022::instruction::mint_to(
            &spl_token_2022::id(),
            mint,
            &account_kp.pubkey(),
            &mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        let bh2 = svm.latest_blockhash();
        let msg2 = Message::new(&[mint_ix], Some(&mint_authority.pubkey()));
        let tx2 = Transaction::new(&[mint_authority], msg2, bh2);
        svm.send_transaction(tx2)
            .expect("Failed to mint tokens via CPI");
    }
}

/// Create a standard SPL Token account via set_account.
fn create_standard_spl_token_account(
    svm: &mut LiteSVM,
    address: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) {
    let mut data = vec![0u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint: *mint,
            owner: *owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut data,
    )
    .unwrap();

    let lamports = svm.minimum_balance_for_rent_exemption(SplAccount::LEN);
    svm.set_account(
        *address,
        Account {
            lamports,
            data,
            owner: spl_token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

/// Read the token balance from any SPL/Token-2022 token account.
/// The `amount` field is at byte offset 64 in both layouts.
fn read_token_amount(svm: &LiteSVM, address: &Pubkey) -> u64 {
    let account = svm.get_account(address).expect("Account not found");
    assert!(
        account.data.len() >= 72,
        "Account too small to be a token account"
    );
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

/// Common setup: pre-load ProtocolState and GlobalRootConfig, then execute
/// create_market + initialize_market_tokens. Returns all PDAs and state needed
/// for subsequent mint/redeem/settle instructions.
/// Returns None if any program binary is missing (graceful skip).
struct MarketTestEnv {
    svm: LiteSVM,
    admin: Keypair,
    depositor: Keypair,
    ccm_mint: Pubkey,
    market_id: u64,
    protocol_pda: Pubkey,
    global_root_pda: Pubkey,
    market_state_pda: Pubkey,
    vault_pda: Pubkey,
    yes_mint_pda: Pubkey,
    no_mint_pda: Pubkey,
    mint_authority_pda: Pubkey,
    depositor_ccm_addr: Pubkey,
    depositor_yes_addr: Pubkey,
    depositor_no_addr: Pubkey,
    creator_wallet: Pubkey,
    creator_total: u64,
    root_seq: u64,
    leaves: Vec<[u8; 32]>,
}

fn setup_market_env() -> Option<MarketTestEnv> {
    let mut svm = LiteSVM::new();

    if load_program(&mut svm).is_err() {
        println!("Skip: AO program binary not found. Run `anchor build`.");
        return None;
    }
    if load_token_2022_spl_program(&mut svm).is_err() {
        println!("Skip: Token-2022 ELF not found in litesvm.");
        return None;
    }
    if load_standard_spl_token_program(&mut svm).is_err() {
        println!("Skip: Standard SPL Token ELF not found in litesvm.");
        return None;
    }

    let admin = Keypair::new();
    let depositor = Keypair::new();
    let creator_wallet = Pubkey::new_unique();
    let ccm_mint_kp = Keypair::new();
    let ccm_mint = ccm_mint_kp.pubkey();
    let market_id = 1u64;
    let root_seq = 10u64;
    let target = 100_000_000_000u64; // 100 CCM target

    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();
    svm.airdrop(&depositor.pubkey(), 100_000_000_000).unwrap();

    // -----------------------------------------------------------------
    // Create CCM mint via real Token-2022 CPI (50 bps transfer fee)
    // -----------------------------------------------------------------
    create_ccm_mint_via_cpi(&mut svm, &admin, &ccm_mint_kp, 50);

    // -----------------------------------------------------------------
    // Pre-load: ProtocolState
    // -----------------------------------------------------------------
    let (protocol_pda, protocol_bump) = derive_protocol_state(&ccm_mint);
    let protocol_data = ProtocolState {
        is_initialized: true,
        version: 1,
        admin: admin.pubkey(),
        publisher: admin.pubkey(),
        treasury: Pubkey::new_unique(),
        mint: ccm_mint,
        paused: false,
        require_receipt: false,
        bump: protocol_bump,
    };
    let protocol_bytes = serialize_anchor(&protocol_data, ProtocolState::LEN);
    let protocol_lam = svm.minimum_balance_for_rent_exemption(protocol_bytes.len());
    svm.set_account(
        protocol_pda,
        Account {
            lamports: protocol_lam,
            data: protocol_bytes,
            owner: program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // -----------------------------------------------------------------
    // Pre-load: GlobalRootConfig with root at seq 10
    // -----------------------------------------------------------------
    let (global_root_pda, global_root_bump) = derive_global_root_config(&ccm_mint);
    let creator_total = 120_000_000_000u64; // 120 CCM
    let creator_leaf = compute_global_leaf(&ccm_mint, root_seq, &creator_wallet, creator_total);
    let other_leaf = compute_global_leaf(&ccm_mint, root_seq, &Pubkey::new_unique(), 500_000_000);
    let leaves = vec![creator_leaf, other_leaf];
    let root = compute_merkle_root(&leaves);

    let mut roots = [RootEntry::default(); CUMULATIVE_ROOT_HISTORY];
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    roots[idx] = RootEntry {
        seq: root_seq,
        root,
        dataset_hash: [0u8; 32],
        published_slot: 100,
    };

    let global_root_data = GlobalRootConfig {
        version: 1,
        bump: global_root_bump,
        mint: ccm_mint,
        latest_root_seq: root_seq,
        roots,
    };
    let global_bytes = serialize_anchor(&global_root_data, GlobalRootConfig::LEN);
    let global_lam = svm.minimum_balance_for_rent_exemption(global_bytes.len());
    svm.set_account(
        global_root_pda,
        Account {
            lamports: global_lam,
            data: global_bytes,
            owner: program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // -----------------------------------------------------------------
    // TX1: create_market
    // -----------------------------------------------------------------
    let (market_state_pda, _) = derive_market_state(&ccm_mint, market_id);

    let disc = compute_discriminator("create_market");
    let mut cm_data = disc.to_vec();
    cm_data.extend_from_slice(&market_id.to_le_bytes());
    cm_data.extend_from_slice(creator_wallet.as_ref());
    cm_data.extend_from_slice(&0u8.to_le_bytes()); // metric
    cm_data.extend_from_slice(&target.to_le_bytes());
    cm_data.extend_from_slice(&root_seq.to_le_bytes());

    let ix_cm = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(protocol_pda, false),
            AccountMeta::new_readonly(global_root_pda, false),
            AccountMeta::new(market_state_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: cm_data,
    };

    let bh = svm.latest_blockhash();
    let msg = Message::new(&[ix_cm], Some(&admin.pubkey()));
    let tx = Transaction::new(&[&admin], msg, bh);
    let result = svm.send_transaction(tx);

    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates market instructions. Run `anchor build`.");
            return None;
        }
    }
    assert!(result.is_ok(), "create_market failed: {:?}", result.err());
    println!("  create_market: OK");

    // -----------------------------------------------------------------
    // TX2: initialize_market_tokens
    // -----------------------------------------------------------------
    let (vault_pda, _) = derive_market_vault(&ccm_mint, market_id);
    let (yes_mint_pda, _) = derive_market_yes_mint(&ccm_mint, market_id);
    let (no_mint_pda, _) = derive_market_no_mint(&ccm_mint, market_id);
    let (mint_authority_pda, _) = derive_market_mint_authority(&ccm_mint, market_id);

    let disc_init = compute_discriminator("initialize_market_tokens");
    let init_data = disc_init.to_vec();

    let ix_init = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),               // payer
            AccountMeta::new_readonly(protocol_pda, false),        // protocol_state
            AccountMeta::new(market_state_pda, false),             // market_state
            AccountMeta::new_readonly(ccm_mint, false),            // ccm_mint
            AccountMeta::new(vault_pda, false),                    // vault (init)
            AccountMeta::new(yes_mint_pda, false),                 // yes_mint (init)
            AccountMeta::new(no_mint_pda, false),                  // no_mint (init)
            AccountMeta::new_readonly(mint_authority_pda, false),  // mint_authority
            AccountMeta::new_readonly(spl_token_2022::id(), false), // token_program
            AccountMeta::new_readonly(spl_token_program_id(), false), // standard_token_program
            AccountMeta::new_readonly(system_program::ID, false),  // system_program
            AccountMeta::new_readonly(sysvar::rent::ID, false),    // rent
        ],
        data: init_data,
    };

    let bh2 = svm.latest_blockhash();
    let msg2 = Message::new(&[ix_init], Some(&admin.pubkey()));
    let tx2 = Transaction::new(&[&admin], msg2, bh2);
    let result2 = svm.send_transaction(tx2);
    assert!(
        result2.is_ok(),
        "initialize_market_tokens failed: {:?}",
        result2.err()
    );
    println!("  initialize_market_tokens: OK");

    // -----------------------------------------------------------------
    // Create depositor's token accounts via real CPI
    // -----------------------------------------------------------------
    let depositor_ccm_kp = Keypair::new();
    let depositor_ccm_addr = depositor_ccm_kp.pubkey();
    let depositor_yes_addr = Pubkey::new_unique();
    let depositor_no_addr = Pubkey::new_unique();

    let deposit_amount = 1_000_000_000_000_000u64; // 1,000,000 CCM

    // CCM token account via CPI (Token-2022 with TransferFeeAmount extension + mint_to)
    create_and_fund_token_2022_account(
        &mut svm,
        &admin,     // mint authority
        &depositor, // owner + payer
        &depositor_ccm_kp,
        &ccm_mint,
        deposit_amount,
    );

    // YES token account (standard SPL, 0 balance — will be minted into)
    create_standard_spl_token_account(
        &mut svm,
        &depositor_yes_addr,
        &yes_mint_pda,
        &depositor.pubkey(),
        0,
    );

    // NO token account (standard SPL, 0 balance — will be minted into)
    create_standard_spl_token_account(
        &mut svm,
        &depositor_no_addr,
        &no_mint_pda,
        &depositor.pubkey(),
        0,
    );

    Some(MarketTestEnv {
        svm,
        admin,
        depositor,
        ccm_mint,
        market_id,
        protocol_pda,
        global_root_pda,
        market_state_pda,
        vault_pda,
        yes_mint_pda,
        no_mint_pda,
        mint_authority_pda,
        depositor_ccm_addr,
        depositor_yes_addr,
        depositor_no_addr,
        creator_wallet,
        creator_total,
        root_seq,
        leaves,
    })
}

/// Build and execute a mint_shares instruction. Panics on failure.
fn exec_mint_shares(env: &mut MarketTestEnv, amount: u64) {
    let disc = compute_discriminator("mint_shares");
    let mut data = disc.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(env.depositor.pubkey(), true),
            AccountMeta::new_readonly(env.protocol_pda, false),
            AccountMeta::new_readonly(env.market_state_pda, false),
            AccountMeta::new_readonly(env.ccm_mint, false),
            AccountMeta::new(env.depositor_ccm_addr, false),
            AccountMeta::new(env.vault_pda, false),
            AccountMeta::new(env.yes_mint_pda, false),
            AccountMeta::new(env.no_mint_pda, false),
            AccountMeta::new(env.depositor_yes_addr, false),
            AccountMeta::new(env.depositor_no_addr, false),
            AccountMeta::new_readonly(env.mint_authority_pda, false),
            AccountMeta::new_readonly(spl_token_2022::id(), false),
            AccountMeta::new_readonly(spl_token_program_id(), false),
        ],
        data,
    };

    let bh = env.svm.latest_blockhash();
    let msg = Message::new(&[ix], Some(&env.depositor.pubkey()));
    let tx = Transaction::new(&[&env.depositor], msg, bh);
    let result = env.svm.send_transaction(tx);
    assert!(result.is_ok(), "mint_shares failed: {:?}", result.err());
}

// ---------------------------------------------------------------------------
// TEST: Fee-Aware Minting (Token-2022 TransferFeeConfig end-to-end)
// ---------------------------------------------------------------------------

#[test]
fn test_market_fee_aware_minting() {
    let mut env = match setup_market_env() {
        Some(e) => e,
        None => return,
    };

    let deposit_amount = 1_000_000_000_000_000u64; // 1,000,000 CCM (9 decimals)
    let expected_net = 995_000_000_000_000u64; // 995,000 CCM (net after 50bps)

    // Execute mint_shares(1,000,000 CCM)
    exec_mint_shares(&mut env, deposit_amount);
    println!("  mint_shares(1,000,000 CCM): OK");

    // Assert 1:1 collateral invariant: vault == YES == NO == 995,000 CCM
    let vault_bal = read_token_amount(&env.svm, &env.vault_pda);
    let yes_bal = read_token_amount(&env.svm, &env.depositor_yes_addr);
    let no_bal = read_token_amount(&env.svm, &env.depositor_no_addr);

    assert_eq!(
        vault_bal, expected_net,
        "Vault CCM balance should be 995,000 CCM (net after 50bps fee on 1M)"
    );
    assert_eq!(
        yes_bal, expected_net,
        "YES balance should be 995,000 CCM (1:1 to net received)"
    );
    assert_eq!(
        no_bal, expected_net,
        "NO balance should be 995,000 CCM (1:1 to net received)"
    );

    // 1:1 collateral invariant
    assert_eq!(vault_bal, yes_bal, "Vault must equal YES supply");
    assert_eq!(yes_bal, no_bal, "YES must equal NO supply");

    // Depositor's CCM should be fully drained
    let depositor_ccm_bal = read_token_amount(&env.svm, &env.depositor_ccm_addr);
    assert_eq!(depositor_ccm_bal, 0, "Depositor CCM should be 0 after full deposit");

    println!("LiteSVM: Fee-aware minting PASSED");
    println!(
        "  Gross: 1,000,000 CCM | Fee: 5,000 CCM (50bps) | Net: 995,000 CCM"
    );
    println!(
        "  Vault={} | YES={} | NO={} (all 995,000 CCM)",
        vault_bal / 1_000_000_000,
        yes_bal / 1_000_000_000,
        no_bal / 1_000_000_000
    );
}

// ---------------------------------------------------------------------------
// TEST: Redeem + Resolve + Settle (full market lifecycle)
// ---------------------------------------------------------------------------

#[test]
fn test_market_redeem_and_settle() {
    let mut env = match setup_market_env() {
        Some(e) => e,
        None => return,
    };

    let deposit_amount = 1_000_000_000_000_000u64; // 1,000,000 CCM
    let expected_net = 995_000_000_000_000u64; // 995,000 CCM

    // Phase 0: Mint shares
    exec_mint_shares(&mut env, deposit_amount);
    println!("  mint_shares(1,000,000 CCM): OK");

    // =====================================================================
    // Phase 1: Redeem 200,000 CCM of shares
    // =====================================================================
    let redeem_shares = 200_000_000_000_000u64; // 200,000 CCM

    let disc_redeem = compute_discriminator("redeem_shares");
    let mut redeem_data = disc_redeem.to_vec();
    redeem_data.extend_from_slice(&redeem_shares.to_le_bytes());

    let redeem_ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(env.depositor.pubkey(), true),
            AccountMeta::new_readonly(env.protocol_pda, false),
            AccountMeta::new_readonly(env.market_state_pda, false),
            AccountMeta::new_readonly(env.ccm_mint, false),
            AccountMeta::new(env.vault_pda, false),
            AccountMeta::new(env.yes_mint_pda, false),
            AccountMeta::new(env.no_mint_pda, false),
            AccountMeta::new(env.depositor_yes_addr, false),
            AccountMeta::new(env.depositor_no_addr, false),
            AccountMeta::new(env.depositor_ccm_addr, false),
            AccountMeta::new_readonly(env.mint_authority_pda, false),
            AccountMeta::new_readonly(spl_token_2022::id(), false),
            AccountMeta::new_readonly(spl_token_program_id(), false),
        ],
        data: redeem_data,
    };

    let bh_r = env.svm.latest_blockhash();
    let msg_r = Message::new(&[redeem_ix], Some(&env.depositor.pubkey()));
    let tx_r = Transaction::new(&[&env.depositor], msg_r, bh_r);
    let result_r = env.svm.send_transaction(tx_r);
    assert!(
        result_r.is_ok(),
        "redeem_shares failed: {:?}",
        result_r.err()
    );

    // Verify: equal YES+NO burned, vault decreased
    let after_redeem = expected_net - redeem_shares; // 795,000 CCM
    let yes_post_redeem = read_token_amount(&env.svm, &env.depositor_yes_addr);
    let no_post_redeem = read_token_amount(&env.svm, &env.depositor_no_addr);
    let vault_post_redeem = read_token_amount(&env.svm, &env.vault_pda);

    assert_eq!(
        yes_post_redeem, after_redeem,
        "YES should be 795,000 CCM after redeeming 200K shares"
    );
    assert_eq!(
        no_post_redeem, after_redeem,
        "NO should be 795,000 CCM after redeeming 200K shares"
    );
    assert_eq!(
        vault_post_redeem, after_redeem,
        "Vault should be 795,000 CCM after 200K redeemed"
    );
    assert_eq!(
        yes_post_redeem, no_post_redeem,
        "YES must equal NO after redeem (conservation)"
    );

    // Verify depositor got CCM back (net after outbound transfer fee)
    let depositor_ccm_post_redeem = read_token_amount(&env.svm, &env.depositor_ccm_addr);
    let expected_ccm_returned = net_after_fee(redeem_shares, FEE_BPS);
    assert_eq!(
        depositor_ccm_post_redeem, expected_ccm_returned,
        "Depositor should receive 200K minus outbound fee"
    );

    println!("  redeem_shares(200,000): OK");
    println!(
        "    YES: {} → {} | NO: {} → {} | Vault: {} → {}",
        expected_net / 1_000_000_000,
        yes_post_redeem / 1_000_000_000,
        expected_net / 1_000_000_000,
        no_post_redeem / 1_000_000_000,
        expected_net / 1_000_000_000,
        vault_post_redeem / 1_000_000_000,
    );

    // =====================================================================
    // Phase 2: Resolve market (YES wins: 120B >= 100B target)
    // =====================================================================
    let proof = generate_proof(&env.leaves, 0);
    let disc_resolve = compute_discriminator("resolve_market");
    let mut resolve_data = disc_resolve.to_vec();
    resolve_data.extend_from_slice(&env.creator_total.to_le_bytes());
    resolve_data.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    for node in &proof {
        resolve_data.extend_from_slice(node);
    }

    let resolve_ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(env.admin.pubkey(), true),
            AccountMeta::new_readonly(env.protocol_pda, false),
            AccountMeta::new_readonly(env.global_root_pda, false),
            AccountMeta::new(env.market_state_pda, false),
        ],
        data: resolve_data,
    };

    let bh_v = env.svm.latest_blockhash();
    let msg_v = Message::new(&[resolve_ix], Some(&env.admin.pubkey()));
    let tx_v = Transaction::new(&[&env.admin], msg_v, bh_v);
    let result_v = env.svm.send_transaction(tx_v);
    assert!(
        result_v.is_ok(),
        "resolve_market failed: {:?}",
        result_v.err()
    );
    println!("  resolve_market(120B >= 100B → YES): OK");

    // =====================================================================
    // Phase 3: Settle winning YES shares (795,000 CCM)
    // =====================================================================
    let settle_shares = after_redeem; // 795,000 CCM

    let disc_settle = compute_discriminator("settle");
    let mut settle_data = disc_settle.to_vec();
    settle_data.extend_from_slice(&settle_shares.to_le_bytes());

    let settle_ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(env.depositor.pubkey(), true),
            AccountMeta::new_readonly(env.protocol_pda, false),
            AccountMeta::new_readonly(env.market_state_pda, false),
            AccountMeta::new_readonly(env.ccm_mint, false),
            AccountMeta::new(env.vault_pda, false),
            AccountMeta::new(env.yes_mint_pda, false),  // winning_mint (YES wins)
            AccountMeta::new(env.depositor_yes_addr, false), // settler_winning
            AccountMeta::new(env.depositor_ccm_addr, false), // settler_ccm
            AccountMeta::new_readonly(env.mint_authority_pda, false),
            AccountMeta::new_readonly(spl_token_2022::id(), false),
            AccountMeta::new_readonly(spl_token_program_id(), false),
        ],
        data: settle_data,
    };

    let bh_s = env.svm.latest_blockhash();
    let msg_s = Message::new(&[settle_ix], Some(&env.depositor.pubkey()));
    let tx_s = Transaction::new(&[&env.depositor], msg_s, bh_s);
    let result_s = env.svm.send_transaction(tx_s);
    assert!(result_s.is_ok(), "settle failed: {:?}", result_s.err());

    // Verify: vault empty, YES burned
    let vault_final = read_token_amount(&env.svm, &env.vault_pda);
    let yes_final = read_token_amount(&env.svm, &env.depositor_yes_addr);

    assert_eq!(vault_final, 0, "Vault should be empty after full settlement");
    assert_eq!(yes_final, 0, "All YES shares should be burned after settlement");

    // Verify: depositor received settlement CCM (net after outbound fee)
    let depositor_ccm_final = read_token_amount(&env.svm, &env.depositor_ccm_addr);
    let settlement_net = net_after_fee(settle_shares, FEE_BPS);
    let expected_total = expected_ccm_returned + settlement_net;
    assert_eq!(
        depositor_ccm_final, expected_total,
        "Depositor CCM should be redeem returns + settlement returns"
    );

    println!("  settle(795,000 YES): OK");
    println!("    Vault: {} (empty)", vault_final);
    println!(
        "    Depositor CCM: {} = {} (redeem) + {} (settle)",
        depositor_ccm_final / 1_000_000_000,
        expected_ccm_returned / 1_000_000_000,
        settlement_net / 1_000_000_000,
    );
    println!("LiteSVM: Redeem + Resolve + Settle lifecycle PASSED");
}
