use anchor_lang::prelude::*;
use crate::constants::ADMIN_AUTHORITY;
use crate::errors::OracleError;

// EpochState discriminator: sha256("account:EpochState")[0..8]
// [191, 63, 139, 237, 144, 12, 223, 210]
const EPOCH_STATE_DISCRIMINATOR: [u8; 8] = [0xbf, 0x3f, 0x8b, 0xed, 0x90, 0x0c, 0xdf, 0xd2];

#[derive(Accounts)]
pub struct ForceCloseEpochStateLegacy<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: Manual discriminator/owner check for legacy cleanup
    #[account(mut)]
    pub epoch_state: UncheckedAccount<'info>,
}

pub fn force_close_epoch_state_legacy(
    ctx: Context<ForceCloseEpochStateLegacy>,
    _epoch: u64,
    _subject_id: Pubkey,
) -> Result<()> {
    // 1. Verify Admin (Legacy Hardcoded OR Protocol Admin)
    let is_legacy_admin = ctx.accounts.admin.key() == ADMIN_AUTHORITY;
    require!(is_legacy_admin, OracleError::Unauthorized);

    let account_info = &ctx.accounts.epoch_state;

    // 2. Verify Ownership
    require!(
        account_info.owner == ctx.program_id,
        OracleError::InvalidChannelState
    );

    // 3. Verify Discriminator (EpochState)
    let data = account_info.try_borrow_data()?;
    if data.len() >= 8 {
        require!(
            data[0..8] == EPOCH_STATE_DISCRIMINATOR,
            OracleError::InvalidChannelState
        );
    }
    drop(data);

    // 4. Close Account
    let admin_info = ctx.accounts.admin.to_account_info();
    let lamports = account_info.lamports();
    
    **account_info.try_borrow_mut_lamports()? = 0;
    **admin_info.try_borrow_mut_lamports()? = admin_info
        .lamports()
        .checked_add(lamports)
        .ok_or(OracleError::MathOverflow)?;

    account_info.assign(&System::id());
    account_info.resize(0)?;

    msg!("Force closed legacy EpochState. Reclaimed {} lamports.", lamports);

    Ok(())
}

#[derive(Accounts)]
pub struct ForceCloseChannelStateLegacy<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: Manual discriminator/owner check
    #[account(mut)]
    pub channel_state: UncheckedAccount<'info>,
}

pub fn force_close_channel_state_legacy(
    ctx: Context<ForceCloseChannelStateLegacy>,
    _mint: Pubkey,
    _subject_id: Pubkey,
) -> Result<()> {
    // 1. Verify Admin
    msg!("Invoking force_close_channel_state_legacy");
    let is_legacy_admin = ctx.accounts.admin.key() == ADMIN_AUTHORITY;
    require!(is_legacy_admin, OracleError::Unauthorized);

    let account_info = &ctx.accounts.channel_state;

    // 3. Verify Ownership (Program must own the account)
    require!(
        account_info.owner == ctx.program_id,
        OracleError::InvalidChannelState
    );

    // 4. Close Account
    let admin_info = ctx.accounts.admin.to_account_info();
    let lamports = account_info.lamports();
    
    **account_info.try_borrow_mut_lamports()? = 0;
    **admin_info.try_borrow_mut_lamports()? = admin_info
        .lamports()
        .checked_add(lamports)
        .ok_or(OracleError::MathOverflow)?;

    account_info.assign(&System::id());
    account_info.resize(0)?;

    msg!("Force closed legacy ChannelState. Reclaimed {} lamports.", lamports);

    Ok(())
}
