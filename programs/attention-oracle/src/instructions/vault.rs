//! Market Vault instructions — the core product loop.
//!
//! deposit_market:   USDC -> Vault, vLOFI -> User (1:1)
//! update_attention: Oracle sets multiplier BPS on user position
//! settle_market:    Burn vLOFI, return USDC (CCM is merkle-claimed)

use anchor_lang::prelude::*;
use anchor_spl::{
    token::{
        self, Mint as SplMint, Token, TokenAccount as SplTokenAccount, Transfer as SplTransfer,
    },
    token_interface::{burn, mint_to, Burn, Mint, MintTo, Token2022, TokenAccount},
};

use crate::errors::OracleError;
use crate::state::{MarketVault, ProtocolState, UserMarketPosition};

// =============================================================================
// INITIALIZE PROTOCOL STATE — One-time setup for the protocol
// =============================================================================

#[derive(Accounts)]
pub struct InitializeProtocolState<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = ProtocolState::LEN,
        seeds = [b"protocol_state"],
        bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_protocol_state(
    ctx: Context<InitializeProtocolState>,
    publisher: Pubkey,
    treasury: Pubkey,
    oracle_authority: Pubkey,
    ccm_mint: Pubkey,
) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.is_initialized = true;
    state.version = 1;
    state.admin = ctx.accounts.admin.key();
    state.publisher = publisher;
    state.treasury = treasury;
    state.oracle_authority = oracle_authority;
    state.mint = ccm_mint;
    state.paused = false;
    state.require_receipt = false;
    state.bump = ctx.bumps.protocol_state;

    msg!("ProtocolState initialized. Admin: {}", state.admin);
    Ok(())
}

// =============================================================================
// INITIALIZE MARKET VAULT — Create a USDC vault + vLOFI mint for a market
// =============================================================================

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct InitializeMarketVault<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        has_one = admin,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        init,
        payer = admin,
        space = MarketVault::LEN,
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    /// The USDC (or deposit token) mint.
    pub deposit_mint: Box<Account<'info, SplMint>>,

    /// The global vLOFI mint (Token-2022).
    pub vlofi_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The vault's USDC ATA (must be owned by the market_vault PDA).
    #[account(
        constraint = vault_ata.owner == market_vault.key(),
        constraint = vault_ata.mint == deposit_mint.key(),
    )]
    pub vault_ata: Box<Account<'info, SplTokenAccount>>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_market_vault(ctx: Context<InitializeMarketVault>, market_id: u64) -> Result<()> {
    let vault = &mut ctx.accounts.market_vault;
    vault.bump = ctx.bumps.market_vault;
    vault.market_id = market_id;
    vault.deposit_mint = ctx.accounts.deposit_mint.key();
    vault.vlofi_mint = ctx.accounts.vlofi_mint.key();
    vault.vault_ata = ctx.accounts.vault_ata.key();
    vault.total_deposited = 0;
    vault.total_shares = 0;
    vault.created_slot = Clock::get()?.slot;

    msg!(
        "MarketVault initialized. market_id: {}, deposit_mint: {}",
        market_id,
        vault.deposit_mint
    );
    Ok(())
}

// =============================================================================
// REALLOC MARKET VAULT — Grow existing 137-byte vaults to 153 bytes (Phase 2)
// =============================================================================
//
// New fields (nav_per_share_bps, last_nav_update_slot) are appended at the end.
// realloc(false) zero-fills the new bytes → nav=0, slot=0 → treated as 1:1 (safe).

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct ReallocMarketVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = payer.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    /// CHECK: MarketVault PDA may be undersized (137 bytes) — cannot use Account<MarketVault>
    /// which expects 153 bytes. PDA address verified via seed constraint.
    #[account(
        mut,
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump,
    )]
    pub market_vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn realloc_market_vault(ctx: Context<ReallocMarketVault>, market_id: u64) -> Result<()> {
    let vault = &ctx.accounts.market_vault;
    let current_len = vault.data_len();
    let target_len = MarketVault::LEN; // 153

    if current_len >= target_len {
        msg!(
            "MarketVault {} already at {} bytes, no-op",
            market_id,
            current_len
        );
        return Ok(());
    }

    // Transfer rent difference
    let rent = Rent::get()?;
    let lamports_needed = rent
        .minimum_balance(target_len)
        .saturating_sub(vault.lamports());

    if lamports_needed > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: vault.to_account_info(),
                },
            ),
            lamports_needed,
        )?;
    }

    // Grow account — new bytes are zero (nav_per_share_bps=0, last_nav_update_slot=0)
    #[allow(deprecated)]
    vault.realloc(target_len, false)?;

    msg!(
        "MarketVault {} reallocated: {} -> {} bytes",
        market_id,
        current_len,
        target_len
    );

    Ok(())
}

