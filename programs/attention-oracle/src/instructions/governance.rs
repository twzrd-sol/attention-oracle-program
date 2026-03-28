#[cfg(feature = "channel_staking")]
use crate::state::FeeConfig;
use crate::state::ProtocolState;
use crate::{constants::PROTOCOL_SEED, errors::OracleError};
use anchor_lang::prelude::*;
use anchor_spl::token_2022_extensions::transfer_fee::{
    withdraw_withheld_tokens_from_accounts, withdraw_withheld_tokens_from_mint,
    WithdrawWithheldTokensFromAccounts, WithdrawWithheldTokensFromMint,
};
use anchor_spl::token_interface::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

// ============================================================================
// Legacy ProtocolState PDA Layout (173 bytes after realloc)
// ============================================================================
// The old PDA (seeds = [b"protocol", CCM_MINT]) is the mint's withdrawWithheldAuthority.
// It was originally 141 bytes (no oracle_authority field). The realloc_legacy_protocol
// instruction extended it to 173 bytes, inserting oracle_authority at offset 106.
//
// Layout (post-realloc, matches current ProtocolState):
//   [disc: 8][is_init: 1][version: 1][admin: 32][publisher: 32][treasury: 32]
//   [oracle_authority: 32][mint: 32][paused: 1][require_receipt: 1][bump: 1] = 173 bytes
//
// We parse treasury (offset 74) and bump (offset 172) manually because
// HarvestFees uses UncheckedAccount (the PDA is not the declaring program's).

/// Byte offset of the treasury pubkey in the legacy ProtocolState PDA.
const LEGACY_TREASURY_OFFSET: usize = 74;
/// Byte offset of the bump in the legacy ProtocolState PDA (post-realloc).
const LEGACY_BUMP_OFFSET: usize = 172;
/// Minimum account data length for the legacy ProtocolState PDA (post-realloc).
const LEGACY_MIN_LEN: usize = 173;

// ============================================================================
// Initialize Fee Configuration PDA (Phase 2 — Revenue Infrastructure)
// ============================================================================

#[cfg(feature = "channel_staking")]
#[event]
pub struct FeeConfigInitialized {
    pub mint: Pubkey,
    pub basis_points: u16,
    pub treasury_fee_bps: u16,
    pub creator_fee_bps: u16,
    pub timestamp: i64,
}

#[cfg(feature = "channel_staking")]
#[derive(Accounts)]
pub struct InitializeFeeConfig<'info> {
    #[account(
        mut,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub admin: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = admin,
        space = FeeConfig::LEN,
        seeds = [PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
        bump,
    )]
    pub fee_config: Account<'info, FeeConfig>,
    pub system_program: Program<'info, System>,
}

