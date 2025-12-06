use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::PROTOCOL_SEED,
    errors::OracleError,
    state::ProtocolState,
};

/// Maximum recipients per batch (CU-safe limit)
pub const MAX_BATCH_SIZE: usize = 20;

/// Push-distribute CCM tokens to multiple recipients in a single TX.
///
/// Requirements:
/// - Caller must be the protocol's publisher
/// - All recipient ATAs must already exist (no init_if_needed to save CU)
/// - Total amount must not exceed treasury balance
///
/// This is a trusted operation - no merkle proof required.
/// Publisher is responsible for computing correct amounts off-chain.
#[derive(Accounts)]
pub struct PushDistribute<'info> {
    /// Publisher (must match protocol_state.publisher)
    #[account(mut)]
    pub publisher: Signer<'info>,

    /// Protocol state - authority over treasury
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
        constraint = protocol_state.publisher == publisher.key() @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CCM mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury ATA (source of tokens)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    // Note: recipient ATAs passed as remaining_accounts
}

/// Push-distribute to multiple recipients.
///
/// # Arguments
/// * `recipients` - List of recipient wallet pubkeys
/// * `amounts` - Corresponding amounts (must match recipients length)
/// * `epoch` - Epoch this distribution is for (for event logging)
/// * `channel` - Channel name (for event logging)
pub fn push_distribute<'info>(
    ctx: Context<'_, '_, 'info, 'info, PushDistribute<'info>>,
    recipients: Vec<Pubkey>,
    amounts: Vec<u64>,
    epoch: u64,
    channel: String,
) -> Result<()> {
    // Validate batch
    require!(
        recipients.len() == amounts.len(),
        OracleError::InvalidInputLength
    );
    require!(
        !recipients.is_empty() && recipients.len() <= MAX_BATCH_SIZE,
        OracleError::InvalidInputLength
    );

    let total_amount: u64 = amounts.iter().sum();
    require!(
        ctx.accounts.treasury_ata.amount >= total_amount,
        OracleError::InsufficientTreasuryBalance
    );

    // Recipient ATAs are in remaining_accounts
    let recipient_atas = ctx.remaining_accounts;
    require!(
        recipient_atas.len() == recipients.len(),
        OracleError::InvalidInputLength
    );

    // Build signer seeds for PDA authority
    let mint_key = ctx.accounts.protocol_state.mint;
    let bump = ctx.accounts.protocol_state.bump;
    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[bump]];
    let signer_seeds = &[seeds];

    let decimals = ctx.accounts.mint.decimals;

    // Execute transfers
    for (i, amount) in amounts.iter().enumerate() {
        if *amount == 0 {
            continue; // Skip zero amounts
        }

        let recipient_ata = &recipient_atas[i];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.treasury_ata.to_account_info(),
                    to: recipient_ata.to_account_info(),
                    authority: ctx.accounts.protocol_state.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                signer_seeds,
            ),
            *amount,
            decimals,
        )?;

        // Emit per-recipient event for indexing
        emit!(PushDistributeEvent {
            recipient: recipients[i],
            amount: *amount,
            epoch,
            channel: channel.clone(),
        });
    }

    // Emit batch summary
    emit!(PushDistributeBatch {
        publisher: ctx.accounts.publisher.key(),
        recipients_count: recipients.len() as u32,
        total_amount,
        epoch,
        channel,
    });

    Ok(())
}

#[event]
pub struct PushDistributeEvent {
    pub recipient: Pubkey,
    pub amount: u64,
    pub epoch: u64,
    pub channel: String,
}

#[event]
pub struct PushDistributeBatch {
    pub publisher: Pubkey,
    pub recipients_count: u32,
    pub total_amount: u64,
    pub epoch: u64,
    pub channel: String,
}