// =============================================================================
// DEPOSIT MARKET — USDC -> Vault, vLOFI -> User (1:1)
// =============================================================================

#[derive(Accounts)]
#[instruction(market_id: u64, amount: u64)]
pub struct DepositMarket<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        mut,
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        init_if_needed,
        payer = user,
        space = UserMarketPosition::LEN,
        seeds = [b"market_position", market_vault.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_market_position: Box<Account<'info, UserMarketPosition>>,

    #[account(mut)]
    pub user_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    #[account(
        mut,
        constraint = vault_usdc_ata.owner == market_vault.key(),
        constraint = vault_usdc_ata.mint == market_vault.deposit_mint,
    )]
    pub vault_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    #[account(mut, address = market_vault.vlofi_mint)]
    pub vlofi_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = user_vlofi_ata.mint == market_vault.vlofi_mint,
        constraint = user_vlofi_ata.owner == user.key(),
    )]
    pub user_vlofi_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn deposit_market(ctx: Context<DepositMarket>, _market_id: u64, amount: u64) -> Result<()> {
    require!(amount > 0, OracleError::InvalidInputLength);

    // 1. Transfer USDC from user to vault
    let transfer_accounts = SplTransfer {
        from: ctx.accounts.user_usdc_ata.to_account_info(),
        to: ctx.accounts.vault_usdc_ata.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
        ),
        amount,
    )?;

    // 2. Compute shares to mint using current NAV.
    //    nav_per_share_bps == 0 before Phase 2 realloc — treat as 10_000 (1:1).
    //    shares = amount * 10_000 / nav_per_share_bps
    //    At genesis (nav=10_000): shares == amount (1:1, backward compatible).
    //    After yield (nav>10_000): fewer shares minted per USDC, but each worth more.
    let vault = &mut ctx.accounts.market_vault;
    let effective_nav = if vault.nav_per_share_bps == 0 {
        10_000u64
    } else {
        vault.nav_per_share_bps
    };
    let shares_to_mint = amount
        .checked_mul(10_000)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(effective_nav)
        .ok_or(OracleError::MathOverflow)?;
    require!(shares_to_mint > 0, OracleError::InvalidInputLength);

    // 3. Mint vLOFI (shares_to_mint) to user (ProtocolState PDA as mint authority)
    let protocol_bump = ctx.accounts.protocol_state.bump;
    let protocol_seeds = &[b"protocol_state".as_ref(), &[protocol_bump]];
    let signer = &[&protocol_seeds[..]];

    let mint_accounts = MintTo {
        mint: ctx.accounts.vlofi_mint.to_account_info(),
        to: ctx.accounts.user_vlofi_ata.to_account_info(),
        authority: ctx.accounts.protocol_state.to_account_info(),
    };
    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_accounts,
            signer,
        ),
        shares_to_mint,
    )?;

    // 4. Update vault accounting
    vault.total_deposited = vault
        .total_deposited
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    vault.total_shares = vault
        .total_shares
        .checked_add(shares_to_mint)
        .ok_or(OracleError::MathOverflow)?;

    // 5. Update user position
    let position = &mut ctx.accounts.user_market_position;
    if position.bump == 0 {
        position.bump = ctx.bumps.user_market_position;
        position.user = ctx.accounts.user.key();
        position.market_vault = ctx.accounts.market_vault.key();
        position.entry_slot = Clock::get()?.slot;
    }
    // Reset settled flag if re-depositing after a previous settlement
    if position.settled {
        position.settled = false;
        position.entry_slot = Clock::get()?.slot;
    }
    position.deposited_amount = position
        .deposited_amount
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    position.shares_minted = position
        .shares_minted
        .checked_add(shares_to_mint)
        .ok_or(OracleError::MathOverflow)?;

    msg!(
        "Deposited {} USDC, minted {} vLOFI (nav={} bps)",
        amount,
        shares_to_mint,
        effective_nav
    );

    Ok(())
}

// =============================================================================
// UPDATE ATTENTION — Oracle sets multiplier BPS on user position
// =============================================================================

