use anchor_lang::prelude::*;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

#[program]
pub mod attention_oracle_program {
    use super::*;

    // ========================================================================
    // ADMIN / ENFORCER LOGIC (NEW)
    // ========================================================================

    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        let config = &mut ctx.accounts.enforcer_config;
        config.authority = ctx.accounts.authority.key();
        config.window_start = 0;
        config.window_duration = 0;
        msg!("Enforcer Config Initialized");
        Ok(())
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        window_start: i64,
        window_duration: i64
    ) -> Result<()> {
        let config = &mut ctx.accounts.enforcer_config;
        // Optional: Check authority if you want to restrict who can update windows
        // require!(config.authority == ctx.accounts.authority.key(), AttentionError::Unauthorized);

        config.window_start = window_start;
        config.window_duration = window_duration;

        msg!("Golden Window Updated: Start={}, Duration={}", window_start, window_duration);
        Ok(())
    }

    // ========================================================================
    // ORACLE LOGIC (EXISTING - PRESERVED)
    // ========================================================================

    /// Submit event from aggregator
    pub fn submit_event(
        ctx: Context<SubmitEvent>,
        points: u64,
    ) -> Result<()> {
        let node_score = &mut ctx.accounts.node_score;
        let clock = Clock::get()?;

        // Check authorization (Hardcoded Aggregator Key as provided)
        let expected_authority = "2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD"
            .parse::<Pubkey>()
            .unwrap();
        require!(
            ctx.accounts.authority.key() == expected_authority,
            AttentionError::Unauthorized
        );

        // Add score
        node_score.score = node_score.score.checked_add(points).ok_or(AttentionError::Overflow)?;
        node_score.last_update = clock.unix_timestamp;

        emit!(ScoreUpdated {
            user: ctx.accounts.user.key(),
            points,
            new_total: node_score.score,
        });

        msg!("Score updated: {} points added. Total: {}", points, node_score.score);
        Ok(())
    }
}

// ============================================================================
// ACCOUNTS
// ============================================================================

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 8 + 8, // disc + pubkey + i64 + i64
        seeds = [b"enforcer_config"],
        bump
    )]
    pub enforcer_config: Account<'info, EnforcerConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"enforcer_config"],
        bump
    )]
    pub enforcer_config: Account<'info, EnforcerConfig>,
}

#[derive(Accounts)]
pub struct SubmitEvent<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: User to update
    pub user: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + 8 + 8,  // disc + score + last_update
        seeds = [b"node_score", user.key().as_ref()],
        bump
    )]
    pub node_score: Account<'info, NodeScore>,

    pub system_program: Program<'info, System>,
}

// ============================================================================
// STATE
// ============================================================================

// NEW: Configuration for the Golden Window
#[account]
pub struct EnforcerConfig {
    pub authority: Pubkey,
    pub window_start: i64,
    pub window_duration: i64,
}

// EXISTING: Critical to keep name 'NodeScore' to preserve Account Discriminator
#[account]
pub struct NodeScore {
    pub score: u64,
    pub last_update: i64,
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct ScoreUpdated {
    pub user: Pubkey,
    pub points: u64,
    pub new_total: u64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum AttentionError {
    #[msg("Not authorized")]
    Unauthorized,
    #[msg("Overflow")]
    Overflow,
}
