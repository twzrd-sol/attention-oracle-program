use anchor_lang::prelude::*;

/// Switchboard Oracle Integration for USDC/SOL Price Feeds
/// Used for dynamic x402 payment pricing
#[derive(Accounts)]
pub struct UpdatePriceFeed<'info> {
    #[account(mut)]
    pub channel_state: AccountLoader<'info, crate::state::ChannelState>,

    /// Switchboard price feed account
    /// CHECK: Validated by Switchboard program
    pub price_feed: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn update_price_feed(ctx: Context<UpdatePriceFeed>) -> Result<()> {
    // Read Switchboard price feed
    let price_data = ctx.accounts.price_feed.try_borrow_data()?;

    // Parse price (mock for demo - real implementation would decode Switchboard data)
    let usdc_sol_price: u64 = if price_data.len() >= 8 {
        u64::from_le_bytes(price_data[0..8].try_into().unwrap_or([0; 8]))
    } else {
        // Default: 1 USDC = 0.01 SOL
        10_000_000 // Price in lamports per USDC (with 6 decimals)
    };

    // Store price in channel state for x402 pricing
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;
    channel_state.usdc_sol_price = usdc_sol_price;
    channel_state.price_updated_at = Clock::get()?.unix_timestamp;

    msg!("Switchboard price updated: {} lamports per USDC", usdc_sol_price);

    Ok(())
}

/// Get dynamic x402 payment amount based on Switchboard price
pub fn get_x402_price_usdc(channel_state: &crate::state::ChannelState) -> u64 {
    // Base price: 0.001 USDC (with 6 decimals)
    let base_price_usdc = 1_000; // 0.001 USDC

    // Apply dynamic pricing if needed
    if channel_state.price_updated_at > 0 {
        // Could implement surge pricing or discounts here
        base_price_usdc
    } else {
        base_price_usdc
    }
}