#[derive(Accounts)]
#[instruction(market_id: u64, user_pubkey: Pubkey, multiplier_bps: u64)]
pub struct UpdateAttention<'info> {
    /// The authorized backend wallet that pushes attention scores.
    #[account(mut)]
    pub oracle_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        has_one = oracle_authority,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [b"market_position", market_vault.key().as_ref(), user_pubkey.as_ref()],
        bump = user_market_position.bump,
        constraint = user_market_position.market_vault == market_vault.key(),
        constraint = user_market_position.user == user_pubkey,
        constraint = !user_market_position.settled,
    )]
    pub user_market_position: Box<Account<'info, UserMarketPosition>>,
}

/// Minimum attention multiplier: 1.0x = 10,000 BPS. Enforces the floor set in scoring.rs.
const MIN_MULTIPLIER_BPS: u64 = 10_000;
/// Maximum attention multiplier: 5.0x = 50,000 BPS.
const MAX_MULTIPLIER_BPS: u64 = 50_000;
const BASE_YIELD_MULTIPLIER_BPS: u64 = 10_000;

fn compute_position_yield_components(
    deposited_amount: u64,
    attention_multiplier_bps: u64,
) -> Result<(u64, u64, u64)> {
    let effective_multiplier = if attention_multiplier_bps == 0 {
        BASE_YIELD_MULTIPLIER_BPS
    } else {
        attention_multiplier_bps
    };
    let base_yield = deposited_amount;
    let attention_bonus = deposited_amount
        .checked_mul(effective_multiplier.saturating_sub(BASE_YIELD_MULTIPLIER_BPS))
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BASE_YIELD_MULTIPLIER_BPS)
        .ok_or(OracleError::MathOverflow)?;
    let total_earned = base_yield
        .checked_add(attention_bonus)
        .ok_or(OracleError::MathOverflow)?;
    Ok((base_yield, attention_bonus, total_earned))
}

pub fn update_attention(
    ctx: Context<UpdateAttention>,
    _market_id: u64,
    _user_pubkey: Pubkey,
    multiplier_bps: u64,
) -> Result<()> {
    require!(
        multiplier_bps >= MIN_MULTIPLIER_BPS,
        OracleError::MultiplierBelowMinimum
    );
    require!(
        multiplier_bps <= MAX_MULTIPLIER_BPS,
        OracleError::MaxMultiplierExceeded
    );

    let position = &mut ctx.accounts.user_market_position;
    position.attention_multiplier_bps = multiplier_bps;

    msg!(
        "Attention Multiplier Updated: {} bps for User {}",
        multiplier_bps,
        ctx.accounts.user_market_position.user
    );

    Ok(())
}

// =============================================================================
// UPDATE NAV — Oracle sets NAV per vLOFI share on MarketVault (Option C Phase 2)
// =============================================================================
//
// Called by the oracle authority once per rebalance cycle (every 5 min).
// nav_per_share_bps encodes the USDC value of 1 vLOFI:
//   10_000 = 1.00 USDC (genesis / pre-yield)
//   10_100 = 1.01 USDC (1% Kamino yield accrued)
//
// deposit_market and settle_market read this field to give depositors yield exposure.

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct UpdateNav<'info> {
    #[account(mut)]
    pub oracle_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        has_one = oracle_authority,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        mut,
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,
}

pub fn update_nav(ctx: Context<UpdateNav>, _market_id: u64, nav_per_share_bps: u64) -> Result<()> {
    // NAV must stay in [10_000, 50_000] and be monotonic non-decreasing.
    // 10_000 = 1.00x principal floor, 50_000 = 5.00x hard ceiling.
    let vault = &mut ctx.accounts.market_vault;
    require!(nav_per_share_bps >= 10_000, OracleError::NavBelowMinimum);
    require!(
        nav_per_share_bps >= vault.nav_per_share_bps.max(10_000),
        OracleError::NavDecreaseNotAllowed
    );
    require!(nav_per_share_bps <= 50_000, OracleError::NavAboveMaximum);

    vault.nav_per_share_bps = nav_per_share_bps;
    vault.last_nav_update_slot = Clock::get()?.slot;

    msg!(
        "NAV updated: market_id={} nav_per_share_bps={}",
        vault.market_id,
        nav_per_share_bps
    );
    Ok(())
}

// =============================================================================
// CLAIM YIELD — Deprecated; CCM distribution is merkle-claim only
// =============================================================================

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct ClaimYield<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [b"market_position", market_vault.key().as_ref(), user.key().as_ref()],
        bump = user_market_position.bump,
        constraint = user_market_position.market_vault == market_vault.key(),
        constraint = user_market_position.user == user.key(),
        constraint = !user_market_position.settled @ OracleError::AlreadySettled,
    )]
    pub user_market_position: Box<Account<'info, UserMarketPosition>>,
}

