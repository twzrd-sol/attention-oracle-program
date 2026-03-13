use crate::{
    constants::PROTOCOL_SEED,
    errors::OracleError,
    events::{ProtocolPaused, PublisherUpdated},
    state::ProtocolState,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

/// Update the allowlisted publisher (keyed by mint)
#[derive(Accounts)]
pub struct UpdatePublisherOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    let old_publisher = state.publisher;
    state.publisher = new_publisher;

    emit!(PublisherUpdated {
        admin: ctx.accounts.admin.key(),
        old_publisher,
        new_publisher,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Emergency pause/unpause (keyed by mint)
#[derive(Accounts)]
pub struct SetPausedOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;

    emit!(ProtocolPaused {
        admin: ctx.accounts.admin.key(),
        paused,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// =============================================================================
// TREASURY WITHDRAW - REMOVED
// =============================================================================
// Admin withdrawal capability was removed after Initial Liquidity Offering.
// Treasury is now locked to claim-based distribution only.
// See: https://solscan.io/tx/L53wKdRPTYKCwR1DJJQjFr34SYsCzjqcyNgXP7BbZAV7Yasz7bDwqP2no6ozm7tLVMawUcADGhZPXRNe4wQajeh
// =============================================================================

// =============================================================================
// SET TREASURY (Fee Destination Owner)
// =============================================================================

#[event]
pub struct TreasuryUpdated {
    pub admin: Pubkey,
    pub old_treasury: Pubkey,
    pub new_treasury: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

/// Update the treasury wallet (fee destination owner).
/// The treasury field stores the OWNER of the fee destination token account.
/// harvest_fees will send withheld fees to ATA(treasury, mint).
#[derive(Accounts)]
pub struct SetTreasury<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
    require!(
        new_treasury != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    let state = &mut ctx.accounts.protocol_state;
    let old_treasury = state.treasury;
    state.treasury = new_treasury;

    emit!(TreasuryUpdated {
        admin: ctx.accounts.admin.key(),
        old_treasury,
        new_treasury,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Treasury updated: {} -> {}", old_treasury, new_treasury);

    Ok(())
}

// =============================================================================
// FIX CCM AUTHORITY
// =============================================================================

#[derive(Accounts)]
pub struct AdminFixCcmAuthority<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(mut, address = protocol_state.mint)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Token-2022 program
    /// CHECK: Validated by address
    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: AccountInfo<'info>,
}

pub fn admin_fix_ccm_authority(ctx: Context<AdminFixCcmAuthority>) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let mint_key = ctx.accounts.mint.key();
    let seeds = &[PROTOCOL_SEED, mint_key.as_ref(), &[protocol_state.bump]];
    let signer = &[&seeds[..]];

    // Manual CPI to UpdateTransferFeeConfig
    // Discriminator for TransferFeeExtension is 26
    // Sub-instruction for UpdateTransferFeeConfig is 4
    let mut data = Vec::with_capacity(36);
    data.push(26); // TransferFeeExtension
    data.push(4); // UpdateTransferFeeConfig
    data.push(0); // new_transfer_fee_config_authority: None (no change)
    data.push(1); // new_withdraw_withheld_authority: Some
    data.extend_from_slice(protocol_state.key().as_ref());

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: anchor_spl::token_2022::ID,
        accounts: vec![
            anchor_lang::solana_program::instruction::AccountMeta::new(
                ctx.accounts.mint.key(),
                false,
            ),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                protocol_state.key(),
                true,
            ),
        ],
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.protocol_state.to_account_info(),
        ],
        signer,
    )?;

    msg!("CCM withdrawal authority fixed to ProtocolState PDA");
    Ok(())
}
