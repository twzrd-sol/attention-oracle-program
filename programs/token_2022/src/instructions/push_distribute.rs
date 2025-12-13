use anchor_lang::{
    accounts::account_loader::AccountLoader,
    prelude::*,
    solana_program::{program::invoke_signed, rent::Rent, system_instruction},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use sha3::{Digest, Keccak256};

use crate::{
    constants::{CHANNEL_STATE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    state::{ChannelState, ProtocolState},
};

pub const MAX_BATCH_SIZE: usize = 20;
const FLAG_SEED: &[u8] = b"push_flag";
const FLAG_SPACE: usize = 1;

#[derive(Accounts)]
#[instruction(channel: String, batch_idx: u32)]
pub struct PushDistribute<'info> {
    #[account(mut)]
    pub publisher: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
        constraint = protocol_state.publisher == publisher.key() @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,
}

fn derive_subject_id(channel: &str) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(b"channel:");
    hasher.update(channel.to_lowercase());
    let digest = hasher.finalize();
    let mut subject = [0u8; 32];
    subject.copy_from_slice(&digest[..32]);
    subject
}

pub fn push_distribute<'info>(
    ctx: Context<'_, '_, 'info, 'info, PushDistribute<'info>>,
    recipients: Vec<Pubkey>,
    amounts: Vec<u64>,
    epoch: u64,
    channel: String,
    batch_idx: u32,
) -> Result<()> {
    require!(recipients.len() == amounts.len(), OracleError::InvalidInputLength);
    require!(
        !recipients.is_empty() && recipients.len() <= MAX_BATCH_SIZE,
        OracleError::InvalidInputLength
    );

    let total_amount: u64 = amounts.iter().sum();
    require!(
        ctx.accounts.treasury_ata.amount >= total_amount,
        OracleError::InsufficientTreasuryBalance
    );

    let remaining = ctx.remaining_accounts;
    let base_remaining = recipients.len() + 1;
    require!(
        remaining.len() >= base_remaining,
        OracleError::InvalidInputLength
    );
    let flag_account = &remaining[0];
    let recipient_atas = &remaining[1..base_remaining];
    let hook_accounts = &remaining[base_remaining..];

    let subject_id = derive_subject_id(&channel);
    let channel_state_seeds = [
        CHANNEL_STATE_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        subject_id.as_ref(),
    ];
    let (expected_channel_state, _) = Pubkey::find_program_address(&channel_state_seeds, ctx.program_id);
    require_keys_eq!(
        expected_channel_state,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    let channel_state = ctx.accounts.channel_state.load()?;
    let slot = channel_state.slot(epoch);
    require!(slot.epoch == epoch, OracleError::SlotMismatch);
    require!(slot.root != [0u8; 32], OracleError::InvalidRoot);

    let mint_key = ctx.accounts.mint.key();
    let flag_seeds = [
        FLAG_SEED,
        mint_key.as_ref(),
        subject_id.as_ref(),
        &epoch.to_le_bytes(),
        &batch_idx.to_le_bytes(),
    ];
    let (flag_pda, flag_bump) = Pubkey::find_program_address(&flag_seeds, ctx.program_id);
    require_keys_eq!(flag_pda, flag_account.key(), OracleError::InvalidInputLength);
    if flag_account.lamports() > 0 {
        return Err(OracleError::AlreadyPushed.into());
    }

    let rent = Rent::get()?;
    invoke_signed(
        &system_instruction::create_account(
            &ctx.accounts.publisher.key(),
            flag_account.key,
            rent.minimum_balance(FLAG_SPACE),
            FLAG_SPACE as u64,
            ctx.program_id,
        ),
        &[
            ctx.accounts.publisher.to_account_info(),
            flag_account.clone(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&[
            FLAG_SEED,
            ctx.accounts.mint.key().as_ref(),
            subject_id.as_ref(),
            &epoch.to_le_bytes(),
            &batch_idx.to_le_bytes(),
            &[flag_bump],
        ]],
    )?;
    {
        let mut data = flag_account.try_borrow_mut_data()?;
        if let Some(first) = data.first_mut() {
            *first = 1;
        }
    }

    let mint_key = ctx.accounts.protocol_state.mint;
    let bump = ctx.accounts.protocol_state.bump;
    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[bump]];
    let signer_seeds = &[seeds];
    let decimals = ctx.accounts.mint.decimals;
    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    for (i, amount) in amounts.iter().enumerate() {
        if *amount == 0 {
            continue;
        }

        let recipient_ata = &recipient_atas[i];

        crate::transfer_checked_with_remaining(
            &token_program,
            &from,
            &mint,
            recipient_ata,
            &authority,
            *amount,
            decimals,
            signer_seeds,
            hook_accounts,
        )?;

        emit!(PushDistributeEvent {
            recipient: recipients[i],
            amount: *amount,
            epoch,
            channel: channel.clone(),
        });
    }

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