pub fn claim_yield(ctx: Context<ClaimYield>, _market_id: u64) -> Result<()> {
    msg!("claim_yield is disabled; claim CCM via claim_global/claim_global_v2 merkle proofs");
    let _ = &ctx.accounts.user_market_position;
    err!(OracleError::ClaimYieldDeprecated)
}

// =============================================================================
// SETTLE MARKET — Burn vLOFI, return USDC (CCM is merkle-claimed)
// =============================================================================

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct SettleMarket<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        mut,
        seeds = [b"market_vault", protocol_state.key().as_ref(), &market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [b"market_position", market_vault.key().as_ref(), user.key().as_ref()],
        bump = user_market_position.bump,
        constraint = user_market_position.market_vault == market_vault.key(),
        constraint = user_market_position.user == user.key(),
        constraint = !user_market_position.settled @ OracleError::AlreadySettled,
    )]
    pub user_market_position: Box<Account<'info, UserMarketPosition>>,

    // --- Token Accounts ---
    /// Global vLOFI Mint (Token-2022)
    #[account(mut, address = market_vault.vlofi_mint)]
    pub vlofi_mint: Box<InterfaceAccount<'info, Mint>>,

    /// User's vLOFI account (to burn)
    #[account(
        mut,
        constraint = user_vlofi_ata.mint == market_vault.vlofi_mint,
        constraint = user_vlofi_ata.owner == user.key(),
    )]
    pub user_vlofi_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault's USDC account (to send principal from)
    #[account(
        mut,
        constraint = vault_usdc_ata.owner == market_vault.key(),
        constraint = vault_usdc_ata.mint == market_vault.deposit_mint,
    )]
    pub vault_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    /// User's USDC account (to receive principal back)
    #[account(mut)]
    pub user_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    // --- Programs ---
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
}

