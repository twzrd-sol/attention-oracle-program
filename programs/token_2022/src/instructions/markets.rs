use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    Mint as MintInterface, TokenAccount as TokenAccountInterface, TokenInterface,
};

use crate::constants::{
    CUMULATIVE_ROOT_HISTORY, GLOBAL_ROOT_SEED, MARKET_METRIC_ATTENTION_SCORE,
    MARKET_MINT_AUTHORITY_SEED, MARKET_NO_MINT_SEED, MARKET_STATE_SEED, MARKET_VAULT_SEED,
    MARKET_YES_MINT_SEED, PROTOCOL_SEED,
};
use crate::errors::OracleError;
use crate::events::{
    MarketCreated, MarketResolved, MarketSettled, MarketTokensInitialized, SharesMinted,
    SharesRedeemed,
};
use crate::merkle_proof::{compute_global_leaf, verify_proof};
use crate::state::{GlobalRootConfig, MarketState, ProtocolState};
use crate::token_transfer::transfer_checked_with_remaining;

const MARKET_STATE_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;
const CCM_DECIMALS: u8 = 9;

// =============================================================================
// CREATE MARKET
// =============================================================================

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [GLOBAL_ROOT_SEED, protocol_state.mint.as_ref()],
        bump = global_root_config.bump,
    )]
    pub global_root_config: Account<'info, GlobalRootConfig>,

    #[account(
        init,
        payer = authority,
        space = MarketState::LEN,
        seeds = [MARKET_STATE_SEED, protocol_state.mint.as_ref(), &market_id.to_le_bytes()],
        bump,
    )]
    pub market_state: Account<'info, MarketState>,

    pub system_program: Program<'info, System>,
}

pub fn create_market(
    ctx: Context<CreateMarket>,
    market_id: u64,
    creator_wallet: Pubkey,
    metric: u8,
    target: u64,
    resolution_root_seq: u64,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let global_root_config = &ctx.accounts.global_root_config;
    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require!(
        creator_wallet != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    require!(
        metric == MARKET_METRIC_ATTENTION_SCORE,
        OracleError::UnsupportedMarketMetric
    );
    require!(resolution_root_seq > 0, OracleError::InvalidRootSeq);
    require!(
        global_root_config.version > 0,
        OracleError::GlobalRootNotInitialized
    );
    require_keys_eq!(
        global_root_config.mint,
        protocol_state.mint,
        OracleError::InvalidMint
    );

    let slot = Clock::get()?.slot;
    let market_state = &mut ctx.accounts.market_state;
    market_state.version = MARKET_STATE_VERSION;
    market_state.bump = ctx.bumps.market_state;
    market_state.metric = metric;
    market_state.resolved = false;
    market_state.outcome = false;
    market_state.tokens_initialized = false;
    market_state._padding = [0u8; 2];
    market_state.market_id = market_id;
    market_state.mint = protocol_state.mint;
    market_state.authority = ctx.accounts.authority.key();
    market_state.creator_wallet = creator_wallet;
    market_state.target = target;
    market_state.resolution_root_seq = resolution_root_seq;
    market_state.resolution_cumulative_total = 0;
    market_state.created_slot = slot;
    market_state.resolved_slot = 0;
    // Token fields are zeroed until initialize_market_tokens is called
    market_state.vault = Pubkey::default();
    market_state.yes_mint = Pubkey::default();
    market_state.no_mint = Pubkey::default();
    market_state.mint_authority = Pubkey::default();

    emit!(MarketCreated {
        market: market_state.key(),
        market_id,
        authority: market_state.authority,
        creator_wallet,
        mint: protocol_state.mint,
        metric,
        target,
        resolution_root_seq,
        created_slot: slot,
    });

    Ok(())
}

// =============================================================================
// INITIALIZE MARKET TOKENS (vault + YES/NO mints)
// =============================================================================

#[derive(Accounts)]
pub struct InitializeMarketTokens<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [MARKET_STATE_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump = market_state.bump,
        constraint = market_state.authority == payer.key() @ OracleError::Unauthorized,
    )]
    pub market_state: Account<'info, MarketState>,

    /// CCM mint (Token-2022)
    /// CHECK: validated by constraint against protocol_state.mint
    #[account(
        constraint = ccm_mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub ccm_mint: InterfaceAccount<'info, MintInterface>,

    /// Market vault — holds CCM collateral backing all shares
    /// CHECK: initialized via CPI below
    #[account(
        init,
        payer = payer,
        token::mint = ccm_mint,
        token::authority = mint_authority,
        token::token_program = token_program,
        seeds = [MARKET_VAULT_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
    )]
    pub vault: InterfaceAccount<'info, TokenAccountInterface>,

    /// YES outcome mint (standard SPL — no transfer fees on outcome tokens)
    /// CHECK: initialized via CPI below
    #[account(
        init,
        payer = payer,
        mint::decimals = CCM_DECIMALS,
        mint::authority = mint_authority,
        mint::token_program = standard_token_program,
        seeds = [MARKET_YES_MINT_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
    )]
    pub yes_mint: Account<'info, anchor_spl::token::Mint>,

    /// NO outcome mint (standard SPL — no transfer fees on outcome tokens)
    /// CHECK: initialized via CPI below
    #[account(
        init,
        payer = payer,
        mint::decimals = CCM_DECIMALS,
        mint::authority = mint_authority,
        mint::token_program = standard_token_program,
        seeds = [MARKET_NO_MINT_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
    )]
    pub no_mint: Account<'info, anchor_spl::token::Mint>,

    /// Mint authority PDA (signs mint/burn of YES/NO tokens)
    /// CHECK: PDA derived from seeds, no data stored
    #[account(
        seeds = [MARKET_MINT_AUTHORITY_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
    )]
    pub mint_authority: SystemAccount<'info>,

    /// Token-2022 program (for CCM vault)
    pub token_program: Interface<'info, TokenInterface>,
    /// Standard SPL token program (for YES/NO mints)
    pub standard_token_program: Program<'info, anchor_spl::token::Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_market_tokens(ctx: Context<InitializeMarketTokens>) -> Result<()> {
    let market_state = &mut ctx.accounts.market_state;
    require!(
        !market_state.tokens_initialized,
        OracleError::MarketTokensAlreadyInitialized
    );

    market_state.vault = ctx.accounts.vault.key();
    market_state.yes_mint = ctx.accounts.yes_mint.key();
    market_state.no_mint = ctx.accounts.no_mint.key();
    market_state.mint_authority = ctx.accounts.mint_authority.key();
    market_state.tokens_initialized = true;

    emit!(MarketTokensInitialized {
        market: market_state.key(),
        market_id: market_state.market_id,
        vault: market_state.vault,
        yes_mint: market_state.yes_mint,
        no_mint: market_state.no_mint,
        mint_authority: market_state.mint_authority,
    });

    Ok(())
}

