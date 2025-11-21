use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, seeds::Seed, state::ExtraAccountMetaList};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: ExtraAccountMetaList PDA - initialized by this instruction
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = 8 + ExtraAccountMetaList::size_of(3).unwrap(),
        payer = payer
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,

    /// The mint that will use this transfer hook
    pub mint: InterfaceAccount<'info, Mint>,

    pub system_program: Program<'info, System>,
}

/// Initialize the ExtraAccountMetaList for the transfer hook
///
/// This tells Token-2022 which additional accounts to pass to the transfer_hook instruction.
/// Required for the hook to access protocol_state, fee_config, and other PDAs.
pub fn initialize_extra_account_meta_list(
    ctx: Context<InitializeExtraAccountMetaList>,
) -> Result<()> {
    // Define the extra accounts that the transfer hook needs
    // These will be automatically added to every transfer_checked call
    let account_metas = vec![
        // Account 0: protocol_state PDA
        // Seeds: ["protocol", mint]
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal {
                    bytes: b"protocol".to_vec(),
                },
                Seed::AccountKey { index: 2 }, // Index 2 = mint in transfer_checked
            ],
            false, // is_signer
            false, // is_writable
        )?,
        // Account 1: fee_config PDA
        // Seeds: ["protocol", mint, "fee_config"]
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal {
                    bytes: b"protocol".to_vec(),
                },
                Seed::AccountKey { index: 2 }, // Index 2 = mint
                Seed::Literal {
                    bytes: b"fee_config".to_vec(),
                },
            ],
            false, // is_signer
            false, // is_writable
        )?,
        // Account 2: system_program
        ExtraAccountMeta::new_with_pubkey(
            &anchor_lang::system_program::ID,
            false, // is_signer
            false, // is_writable
        )?,
    ];

    // Get mutable reference to the account data
    let account_data = &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?;

    // Initialize the ExtraAccountMetaList in the account data
    ExtraAccountMetaList::init::<ExecuteInstruction>(account_data, &account_metas)?;

    msg!("✅ ExtraAccountMetaList initialized for mint: {}", ctx.accounts.mint.key());
    msg!("   Extra accounts: {}", account_metas.len());

    Ok(())
}