#[cfg(feature = "channel_staking")]
pub fn initialize_fee_config(
    ctx: Context<InitializeFeeConfig>,
    basis_points: u16,
    treasury_fee_bps: u16,
    creator_fee_bps: u16,
    tier_multipliers: [u32; 6],
) -> Result<()> {
    require!(treasury_fee_bps <= 10_000, OracleError::InvalidInputLength);
    require!(creator_fee_bps <= 10_000, OracleError::InvalidInputLength);
    require!(
        treasury_fee_bps + creator_fee_bps <= 10_000,
        OracleError::InvalidInputLength
    );

    let config = &mut ctx.accounts.fee_config;
    config.basis_points = basis_points;
    config.max_fee = 1_000_000_000u64;
    config.drip_threshold = 1_000_000u64;
    config.treasury_fee_bps = treasury_fee_bps;
    config.creator_fee_bps = creator_fee_bps;
    config.tier_multipliers = tier_multipliers;
    config.bump = ctx.bumps.fee_config;

    emit!(FeeConfigInitialized {
        mint: ctx.accounts.mint.key(),
        basis_points,
        treasury_fee_bps,
        creator_fee_bps,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "FeeConfig initialized: mint={}, basis_points={}, treasury_bps={}, creator_bps={}",
        ctx.accounts.mint.key(),
        basis_points,
        treasury_fee_bps,
        creator_fee_bps
    );

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

#[event]
pub struct FeesWithdrawnFromMint {
    pub mint: Pubkey,
    pub withdrawn_amount: u64,
    pub destination: Pubkey,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct HarvestFees<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Legacy ProtocolState PDA (seeds = [PROTOCOL_SEED, mint_key], 173 bytes post-realloc).
    /// This is the mint's withdraw_withheld_authority. We use UncheckedAccount because the
    /// PDA is not the declaring program's — we parse treasury and bump manually.
    /// Safety: PDA address verified via find_program_address in the handler.
    pub protocol_state: UncheckedAccount<'info>,

    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        constraint = treasury.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub treasury: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn harvest_and_distribute_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>,
) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let mint_key = ctx.accounts.mint.key();

    let protocol_data = ctx.accounts.protocol_state.try_borrow_data()?;
    require!(
        protocol_data.len() >= LEGACY_MIN_LEN,
        OracleError::InvalidInputLength
    );

    let treasury_bytes: [u8; 32] = protocol_data
        [LEGACY_TREASURY_OFFSET..LEGACY_TREASURY_OFFSET + 32]
        .try_into()
        .map_err(|_| OracleError::InvalidInputLength)?;
    let treasury = Pubkey::new_from_array(treasury_bytes);
    let bump = protocol_data[LEGACY_BUMP_OFFSET];
    drop(protocol_data);

    let (expected_pda, _bump_check) =
        Pubkey::find_program_address(&[PROTOCOL_SEED, mint_key.as_ref()], ctx.program_id);
    require_keys_eq!(
        ctx.accounts.protocol_state.key(),
        expected_pda,
        OracleError::Unauthorized
    );

    require_keys_eq!(
        ctx.accounts.treasury.owner,
        treasury,
        OracleError::Unauthorized
    );

    require!(
        !ctx.remaining_accounts.is_empty(),
        OracleError::InvalidInputLength
    );
    require!(
        ctx.remaining_accounts.len() <= 30,
        OracleError::InvalidInputLength
    );

    for source_info in ctx.remaining_accounts.iter() {
        require!(
            source_info.owner == &ctx.accounts.token_program.key(),
            OracleError::InvalidTokenProgram
        );
        let data = source_info.try_borrow_data()?;
        require!(data.len() >= 32, OracleError::InvalidTokenProgram);
        let account_mint = Pubkey::new_from_array(
            data[0..32]
                .try_into()
                .map_err(|_| OracleError::InvalidTokenProgram)?,
        );
        require_keys_eq!(account_mint, mint_key, OracleError::InvalidMint);
    }

    let treasury_before = ctx.accounts.treasury.amount;

    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[bump]];
    let signer_seeds = &[seeds];

    let sources: Vec<AccountInfo<'info>> = ctx.remaining_accounts.to_vec();

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

    ctx.accounts.treasury.reload()?;
    let treasury_after = ctx.accounts.treasury.amount;
    let withheld_amount = treasury_after.saturating_sub(treasury_before);

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
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Legacy ProtocolState PDA (seeds = [PROTOCOL_SEED, mint_key], 173 bytes post-realloc).
    /// PDA address verified in handler via find_program_address.
    pub protocol_state: UncheckedAccount<'info>,

    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        constraint = treasury_ata.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn withdraw_fees_from_mint(ctx: Context<WithdrawFeesFromMint>) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let mint_key = ctx.accounts.mint.key();

    let protocol_data = ctx.accounts.protocol_state.try_borrow_data()?;
    require!(
        protocol_data.len() >= LEGACY_MIN_LEN,
        OracleError::InvalidInputLength
    );

    let treasury_bytes: [u8; 32] = protocol_data
        [LEGACY_TREASURY_OFFSET..LEGACY_TREASURY_OFFSET + 32]
        .try_into()
        .map_err(|_| OracleError::InvalidInputLength)?;
    let treasury = Pubkey::new_from_array(treasury_bytes);
    let bump = protocol_data[LEGACY_BUMP_OFFSET];
    drop(protocol_data);

    let (expected_pda, _bump_check) =
        Pubkey::find_program_address(&[PROTOCOL_SEED, mint_key.as_ref()], ctx.program_id);
    require_keys_eq!(
        ctx.accounts.protocol_state.key(),
        expected_pda,
        OracleError::Unauthorized
    );

    require_keys_eq!(
        ctx.accounts.treasury_ata.owner,
        treasury,
        OracleError::Unauthorized
    );

    let treasury_before = ctx.accounts.treasury_ata.amount;

    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[bump]];
    let signer_seeds = &[seeds];

    withdraw_withheld_tokens_from_mint(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        WithdrawWithheldTokensFromMint {
            token_program_id: ctx.accounts.token_program.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            destination: ctx.accounts.treasury_ata.to_account_info(),
            authority: ctx.accounts.protocol_state.to_account_info(),
        },
        signer_seeds,
    ))?;

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
// Treasury Routing (Phase 2 — requires 173-byte live ProtocolState PDA)
// ============================================================================

#[event]
pub struct TreasuryRouted {
    pub mint: Pubkey,
    pub amount: u64,
    pub destination: Pubkey,
    pub treasury_remaining: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct RouteTreasury<'info> {
    #[account(
        mut,
        constraint = (admin.key() == protocol_state.admin || admin.key() == protocol_state.oracle_authority) @ OracleError::Unauthorized,
    )]
    pub admin: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        constraint = treasury_ata.mint == mint.key() @ OracleError::InvalidMint,
        constraint = treasury_ata.owner == protocol_state.key() @ OracleError::Unauthorized,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        constraint = destination_ata.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub destination_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn route_treasury(ctx: Context<RouteTreasury>, amount: u64, min_reserve: u64) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let protocol_state = &ctx.accounts.protocol_state;
    let mint_key = ctx.accounts.mint.key();

    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require!(amount > 0, OracleError::InvalidInputLength);
    require!(min_reserve > 0, OracleError::InvalidInputLength);

    let treasury_balance = ctx.accounts.treasury_ata.amount;
    let balance_after = treasury_balance
        .checked_sub(amount)
        .ok_or(OracleError::InsufficientTreasuryBalance)?;
    require!(
        balance_after >= min_reserve,
        OracleError::InsufficientTreasuryBalance
    );

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