// =============================================================================
// MINT SHARES (deposit CCM → get YES + NO)
// =============================================================================

#[derive(Accounts)]
pub struct MintShares<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_STATE_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump = market_state.bump,
        constraint = market_state.tokens_initialized @ OracleError::MarketTokensNotInitialized,
        constraint = !market_state.resolved @ OracleError::MarketAlreadyResolved,
    )]
    pub market_state: Box<Account<'info, MarketState>>,

    /// CCM mint (Token-2022)
    /// CHECK: validated against protocol_state.mint
    #[account(
        constraint = ccm_mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Depositor's CCM token account
    #[account(
        mut,
        token::mint = ccm_mint,
        token::authority = depositor,
        token::token_program = token_program,
    )]
    pub depositor_ccm: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Market vault (receives CCM collateral)
    #[account(
        mut,
        token::mint = ccm_mint,
        token::token_program = token_program,
        constraint = vault.key() == market_state.vault @ OracleError::InvalidMarketState,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// YES outcome mint
    #[account(
        mut,
        constraint = yes_mint.key() == market_state.yes_mint @ OracleError::InvalidMarketState,
    )]
    pub yes_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// NO outcome mint
    #[account(
        mut,
        constraint = no_mint.key() == market_state.no_mint @ OracleError::InvalidMarketState,
    )]
    pub no_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// Depositor's YES token account
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = depositor,
    )]
    pub depositor_yes: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Depositor's NO token account
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = depositor,
    )]
    pub depositor_no: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Mint authority PDA
    /// CHECK: validated against market_state.mint_authority
    #[account(
        seeds = [MARKET_MINT_AUTHORITY_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
        constraint = mint_authority.key() == market_state.mint_authority @ OracleError::InvalidMarketState,
    )]
    pub mint_authority: SystemAccount<'info>,

    /// Token-2022 (for CCM transfers)
    pub token_program: Interface<'info, TokenInterface>,
    /// Standard SPL token (for YES/NO mint operations)
    pub standard_token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn mint_shares<'info>(
    ctx: Context<'_, '_, '_, 'info, MintShares<'info>>,
    amount: u64,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require!(amount > 0, OracleError::ZeroSharesMinted);

    // CRITICAL: Snapshot vault balance BEFORE transfer to calculate net received
    let vault_before = ctx.accounts.vault.amount;

    // Transfer CCM from depositor to vault (Token-2022 — may deduct transfer fee)
    transfer_checked_with_remaining(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.depositor_ccm.to_account_info(),
        &ctx.accounts.ccm_mint.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.depositor.to_account_info(),
        amount,
        CCM_DECIMALS,
        &[], // depositor signs directly
        ctx.remaining_accounts,
    )?;

    // Reload vault to get post-transfer balance
    ctx.accounts.vault.reload()?;
    let vault_after = ctx.accounts.vault.amount;
    let net_received = vault_after
        .checked_sub(vault_before)
        .ok_or(OracleError::MathOverflow)?;

    require!(net_received > 0, OracleError::ZeroSharesMinted);

    // Mint exactly net_received YES + NO tokens (1:1 backing)
    let market_id_bytes = ctx.accounts.market_state.market_id.to_le_bytes();
    let mint_key = protocol_state.mint;
    let auth_seeds: &[&[u8]] = &[
        MARKET_MINT_AUTHORITY_SEED,
        mint_key.as_ref(),
        &market_id_bytes,
        &[ctx.bumps.mint_authority],
    ];

    // Mint YES shares
    anchor_spl::token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.standard_token_program.to_account_info(),
            anchor_spl::token::MintTo {
                mint: ctx.accounts.yes_mint.to_account_info(),
                to: ctx.accounts.depositor_yes.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            &[auth_seeds],
        ),
        net_received,
    )?;

    // Mint NO shares
    anchor_spl::token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.standard_token_program.to_account_info(),
            anchor_spl::token::MintTo {
                mint: ctx.accounts.no_mint.to_account_info(),
                to: ctx.accounts.depositor_no.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            &[auth_seeds],
        ),
        net_received,
    )?;

    emit!(SharesMinted {
        market: ctx.accounts.market_state.key(),
        market_id: ctx.accounts.market_state.market_id,
        depositor: ctx.accounts.depositor.key(),
        deposit_amount: amount,
        net_amount: net_received,
        shares_minted: net_received,
    });

    Ok(())
}

