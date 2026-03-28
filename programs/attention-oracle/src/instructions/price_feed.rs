//! Switchboard price feed bridge — permissionless cranker writes validated prices.
//!
//! A registered `updater` reads Switchboard PullFeed via TS SDK, then calls
//! `update_price` to push the value on-chain. The program enforces staleness
//! and deviation guards. Other protocols CPI-read the PriceFeedState PDA.
//!
//! This is a bridge pattern — when Switchboard crate borsh versions align with
//! Anchor 0.32, we can swap to direct PullFeed account parsing.

use anchor_lang::prelude::*;

use crate::errors::OracleError;
use crate::state::{PriceFeedState, ProtocolState};

/// Maximum price deviation between updates: 20% (2000 BPS).
/// Prevents rogue cranker from pushing garbage prices.
const MAX_DEVIATION_BPS: u64 = 2_000;

// =============================================================================
// INITIALIZE PRICE FEED — Admin creates a new price feed PDA
// =============================================================================

#[derive(Accounts)]
#[instruction(label: [u8; 32])]
pub struct InitializePriceFeed<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        has_one = admin,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        init,
        payer = admin,
        space = PriceFeedState::LEN,
        seeds = [b"price_feed" as &[u8], label.as_ref()],
        bump,
    )]
    pub price_feed: Box<Account<'info, PriceFeedState>>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_price_feed(
    ctx: Context<InitializePriceFeed>,
    label: [u8; 32],
    updater: Pubkey,
    max_staleness_slots: u64,
) -> Result<()> {
    let feed = &mut ctx.accounts.price_feed;
    feed.bump = ctx.bumps.price_feed;
    feed.version = 1;
    feed.label = label;
    feed.authority = ctx.accounts.admin.key();
    feed.updater = updater;
    feed.price = 0;
    feed.last_update_slot = 0;
    feed.last_update_ts = 0;
    feed.max_staleness_slots = max_staleness_slots;
    feed.num_updates = 0;

    msg!(
        "PriceFeed initialized. Updater: {}, max_staleness: {} slots",
        updater,
        max_staleness_slots
    );
    Ok(())
}

// =============================================================================
// UPDATE PRICE — Permissionless cranker pushes a Switchboard-sourced price
// =============================================================================

#[derive(Accounts)]
#[instruction(label: [u8; 32])]
pub struct UpdatePrice<'info> {
    #[account(mut)]
    pub updater: Signer<'info>,

    #[account(
        mut,
        seeds = [b"price_feed" as &[u8], label.as_ref()],
        bump = price_feed.bump,
        constraint = price_feed.updater == updater.key() @ OracleError::Unauthorized,
    )]
    pub price_feed: Box<Account<'info, PriceFeedState>>,
}

pub fn update_price(ctx: Context<UpdatePrice>, _label: [u8; 32], price: i64) -> Result<()> {
    require!(price > 0, OracleError::InvalidInputLength);

    let feed = &mut ctx.accounts.price_feed;
    let clock = Clock::get()?;

    // Deviation guard: if we have a previous price, reject updates that deviate > 20%
    if feed.price > 0 {
        let prev = feed.price as u64; // SAFE: guarded by price > 0 check above
        let curr = price as u64; // SAFE: guarded by price > 0 check above
        let diff = if curr > prev {
            curr.saturating_sub(prev)
        } else {
            prev.saturating_sub(curr)
        };
        // diff * 10000 / prev > MAX_DEVIATION_BPS → reject
        let deviation_bps = diff
            .checked_mul(10_000)
            .and_then(|n| n.checked_div(prev))
            .unwrap_or(u64::MAX);
        require!(
            deviation_bps <= MAX_DEVIATION_BPS,
            OracleError::PriceDeviationTooLarge
        );
    }

    feed.price = price;
    feed.last_update_slot = clock.slot;
    feed.last_update_ts = clock.unix_timestamp;
    feed.num_updates = feed.num_updates.saturating_add(1);

    msg!("Price updated: {} (update #{})", price, feed.num_updates);
    Ok(())
}

// =============================================================================
// SET PRICE UPDATER — Authority rotates the cranker key
// =============================================================================

#[derive(Accounts)]
#[instruction(label: [u8; 32])]
pub struct SetPriceUpdater<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"price_feed" as &[u8], label.as_ref()],
        bump = price_feed.bump,
        constraint = price_feed.authority == authority.key() @ OracleError::Unauthorized,
    )]
    pub price_feed: Box<Account<'info, PriceFeedState>>,
}

pub fn set_price_updater(
    ctx: Context<SetPriceUpdater>,
    _label: [u8; 32],
    new_updater: Pubkey,
) -> Result<()> {
    ctx.accounts.price_feed.updater = new_updater;
    msg!("Price updater changed to: {}", new_updater);
    Ok(())
}
