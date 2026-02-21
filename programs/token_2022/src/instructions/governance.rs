use crate::{
    constants::PROTOCOL_SEED,
    errors::OracleError,
    state::{FeeConfig, ProtocolState},
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022_extensions::transfer_fee::{
    withdraw_withheld_tokens_from_accounts, withdraw_withheld_tokens_from_mint,
    WithdrawWithheldTokensFromAccounts, WithdrawWithheldTokensFromMint,
};
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

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

#[event]
pub struct FeesWithdrawnFromMint {
    pub mint: Pubkey,
    pub withdrawn_amount: u64,
    pub destination: Pubkey,
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
    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
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

// ============================================================================
// Mint-Level Fee Withdrawal (Token-2022 Withheld on Mint -> Treasury ATA)
// ============================================================================

#[derive(Accounts)]
pub struct WithdrawFeesFromMint<'info> {
    /// Permissionless crank -- anyone can trigger the CPI.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state PDA is the mint's withdraw_withheld_authority.
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Token-2022 mint holding aggregated withheld fees.
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury ATA destination for withdrawn fees.
    /// Owner must match protocol_state.treasury (fee destination owner).
    #[account(
        mut,
        constraint = treasury_ata.owner == protocol_state.treasury @ OracleError::Unauthorized,
        constraint = treasury_ata.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Token-2022 program
    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn withdraw_fees_from_mint(ctx: Context<WithdrawFeesFromMint>) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let protocol_state = &ctx.accounts.protocol_state;
    let mint_key = ctx.accounts.mint.key();

    let treasury_before = ctx.accounts.treasury_ata.amount;

    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[protocol_state.bump]];
    let signer_seeds = &[seeds];

    withdraw_withheld_tokens_from_mint(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            WithdrawWithheldTokensFromMint {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                destination: ctx.accounts.treasury_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
            },
            signer_seeds,
        ),
    )?;

    ctx.accounts.treasury_ata.reload()?;
    let treasury_after = ctx.accounts.treasury_ata.amount;
    let withdrawn_amount = treasury_after.saturating_sub(treasury_before);

    emit!(FeesWithdrawnFromMint {
        mint: mint_key,
        withdrawn_amount,
        destination: ctx.accounts.treasury_ata.key(),
        timestamp: ts,
    });

    msg!(
        "Mint withdraw complete: {} tokens moved to treasury {}",
        withdrawn_amount,
        ctx.accounts.treasury_ata.key()
    );

    Ok(())
}

// ============================================================================
// Treasury Routing (Treasury ATA -> Destination ATA, e.g. Vault Buffer)
// ============================================================================

#[event]
pub struct TreasuryRouted {
    pub mint: Pubkey,
    pub amount: u64,
    pub destination: Pubkey,
    pub treasury_remaining: u64,
    pub timestamp: i64,
}

/// Route excess CCM from treasury to a destination (e.g. vault buffer).
/// Admin-only. Enforces an on-chain minimum reserve to protect claim funding.
#[derive(Accounts)]
pub struct RouteTreasury<'info> {
    /// Protocol admin — only admin can route treasury funds.
    #[account(
        mut,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub admin: Signer<'info>,

    /// Protocol state PDA — token owner of the treasury ATA.
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Token-2022 mint (for decimals in transfer_checked).
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury ATA (source). Must be owned by Protocol State PDA.
    #[account(
        mut,
        constraint = treasury_ata.mint == mint.key() @ OracleError::InvalidMint,
        constraint = treasury_ata.owner == protocol_state.key() @ OracleError::Unauthorized,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Destination ATA (e.g. vault buffer). Must be same mint.
    #[account(
        mut,
        constraint = destination_ata.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub destination_ata: InterfaceAccount<'info, TokenAccount>,

    /// Token-2022 program
    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn route_treasury(
    ctx: Context<RouteTreasury>,
    amount: u64,
    min_reserve: u64,
) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let protocol_state = &ctx.accounts.protocol_state;
    let mint_key = ctx.accounts.mint.key();

    // Block routing while paused
    require!(!protocol_state.paused, OracleError::ProtocolPaused);

    require!(amount > 0, OracleError::InvalidInputLength);
    require!(min_reserve > 0, OracleError::InvalidInputLength);

    // Enforce minimum reserve: treasury must retain at least min_reserve after transfer
    let treasury_balance = ctx.accounts.treasury_ata.amount;
    let balance_after = treasury_balance
        .checked_sub(amount)
        .ok_or(OracleError::InsufficientTreasuryBalance)?;
    require!(
        balance_after >= min_reserve,
        OracleError::InsufficientTreasuryBalance
    );

    // CPI: transfer_checked from treasury to destination, signed by Protocol State PDA
    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[protocol_state.bump]];
    let signer_seeds = &[seeds];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.destination_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // Reload treasury for post-transfer balance
    ctx.accounts.treasury_ata.reload()?;

    emit!(TreasuryRouted {
        mint: mint_key,
        amount,
        destination: ctx.accounts.destination_ata.key(),
        treasury_remaining: ctx.accounts.treasury_ata.amount,
        timestamp: ts,
    });

    msg!(
        "Treasury routed: {} tokens to {}, {} remaining",
        amount,
        ctx.accounts.destination_ata.key(),
        ctx.accounts.treasury_ata.amount
    );

    Ok(())
}
