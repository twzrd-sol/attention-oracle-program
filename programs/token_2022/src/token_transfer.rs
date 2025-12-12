use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::AccountMeta, program::invoke_signed};
use anchor_spl::token_2022::spl_token_2022;

pub fn transfer_checked_with_remaining<'info>(
    token_program: &AccountInfo<'info>,
    from: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let mut ix = spl_token_2022::instruction::transfer_checked(
        token_program.key,
        from.key,
        mint.key,
        to.key,
        authority.key,
        &[],
        amount,
        decimals,
    )?;

    ix.accounts.extend(remaining_accounts.iter().map(|ai| AccountMeta {
        pubkey: *ai.key,
        is_signer: ai.is_signer,
        is_writable: ai.is_writable,
    }));

    let mut account_infos = Vec::with_capacity(4 + remaining_accounts.len());
    account_infos.push(from.clone());
    account_infos.push(mint.clone());
    account_infos.push(to.clone());
    account_infos.push(authority.clone());
    account_infos.extend_from_slice(remaining_accounts);

    invoke_signed(&ix, &account_infos, signer_seeds).map_err(Into::into)
}