// =============================================================================
// REDEEM SHARES (burn equal YES + NO → get CCM back, pre-resolution only)
// =============================================================================

#[derive(Accounts)]
pub struct RedeemShares<'info> {
    #[account(mut)]
    pub redeemer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_STATE_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump = market_state.bump,
        constraint = market_state.tokens_initialized @ OracleError::MarketTokensNotInitialized,
        constraint = !market_state.resolved @ OracleError::MarketAlreadyResolved,
    )]
    pub market_state: Box<Account<'info, MarketState>>,

    /// CCM mint (Token-2022)
    #[account(
        constraint = ccm_mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Market vault
    #[account(
        mut,
        token::mint = ccm_mint,
        token::token_program = token_program,
        constraint = vault.key() == market_state.vault @ OracleError::InvalidMarketState,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// YES outcome mint
    #[account(
        mut,
        constraint = yes_mint.key() == market_state.yes_mint @ OracleError::InvalidMarketState,
    )]
    pub yes_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// NO outcome mint
    #[account(
        mut,
        constraint = no_mint.key() == market_state.no_mint @ OracleError::InvalidMarketState,
    )]
    pub no_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// Redeemer's YES token account
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = redeemer,
    )]
    pub redeemer_yes: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Redeemer's NO token account
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = redeemer,
    )]
    pub redeemer_no: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Redeemer's CCM token account (receives CCM back)
    #[account(
        mut,
        token::mint = ccm_mint,
        token::authority = redeemer,
        token::token_program = token_program,
    )]
    pub redeemer_ccm: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Mint authority PDA
    #[account(
        seeds = [MARKET_MINT_AUTHORITY_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
        constraint = mint_authority.key() == market_state.mint_authority @ OracleError::InvalidMarketState,
    )]
    pub mint_authority: SystemAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub standard_token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn redeem_shares<'info>(
    ctx: Context<'_, '_, '_, 'info, RedeemShares<'info>>,
    shares: u64,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require!(shares > 0, OracleError::ZeroSharesMinted);

    // Burn equal YES and NO tokens
    anchor_spl::token::burn(
        CpiContext::new(
            ctx.accounts.standard_token_program.to_account_info(),
            anchor_spl::token::Burn {
                mint: ctx.accounts.yes_mint.to_account_info(),
                from: ctx.accounts.redeemer_yes.to_account_info(),
                authority: ctx.accounts.redeemer.to_account_info(),
            },
        ),
        shares,
    )?;

    anchor_spl::token::burn(
        CpiContext::new(
            ctx.accounts.standard_token_program.to_account_info(),
            anchor_spl::token::Burn {
                mint: ctx.accounts.no_mint.to_account_info(),
                from: ctx.accounts.redeemer_no.to_account_info(),
                authority: ctx.accounts.redeemer.to_account_info(),
            },
        ),
        shares,
    )?;

    // Transfer CCM from vault back to redeemer (outbound transfer fee applies)
    let market_id_bytes = ctx.accounts.market_state.market_id.to_le_bytes();
    let mint_key = protocol_state.mint;
    let auth_seeds: &[&[u8]] = &[
        MARKET_MINT_AUTHORITY_SEED,
        mint_key.as_ref(),
        &market_id_bytes,
        &[ctx.bumps.mint_authority],
    ];

    transfer_checked_with_remaining(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.ccm_mint.to_account_info(),
        &ctx.accounts.redeemer_ccm.to_account_info(),
        &ctx.accounts.mint_authority.to_account_info(),
        shares,
        CCM_DECIMALS,
        &[auth_seeds],
        ctx.remaining_accounts,
    )?;

    emit!(SharesRedeemed {
        market: ctx.accounts.market_state.key(),
        market_id: ctx.accounts.market_state.market_id,
        redeemer: ctx.accounts.redeemer.key(),
        shares_burned: shares,
        ccm_returned: shares, // gross; net is less after transfer fee
    });

    Ok(())
}

