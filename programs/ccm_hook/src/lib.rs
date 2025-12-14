use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
use spl_tlv_account_resolution::state::ExtraAccountMetaList;
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

declare_id!("8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS");

/// CCM Transfer Hook Program
///
/// Implements the Token-2022 Transfer Hook interface for CCM token.
/// Currently logs all transfers; can be extended for fee collection.
#[program]
pub mod ccm_hook {
    use super::*;

    /// Initialize the extra account metas for the transfer hook.
    /// Must be called once per mint to set up the hook.
    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        // For now, we don't need any extra accounts
        // This can be extended to include treasury, protocol state, etc.
        let account_metas = vec![
            // Example: Add treasury ATA for fee collection
            // ExtraAccountMeta::new_with_seeds(
            //     &[
            //         Seed::Literal { bytes: b"treasury".to_vec() },
            //         Seed::AccountKey { index: 1 }, // mint
            //     ],
            //     false, // is_signer
            //     true,  // is_writable
            // )?,
        ];

        // Calculate space needed
        let account_size = ExtraAccountMetaList::size_of(account_metas.len())?;

        // Initialize the extra account meta list
        let extra_account_metas = &ctx.accounts.extra_account_meta_list;

        // Validate size
        require!(
            extra_account_metas.data_len() >= account_size,
            HookError::AccountTooSmall
        );

        // Write the account metas
        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut extra_account_metas.try_borrow_mut_data()?,
            &account_metas,
        )?;

        msg!("CCM Hook: Extra account metas initialized");
        Ok(())
    }

    /// Transfer hook - called on every CCM transfer.
    ///
    /// This is called by the Token-2022 program after each transfer.
    /// Currently logs the transfer; can be extended for:
    /// - Fee collection (requires PermanentDelegate or pre-approval)
    /// - Transfer validation/blocking
    /// - Analytics/metrics
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        let source = &ctx.accounts.source_token;
        let destination = &ctx.accounts.destination_token;
        let mint = &ctx.accounts.mint;

        // Log the transfer
        msg!(
            "CCM Transfer: {} tokens from {} to {}",
            amount,
            source.key(),
            destination.key()
        );

        // Emit event for off-chain indexing
        emit!(TransferEvent {
            mint: mint.key(),
            source: source.key(),
            destination: destination.key(),
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });

        // Future: Fee collection logic would go here
        // This would require:
        // 1. PermanentDelegate on the mint, OR
        // 2. Pre-approved allowance from users
        //
        // let fee = amount / 100; // 1%
        // if fee > 0 {
        //     // CPI to transfer fee from destination to treasury
        // }

        Ok(())
    }

    /// Fallback instruction for interface compatibility
    pub fn fallback<'info>(
        _program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        // Parse amount from instruction data (8 byte discriminator + 8 byte amount)
        if data.len() < 16 {
            return Err(HookError::InvalidInstruction.into());
        }
        let amount_bytes: [u8; 8] = data[8..16]
            .try_into()
            .map_err(|_| HookError::InvalidInstruction)?;
        let amount = u64::from_le_bytes(amount_bytes);

        // Log for debugging
        msg!("CCM Hook fallback: amount={}", amount);

        // Validate accounts
        require!(accounts.len() >= 5, HookError::NotEnoughAccounts);

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The mint that this hook is for
    #[account(mint::token_program = anchor_spl::token_interface::spl_token_2022::ID)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// The extra account meta list PDA
    /// Seeds: ["extra-account-metas", mint]
    #[account(
        init,
        payer = payer,
        space = ExtraAccountMetaList::size_of(0)? + 8, // No extra accounts for now
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
    )]
    /// CHECK: Validated by seeds
    pub extra_account_meta_list: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TransferHook<'info> {
    /// Source token account
    #[account(token::token_program = anchor_spl::token_interface::spl_token_2022::ID)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    /// The CCM mint
    #[account(mint::token_program = anchor_spl::token_interface::spl_token_2022::ID)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Destination token account
    #[account(token::token_program = anchor_spl::token_interface::spl_token_2022::ID)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    /// Source token owner
    /// CHECK: Validated by token account
    pub owner: UncheckedAccount<'info>,

    /// Extra account metas PDA
    /// CHECK: Validated by seeds
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
}

#[event]
pub struct TransferEvent {
    pub mint: Pubkey,
    pub source: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum HookError {
    #[msg("Account is too small")]
    AccountTooSmall,
    #[msg("Not enough accounts provided")]
    NotEnoughAccounts,
    #[msg("Invalid instruction data")]
    InvalidInstruction,
}
