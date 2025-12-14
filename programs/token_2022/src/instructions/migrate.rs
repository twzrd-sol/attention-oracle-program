use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, Burn, MintTo, Token2022};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::constants::PROTOCOL_SEED;

// CCM-v1 mint (no TransferFeeConfig)
pub const OLD_CCM_MINT: Pubkey = Pubkey::new_from_array([
    0xc7, 0xc5, 0x48, 0x77, 0x0e, 0xbf, 0x19, 0x21,
    0x48, 0x86, 0x59, 0x78, 0xf9, 0x0e, 0xc6, 0x97,
    0xf0, 0xca, 0xeb, 0xae, 0xe2, 0xfa, 0xbf, 0x8c,
    0xac, 0x05, 0x9d, 0xd0, 0xa8, 0xc1, 0xce, 0x5b,
]); // ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe

// CCM-v2 mint (with TransferFeeConfig 50 bps)
pub const NEW_CCM_MINT: Pubkey = Pubkey::new_from_array([
    0x02, 0xcd, 0xbc, 0xc2, 0xe1, 0x58, 0x38, 0x91,
    0x52, 0x22, 0x99, 0xb0, 0xd2, 0x18, 0x8b, 0xdb,
    0x1b, 0x88, 0xd8, 0x49, 0xae, 0x11, 0x5d, 0xfb,
    0x76, 0x15, 0x6c, 0x1f, 0x4b, 0x90, 0x8a, 0xc2,
]); // Bwmh8UfYuUEh31gYuxgBRGct4jCut6TkpfnB6ba5MbF

/// Migrate CCM-v1 to CCM-v2 at 1:1 ratio
///
/// Burns v1 tokens from user, mints v2 tokens to user.
/// Protocol state PDA is mint authority for v2.
#[derive(Accounts)]
pub struct Migrate<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CCM-v1 mint to burn from
    #[account(
        mut,
        address = OLD_CCM_MINT @ MigrateError::InvalidOldMint,
    )]
    pub old_mint: InterfaceAccount<'info, Mint>,

    /// CCM-v2 mint to mint to
    #[account(
        mut,
        address = NEW_CCM_MINT @ MigrateError::InvalidNewMint,
    )]
    pub new_mint: InterfaceAccount<'info, Mint>,

    /// User's CCM-v1 token account (source)
    #[account(
        mut,
        token::mint = old_mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_old_ata: InterfaceAccount<'info, TokenAccount>,

    /// User's CCM-v2 token account (destination)
    #[account(
        mut,
        token::mint = new_mint,
        token::token_program = token_program,
    )]
    pub user_new_ata: InterfaceAccount<'info, TokenAccount>,

    /// Protocol state PDA - mint authority for CCM-v2
    /// CHECK: Seeds validated, used only as signer
    #[account(
        seeds = [PROTOCOL_SEED, new_mint.key().as_ref()],
        bump,
    )]
    pub mint_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token2022>,
}

#[error_code]
pub enum MigrateError {
    #[msg("Invalid old mint - must be CCM-v1")]
    InvalidOldMint,

    #[msg("Invalid new mint - must be CCM-v2")]
    InvalidNewMint,

    #[msg("Amount must be greater than zero")]
    ZeroAmount,
}

#[event]
pub struct MigrationEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub old_mint: Pubkey,
    pub new_mint: Pubkey,
    pub timestamp: i64,
}

pub fn migrate(ctx: Context<Migrate>, amount: u64) -> Result<()> {
    require!(amount > 0, MigrateError::ZeroAmount);

    let user = &ctx.accounts.user;
    let old_mint = &ctx.accounts.old_mint;
    let new_mint = &ctx.accounts.new_mint;
    let user_old_ata = &ctx.accounts.user_old_ata;
    let user_new_ata = &ctx.accounts.user_new_ata;
    let mint_authority = &ctx.accounts.mint_authority;
    let token_program = &ctx.accounts.token_program;

    // 1. Burn CCM-v1 from user
    token_2022::burn(
        CpiContext::new(
            token_program.to_account_info(),
            Burn {
                mint: old_mint.to_account_info(),
                from: user_old_ata.to_account_info(),
                authority: user.to_account_info(),
            },
        ),
        amount,
    )?;

    // 2. Mint CCM-v2 to user (1:1)
    let new_mint_key = new_mint.key();
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        new_mint_key.as_ref(),
        &[ctx.bumps.mint_authority],
    ];

    token_2022::mint_to(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            MintTo {
                mint: new_mint.to_account_info(),
                to: user_new_ata.to_account_info(),
                authority: mint_authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    // 3. Emit event
    emit!(MigrationEvent {
        user: user.key(),
        amount,
        old_mint: old_mint.key(),
        new_mint: new_mint.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Migrated {} CCM from v1 to v2 for user {}",
        amount,
        user.key()
    );

    Ok(())
}
