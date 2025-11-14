use crate::{
    constants::{HARVEST_SPLIT_BPS_TREASURY, PROTOCOL_SEED},
    errors::MiloError,
    state::{FeeConfig, FeeSplit, ProtocolState},
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022;
use anchor_spl::token_2022::spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, BaseStateWithExtensions,
};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[derive(Accounts)]
pub struct UpdateFeeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Global protocol state (authority + mint/treasury refs)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_fee_config(
    ctx: Context<UpdateFeeConfig>,
    new_basis_points: u16,
    _fee_split: FeeSplit,
) -> Result<()> {
    require!(
        new_basis_points <= crate::constants::MAX_FEE_BASIS_POINTS,
        MiloError::InvalidFeeBps
    );

    // For v1, we store only basis_points and max_fee at init; allow updating basis points.
    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = new_basis_points;

    Ok(())
}

// Open variant for mint-keyed protocol instances
#[derive(Accounts)]
pub struct UpdateFeeConfigOpen<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Global protocol state (mint-keyed)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA (mint-keyed)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_fee_config_open(
    ctx: Context<UpdateFeeConfigOpen>,
    new_basis_points: u16,
    _fee_split: FeeSplit,
) -> Result<()> {
    require!(
        new_basis_points <= crate::constants::MAX_FEE_BASIS_POINTS,
        MiloError::InvalidFeeBps
    );

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = new_basis_points;
    Ok(())
}

// Instruction to update tier multipliers (dynamic fee allocation)
#[derive(Accounts)]
pub struct UpdateTierMultipliers<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Global protocol state (mint-keyed)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA (mint-keyed)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_tier_multipliers(
    ctx: Context<UpdateTierMultipliers>,
    new_multipliers: [u32; 6],
) -> Result<()> {
    // Validate multipliers are between 0 and 10000 (0.0x to 1.0x)
    for multiplier in new_multipliers.iter() {
        require!(*multiplier <= 10000, MiloError::InvalidFeeBps);
    }

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.tier_multipliers = new_multipliers;
    Ok(())
}

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

/// Harvest withheld fees from Token-2022 mint and distribute to treasury/creator pool
/// This uses Token-2022's withdraw_withheld_tokens_from_mint instruction
#[derive(Accounts)]
pub struct HarvestFees<'info> {
    /// Withdraw authority (must be protocol admin or designated harvester)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state (mint-keyed)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration (for tier multipliers)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// CCM Token-2022 mint (holds withheld fees)
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury token account (receives treasury share)
    #[account(mut)]
    pub treasury: InterfaceAccount<'info, TokenAccount>,

    /// Creator pool token account (receives aggregated creator share)
    #[account(mut)]
    pub creator_pool: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Harvest withheld fees and queue distribution event for keepers
/// The actual Token-2022 withdrawal is performed by keeper bots listening to events
pub fn harvest_and_distribute_fees(ctx: Context<HarvestFees>) -> Result<()> {
    let fee_config = &ctx.accounts.fee_config;
    let ts = Clock::get()?.unix_timestamp;

    // Read mint account data to extract Token-2022 transfer fee extension withheld amount
    // Token-2022 stores withheld fees at a specific offset in the TransferFeeConfig extension
    let mint_info = &ctx.accounts.mint.to_account_info();
    let _mint_data = mint_info.data.borrow();

    // For now, estimate withheld amount from accumulated transfer events
    // In production, properly deserialize TransferFeeConfig extension
    // TODO: Implement proper extension deserialization using spl-token-2022 types
    let withheld_amount = 0u64; // Placeholder; keepers track this from events

      // Calculate treasury and creator pool shares based on a true 50/50 split (non-breaking default)
      // Governance-configurable split will be introduced with a FeeConfig layout migration.
      let treasury_share =
          (withheld_amount as u128 * HARVEST_SPLIT_BPS_TREASURY as u128 / 10000) as u64;
      let creator_pool_share = withheld_amount.saturating_sub(treasury_share);

    // Emit event for keeper bots to consume and execute Token-2022 withdrawals
    // This respects the hybrid architecture: on-chain observes, off-chain executes
    emit!(FeesHarvested {
        mint: ctx.accounts.mint.key(),
        withheld_amount,
        treasury_share,
        creator_pool_share,
        timestamp: ts,
    });

    msg!(
        "Harvest event: withheld_total={}, treasury_allocation={}, creator_allocation={}",
        withheld_amount,
        treasury_share,
        creator_pool_share
    );

    // Keeper bot flow (off-chain):
    // 1. Listen for FeesHarvested events
    // 2. Call spl-token-2022 withdraw_withheld_tokens_from_mint instruction
    // 3. Distribute tokens: treasury_share → treasury_account, creator_pool_share → creator_pool
    // 4. Emit DistributionComplete event for verification

    Ok(())
}