// =============================================================================
// RESOLVE MARKET
// =============================================================================

#[derive(Accounts)]
pub struct ResolveMarket<'info> {
    pub resolver: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [GLOBAL_ROOT_SEED, protocol_state.mint.as_ref()],
        bump = global_root_config.bump,
    )]
    pub global_root_config: Account<'info, GlobalRootConfig>,

    #[account(
        mut,
        seeds = [MARKET_STATE_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump = market_state.bump,
    )]
    pub market_state: Account<'info, MarketState>,
}

pub fn resolve_market(
    ctx: Context<ResolveMarket>,
    cumulative_total: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let global_root_config = &ctx.accounts.global_root_config;
    let market_state = &mut ctx.accounts.market_state;
    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require!(
        proof.len() <= MAX_PROOF_LEN,
        OracleError::InvalidProofLength
    );
    require!(
        market_state.version == MARKET_STATE_VERSION,
        OracleError::InvalidMarketState
    );
    require_keys_eq!(
        market_state.mint,
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        market_state.metric == MARKET_METRIC_ATTENTION_SCORE,
        OracleError::UnsupportedMarketMetric
    );
    require!(!market_state.resolved, OracleError::MarketAlreadyResolved);
    require!(
        global_root_config.version > 0,
        OracleError::GlobalRootNotInitialized
    );
    require_keys_eq!(
        global_root_config.mint,
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        market_state.resolution_root_seq <= global_root_config.latest_root_seq,
        OracleError::MarketNotResolvableYet
    );

    let root_seq = market_state.resolution_root_seq;
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = global_root_config.roots[idx];
    require!(entry.seq == root_seq, OracleError::RootTooOldOrMissing);

    let leaf = compute_global_leaf(
        &protocol_state.mint,
        root_seq,
        &market_state.creator_wallet,
        cumulative_total,
    );
    require!(
        verify_proof(&proof, leaf, entry.root),
        OracleError::InvalidProof
    );

    let outcome = cumulative_total >= market_state.target;
    let slot = Clock::get()?.slot;
    market_state.resolved = true;
    market_state.outcome = outcome;
    market_state.resolution_cumulative_total = cumulative_total;
    market_state.resolved_slot = slot;

    emit!(MarketResolved {
        market: market_state.key(),
        market_id: market_state.market_id,
        resolver: ctx.accounts.resolver.key(),
        creator_wallet: market_state.creator_wallet,
        metric: market_state.metric,
        target: market_state.target,
        resolution_root_seq: market_state.resolution_root_seq,
        verified_cumulative_total: cumulative_total,
        outcome,
        resolved_slot: slot,
    });

    Ok(())
}

