use crate::{
    constants::PROTOCOL_SEED,
    errors::OracleError,
    state::{FeeConfig, ProtocolState},
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022_extensions::transfer_fee::{
    withdraw_withheld_tokens_from_accounts, WithdrawWithheldTokensFromAccounts,
};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

// ============================================================================
// Fee Harvesting (Token-2022 Withheld Tokens)
// ============================================================================

#[event]
pub struct FeesHarvested {
    pub mint: Pubkey,
    pub withheld_amount: u64,
    pub treasury_share: u64,
    pub creator_pool_share: u64,
    pub timestamp: i64,
}

/// Harvest withheld fees from Token-2022 accounts and transfer to treasury
/// Uses Token-2022's withdraw_withheld_tokens_from_accounts instruction
/// Source ATAs (user/LP accounts with withheld fees) passed via remaining_accounts
#[derive(Accounts)]
pub struct HarvestFees<'info> {
    /// Withdraw authority (must be protocol admin)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state (mint-keyed) - PDA is the withdraw_withheld_authority
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration (for tier multipliers)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// CCM Token-2022 mint
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury token account (receives harvested fees)
    #[account(
        mut,
        constraint = treasury.owner == protocol_state.key() @ OracleError::Unauthorized,
        constraint = treasury.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub treasury: InterfaceAccount<'info, TokenAccount>,

    /// Token-2022 program
    pub token_program: Interface<'info, TokenInterface>,
    // remaining_accounts: source ATAs with withheld fees to harvest from
}

/// Harvest withheld fees from source ATAs to treasury
/// Source ATAs passed via remaining_accounts - validates they have withheld fees
pub fn harvest_and_distribute_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>,
) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let protocol_state = &ctx.accounts.protocol_state;
    let mint_key = ctx.accounts.mint.key();

    // Require at least one source ATA to harvest from
    require!(
        !ctx.remaining_accounts.is_empty(),
        OracleError::InvalidInputLength
    );

    // Cap sources to prevent compute exhaustion (Token-2022 limit is ~35 per tx)
    require!(
        ctx.remaining_accounts.len() <= 30,
        OracleError::InvalidInputLength
    );

    // Record treasury balance before harvest
    let treasury_before = ctx.accounts.treasury.amount;

    // PDA signer seeds for protocol_state (withdraw_withheld_authority)
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        mint_key.as_ref(),
        &[protocol_state.bump],
    ];
    let signer_seeds = &[seeds];

    // Collect source account infos from remaining_accounts
    let sources: Vec<AccountInfo<'info>> = ctx.remaining_accounts.to_vec();

    // Call Token-2022 withdraw_withheld_tokens_from_accounts CPI
    // This withdraws withheld fees from all source ATAs to treasury in one tx
    withdraw_withheld_tokens_from_accounts(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            WithdrawWithheldTokensFromAccounts {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                destination: ctx.accounts.treasury.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
            },
            signer_seeds,
        ),
        sources,
    )?;

    // Reload treasury to get new balance
    ctx.accounts.treasury.reload()?;
    let treasury_after = ctx.accounts.treasury.amount;
    let withheld_amount = treasury_after.saturating_sub(treasury_before);

    // For now, all fees go to treasury (creator pool split can be added later)
    let treasury_share = withheld_amount;
    let creator_pool_share = 0u64;

    emit!(FeesHarvested {
        mint: mint_key,
        withheld_amount,
        treasury_share,
        creator_pool_share,
        timestamp: ts,
    });

    msg!(
        "Harvest complete: {} sources, {} CCM withdrawn to treasury",
        ctx.remaining_accounts.len(),
        withheld_amount
    );

    Ok(())
}