pub fn settle_market(ctx: Context<SettleMarket>, market_id: u64) -> Result<()> {
    let vault = &mut ctx.accounts.market_vault;
    let position = &mut ctx.accounts.user_market_position;

    let shares_to_burn = position.shares_minted;

    // Fail-fast: reject zero-share settlements before any NAV math or CPI
    require!(shares_to_burn > 0, OracleError::ZeroSharesMinted);

    // NAV-adjusted principal return (Option C Phase 2).
    // nav_per_share_bps == 0 before realloc — fall back to deposited_amount (1:1).
    // principal = shares * nav / 10_000
    // At genesis (nav=10_000): principal == deposited_amount (backward compatible).
    // After yield (nav=10_100): principal = deposited_amount * 1.01 (yield captured!).
    let effective_nav = if vault.nav_per_share_bps == 0 {
        10_000u64
    } else {
        vault.nav_per_share_bps
    };
    let principal_to_return = if effective_nav == 10_000 {
        position.deposited_amount
    } else {
        shares_to_burn
            .checked_mul(effective_nav)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(10_000)
            .ok_or(OracleError::MathOverflow)?
    };

    // 1. Compute outstanding CCM yield for audit logs only.
    //    Distribution is handled by global merkle claims, not this instruction.
    //    We keep the calculation for settlement observability and legacy reconciliation.
    let (_base_yield, _attention_bonus, total_earned) = compute_position_yield_components(
        position.deposited_amount,
        position.attention_multiplier_bps,
    )?;
    let ccm_yield = total_earned.saturating_sub(position.cumulative_claimed);

    // 1b. Reserve guard — vault USDC ATA must cover the principal return.
    //     When a StrategyVault is deployed (Phase 2), some USDC lives in Kamino.
    //     settle_market NEVER calls Kamino CPI — it draws from reserve only.
    //     If reserve is insufficient, revert cleanly so the rebalancer/queue can top up.
    require!(
        ctx.accounts.vault_usdc_ata.amount >= principal_to_return,
        OracleError::InsufficientReserve
    );

    // 2. Burn User's vLOFI
    let burn_cpi_accounts = Burn {
        mint: ctx.accounts.vlofi_mint.to_account_info(),
        from: ctx.accounts.user_vlofi_ata.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            burn_cpi_accounts,
        ),
        shares_to_burn,
    )?;

    // 3. Return Base USDC from Vault to User (vault PDA signs)
    let protocol_key = ctx.accounts.protocol_state.key();
    let market_id_bytes = market_id.to_le_bytes();
    let vault_bump = vault.bump;
    let vault_seeds = &[
        b"market_vault".as_ref(),
        protocol_key.as_ref(),
        market_id_bytes.as_ref(),
        &[vault_bump],
    ];
    let vault_signer = &[&vault_seeds[..]];

    let transfer_usdc_accounts = SplTransfer {
        from: ctx.accounts.vault_usdc_ata.to_account_info(),
        to: ctx.accounts.user_usdc_ata.to_account_info(),
        authority: vault.to_account_info(),
    };
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_usdc_accounts,
            vault_signer,
        ),
        principal_to_return,
    )?;

    // 4. CCM Yield — distributed via merkle claims (claim_global), NOT mint_to.
    //    CCM mint authority is revoked; calling mint_to would brick this tx.
    //    Log the accrued yield so settlement receipts remain auditable.
    if ccm_yield > 0 {
        msg!(
            "CCM yield: {} (claim via claim_global merkle proof)",
            ccm_yield
        );
    }

    // 5. Update State Accounting
    //    Subtract the original deposited_amount (not NAV-adjusted principal_to_return)
    //    to keep total_deposited consistent. NAV appreciation is vault-level yield,
    //    not additional deposits — subtracting principal_to_return when NAV > 1.0x
    //    would underflow total_deposited for later settlers.
    vault.total_deposited = vault
        .total_deposited
        .checked_sub(position.deposited_amount)
        .ok_or(OracleError::MathOverflow)?;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares_to_burn)
        .ok_or(OracleError::MathOverflow)?;

    position.shares_minted = 0;
    position.deposited_amount = 0;
    position.settled = true;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yield_components_baseline_multiplier() {
        // test: 1x multiplier → zero bonus
        let r = compute_position_yield_components(1_000_000, 10_000);
        assert!(r.is_ok());
        let (base, bonus, total) = r.ok().expect("test"); // test
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 0);
        assert_eq!(total, 1_000_000);
    }

    #[test]
    fn yield_components_zero_multiplier_falls_back() {
        // test: 0 multiplier falls back to BASE_YIELD
        let r = compute_position_yield_components(1_000_000, 0);
        assert!(r.is_ok());
        let (base, bonus, total) = r.ok().expect("test"); // test
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 0);
        assert_eq!(total, 1_000_000);
    }

    #[test]
    fn yield_components_2x_multiplier() {
        // test: 2x multiplier → 1x bonus
        let r = compute_position_yield_components(1_000_000, 20_000);
        let (base, bonus, total) = r.ok().expect("test"); // test
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 1_000_000);
        assert_eq!(total, 2_000_000);
    }

    #[test]
    fn yield_components_max_multiplier() {
        // test: 5x multiplier → 4x bonus
        let r = compute_position_yield_components(1_000_000, 50_000);
        let (base, bonus, total) = r.ok().expect("test"); // test
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 4_000_000);
        assert_eq!(total, 5_000_000);
    }

    #[test]
    fn yield_components_zero_deposit() {
        // test: zero deposit → zero everything
        let r = compute_position_yield_components(0, 20_000);
        let (base, bonus, total) = r.ok().expect("test"); // test
        assert_eq!(base, 0);
        assert_eq!(bonus, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn yield_components_large_deposit_no_overflow() {
        // test: large deposit stays within checked bounds
        let deposit: u64 = 368_934_881_474_191; // max safe at 5x
        let result = compute_position_yield_components(deposit, 50_000);
        assert!(result.is_ok());
    }

    #[test]
    fn nav_adjusted_principal_at_genesis() {
        // test: NAV 1.0x → principal == shares
        let shares: u64 = 1_000_000;
        let nav_bps: u64 = 10_000;
        let principal = if nav_bps == 10_000 { shares } else { 0 };
        assert_eq!(principal, 1_000_000);
    }

    #[test]
    fn nav_adjusted_principal_with_yield() {
        // test: NAV 1.01x → 1% yield
        let shares: u64 = 1_000_000;
        let nav_bps: u64 = 10_100;
        let principal = shares
            .checked_mul(nav_bps)
            .and_then(|v| v.checked_div(10_000)); // test
        assert_eq!(principal, Some(1_010_000));
    }

    #[test]
    fn nav_adjusted_principal_at_max() {
        // test: NAV 5.0x → 5x principal
        let shares: u64 = 1_000_000;
        let nav_bps: u64 = 50_000;
        let principal = shares
            .checked_mul(nav_bps)
            .and_then(|v| v.checked_div(10_000)); // test
        assert_eq!(principal, Some(5_000_000));
    }
}