// =============================================================================
// SETTLE (burn winning shares → claim CCM, post-resolution only)
// =============================================================================

#[derive(Accounts)]
pub struct Settle<'info> {
    #[account(mut)]
    pub settler: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_STATE_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump = market_state.bump,
        constraint = market_state.tokens_initialized @ OracleError::MarketTokensNotInitialized,
        constraint = market_state.resolved @ OracleError::MarketNotResolved,
    )]
    pub market_state: Box<Account<'info, MarketState>>,

    /// CCM mint (Token-2022)
    #[account(
        constraint = ccm_mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Market vault
    #[account(
        mut,
        token::mint = ccm_mint,
        token::token_program = token_program,
        constraint = vault.key() == market_state.vault @ OracleError::InvalidMarketState,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// The WINNING outcome mint (YES if outcome=true, NO if outcome=false)
    #[account(mut)]
    pub winning_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// Settler's winning token account
    #[account(
        mut,
        token::mint = winning_mint,
        token::authority = settler,
    )]
    pub settler_winning: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Settler's CCM token account (receives settlement)
    #[account(
        mut,
        token::mint = ccm_mint,
        token::authority = settler,
        token::token_program = token_program,
    )]
    pub settler_ccm: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Mint authority PDA
    #[account(
        seeds = [MARKET_MINT_AUTHORITY_SEED, protocol_state.mint.as_ref(), &market_state.market_id.to_le_bytes()],
        bump,
        constraint = mint_authority.key() == market_state.mint_authority @ OracleError::InvalidMarketState,
    )]
    pub mint_authority: SystemAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub standard_token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn settle<'info>(
    ctx: Context<'_, '_, '_, 'info, Settle<'info>>,
    shares: u64,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let market_state = &ctx.accounts.market_state;
    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require!(shares > 0, OracleError::ZeroSharesMinted);

    // Verify the settler is burning the correct winning side
    let expected_winning_mint = if market_state.outcome {
        market_state.yes_mint
    } else {
        market_state.no_mint
    };
    require_keys_eq!(
        ctx.accounts.winning_mint.key(),
        expected_winning_mint,
        OracleError::WrongOutcomeToken
    );

    // Verify vault has enough CCM
    require!(
        ctx.accounts.vault.amount >= shares,
        OracleError::InsufficientVaultBalance
    );

    // Burn winning shares
    anchor_spl::token::burn(
        CpiContext::new(
            ctx.accounts.standard_token_program.to_account_info(),
            anchor_spl::token::Burn {
                mint: ctx.accounts.winning_mint.to_account_info(),
                from: ctx.accounts.settler_winning.to_account_info(),
                authority: ctx.accounts.settler.to_account_info(),
            },
        ),
        shares,
    )?;

    // Transfer CCM from vault to settler (Token-2022 transfer fee applies on exit)
    let market_id_bytes = market_state.market_id.to_le_bytes();
    let mint_key = protocol_state.mint;
    let auth_seeds: &[&[u8]] = &[
        MARKET_MINT_AUTHORITY_SEED,
        mint_key.as_ref(),
        &market_id_bytes,
        &[ctx.bumps.mint_authority],
    ];

    transfer_checked_with_remaining(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.ccm_mint.to_account_info(),
        &ctx.accounts.settler_ccm.to_account_info(),
        &ctx.accounts.mint_authority.to_account_info(),
        shares,
        CCM_DECIMALS,
        &[auth_seeds],
        ctx.remaining_accounts,
    )?;

    emit!(MarketSettled {
        market: market_state.key(),
        market_id: market_state.market_id,
        settler: ctx.accounts.settler.key(),
        winning_side: market_state.outcome,
        shares_burned: shares,
        ccm_returned: shares, // gross; net is less after transfer fee
    });

    Ok(())
}
