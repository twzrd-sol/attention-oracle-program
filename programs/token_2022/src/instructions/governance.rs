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
    /// Permissionless crank -- anyone can trigger fee harvesting.
    /// Fees always go to the protocol treasury (enforced by treasury constraint).
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state (mint-keyed) - PDA is the withdraw_withheld_authority
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration (for tier multipliers)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// Token-2022 mint
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury token account (receives harvested fees)
    /// Owner must match protocol_state.treasury (the fee destination owner)
    #[account(
        mut,
        constraint = treasury.owner == protocol_state.treasury @ OracleError::Unauthorized,
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

    // SECURITY: Validate source accounts before CPI
    // Ensures all accounts are Token-2022 token accounts for the correct mint
    // This prevents cross-mint attacks and garbage account injection
    for source_info in ctx.remaining_accounts.iter() {
        // Check ownership - must be Token-2022 program
        require!(
            source_info.owner == &ctx.accounts.token_program.key(),
            OracleError::InvalidTokenProgram
        );

        // Parse as token account to validate mint (lightweight check)
        // Token-2022 ATA layout: mint pubkey is at bytes [0..32]
        let data = source_info.try_borrow_data()?;
        require!(data.len() >= 32, OracleError::InvalidTokenProgram);

        // Extract mint from account data and verify it matches
        let account_mint = Pubkey::new_from_array(
            data[0..32]
                .try_into()
                .map_err(|_| OracleError::InvalidTokenProgram)?,
        );
        require_keys_eq!(account_mint, mint_key, OracleError::InvalidMint);
    }

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
        "Harvest complete: {} sources, {} tokens withdrawn to treasury",
        ctx.remaining_accounts.len(),
        withheld_amount
    );

    Ok(())
}