// =============================================================================
// REALLOC LEGACY PROTOCOL STATE PDA
// =============================================================================
// The old ProtocolState PDA (seeds = ["protocol", CCM_MINT]) is 141 bytes — it
// predates the oracle_authority field. RouteTreasury (phase2) uses
// Account<'info, ProtocolState> which needs 173 bytes. This instruction extends
// the legacy PDA and inserts oracle_authority so Anchor can deserialize it.
//
// Data migration:
//   Old layout (141 bytes): disc(8) | init(1) | ver(1) | admin(32) | pub(32) |
//                           treasury(32) | mint(32) | paused(1) | receipt(1) | bump(1)
//   New layout (173 bytes): disc(8) | init(1) | ver(1) | admin(32) | pub(32) |
//                           treasury(32) | oracle_auth(32) | mint(32) | paused(1) | receipt(1) | bump(1)
//
// Steps: realloc → shift [106..141] to [138..173] → write oracle_auth at [106..138]

#[derive(Accounts)]
pub struct ReallocLegacyProtocol<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Live ProtocolState PDA (seeds = ["protocol_state"]) — used to verify admin
    /// and read the oracle_authority value to copy into the legacy PDA.
    #[account(
        seeds = [b"protocol_state"],
        bump = live_protocol_state.bump,
        constraint = admin.key() == live_protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub live_protocol_state: Account<'info, ProtocolState>,

    /// CHECK: Legacy 141-byte ProtocolState PDA (seeds = ["protocol", mint]).
    /// Cannot use Account<ProtocolState> because it's undersized (141 < 173).
    /// PDA address verified via seed constraint.
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump,
    )]
    pub legacy_protocol_state: AccountInfo<'info>,

    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
}

pub fn realloc_legacy_protocol(ctx: Context<ReallocLegacyProtocol>) -> Result<()> {
    let legacy = &ctx.accounts.legacy_protocol_state;
    let current_len = legacy.data_len();
    let target_len = ProtocolState::LEN; // 173

    // Guard: already migrated
    if current_len >= target_len {
        msg!(
            "Legacy PDA already at {} bytes (target {}), no-op",
            current_len,
            target_len
        );
        return Ok(());
    }

    require!(
        current_len == LEGACY_MIN_LEN,
        OracleError::InvalidInputLength
    );

    // Transfer rent difference
    let rent = Rent::get()?;
    let lamports_needed = rent
        .minimum_balance(target_len)
        .saturating_sub(legacy.lamports());

    if lamports_needed > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.admin.to_account_info(),
                    to: legacy.to_account_info(),
                },
            ),
            lamports_needed,
        )?;
    }

    // Resize the account data
    #[allow(deprecated)]
    legacy.realloc(target_len, false)?;

    // Shift mint + flags + bump (35 bytes at [106..141]) → [138..173]
    let mut data = legacy.try_borrow_mut_data()?;
    data.copy_within(106..141, 138);

    // Write oracle_authority at [106..138]
    let oracle_auth = ctx.accounts.live_protocol_state.oracle_authority;
    data[106..138].copy_from_slice(oracle_auth.as_ref());

    msg!(
        "Legacy ProtocolState reallocated: {} -> {} bytes, oracle_authority={}",
        current_len,
        target_len,
        oracle_auth
    );

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
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        has_one = admin,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(mut, address = protocol_state.mint)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Token-2022 program
    /// CHECK: Validated by address
    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: AccountInfo<'info>,
}

pub fn admin_fix_ccm_authority(ctx: Context<AdminFixCcmAuthority>) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let _mint_key = ctx.accounts.mint.key();
    let seeds = &[
        b"protocol_state",
        std::slice::from_ref(&protocol_state.bump),
    ];
    let signer = &[&seeds[..]];

    // Manual CPI to UpdateTransferFeeConfig
    // Layout: [1] extension index 26
    //         [1] sub-instruction index 4
    //         [1] config_authority option (0 = None)
    //         [1] withdraw_withheld_authority option (1 = Some)
    //         [32] withdraw_withheld_authority pubkey
    let mut data = Vec::with_capacity(36);
    data.push(26); // TransferFeeExtension
    data.push(4); // UpdateTransferFeeConfig
    data.push(0); // new_transfer_fee_config_authority: None
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
