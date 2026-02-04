use anchor_lang::prelude::*;
use crate::errors::*;
use crate::events::*;

#[account]
pub struct KeeperHealthPDA {
    pub keeper: Pubkey,
    pub last_update: i64,
    pub markets_processed: u64,
    pub errors_count: u64,
    pub average_latency: u64, // milliseconds
    pub is_healthy: bool,
    pub consecutive_failures: u8,
}

impl KeeperHealthPDA {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8 + 8 + 1 + 1;

    pub fn update_health(&mut self, clock: &Clock) {
        let time_since_update = clock.unix_timestamp - self.last_update;

        // Mark unhealthy if no update for 5 minutes
        if time_since_update > 300 {
            self.is_healthy = false;
            self.consecutive_failures += 1;
        } else {
            self.is_healthy = true;
            self.consecutive_failures = 0;
        }

        self.last_update = clock.unix_timestamp;
    }

    pub fn check_error_rate(&mut self) -> bool {
        // Check if error rate is acceptable
        if self.markets_processed > 0 {
            let error_rate = (self.errors_count as f64) / (self.markets_processed as f64);
            if error_rate > 0.1 {
                // More than 10% error rate
                self.is_healthy = false;
                return false;
            }
        }
        true
    }
}

#[derive(Accounts)]
pub struct InitializeKeeperHealth<'info> {
    #[account(
        init,
        payer = authority,
        space = KeeperHealthPDA::LEN,
        seeds = [b"keeper_health", keeper.key().as_ref()],
        bump
    )]
    pub keeper_health: Account<'info, KeeperHealthPDA>,
    
    pub keeper: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize_keeper_health(
    ctx: Context<InitializeKeeperHealth>,
) -> Result<()> {
    let keeper_health = &mut ctx.accounts.keeper_health;
    
    keeper_health.keeper = ctx.accounts.keeper.key();
    keeper_health.last_update = Clock::get()?.unix_timestamp;
    keeper_health.markets_processed = 0;
    keeper_health.errors_count = 0;
    keeper_health.average_latency = 0;
    keeper_health.is_healthy = true;
    keeper_health.consecutive_failures = 0;
    
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateKeeperHealth<'info> {
    #[account(
        mut,
        seeds = [b"keeper_health", keeper.key().as_ref()],
        bump
    )]
    pub keeper_health: Account<'info, KeeperHealthPDA>,

    pub keeper: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn report_keeper_metrics(
    ctx: Context<UpdateKeeperHealth>,
    markets_processed: u64,
    errors: u64,
    avg_latency: u64,
) -> Result<()> {
    let health = &mut ctx.accounts.keeper_health;
    let clock = &ctx.accounts.clock;

    health.markets_processed += markets_processed;
    health.errors_count += errors;

    // Rolling average for latency
    health.average_latency = (health.average_latency * 9 + avg_latency) / 10;

    health.update_health(clock);

    // Check error rate
    if !health.check_error_rate() {
        msg!("Keeper unhealthy: high error rate");
    }

    // Check latency
    if health.average_latency > 5000 {
        health.is_healthy = false;
        msg!("Keeper unhealthy: high latency");
    }

    emit!(KeeperHealthEvent {
        keeper: ctx.accounts.keeper.key(),
        is_healthy: health.is_healthy,
        metrics: KeeperMetrics {
            markets_processed,
            errors,
            avg_latency,
        },
    });

    Ok(())
}

#[account]
pub struct PerformanceMetricsPDA {
    pub keeper: Pubkey,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_latency: u128, // Sum for average calculation
    pub min_latency: u64,
    pub max_latency: u64,
    pub last_reset: i64,
}

impl PerformanceMetricsPDA {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8 + 16 + 8 + 8 + 8;
}

#[derive(Accounts)]
pub struct InitializePerformanceMetrics<'info> {
    #[account(
        init,
        payer = authority,
        space = PerformanceMetricsPDA::LEN,
        seeds = [b"performance_metrics", keeper.key().as_ref()],
        bump
    )]
    pub metrics: Account<'info, PerformanceMetricsPDA>,
    
    pub keeper: Signer<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize_performance_metrics(
    ctx: Context<InitializePerformanceMetrics>,
) -> Result<()> {
    let metrics = &mut ctx.accounts.metrics;
    
    metrics.keeper = ctx.accounts.keeper.key();
    metrics.total_requests = 0;
    metrics.successful_requests = 0;
    metrics.failed_requests = 0;
    metrics.total_latency = 0;
    metrics.min_latency = u64::MAX;
    metrics.max_latency = 0;
    metrics.last_reset = Clock::get()?.unix_timestamp;
    
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateMetrics<'info> {
    #[account(
        mut,
        seeds = [b"performance_metrics", keeper.key().as_ref()],
        bump
    )]
    pub metrics: Account<'info, PerformanceMetricsPDA>,

    pub keeper: Signer<'info>,
}

pub fn update_performance_metrics(
    ctx: Context<UpdateMetrics>,
    request_count: u64,
    success_count: u64,
    fail_count: u64,
    latencies: Vec<u64>,
) -> Result<()> {
    let metrics = &mut ctx.accounts.metrics;
    let clock = Clock::get()?;

    // Reset daily
    if clock.unix_timestamp - metrics.last_reset > 86400 {
        metrics.total_requests = 0;
        metrics.successful_requests = 0;
        metrics.failed_requests = 0;
        metrics.total_latency = 0;
        metrics.min_latency = u64::MAX;
        metrics.max_latency = 0;
        metrics.last_reset = clock.unix_timestamp;
    }

    metrics.total_requests += request_count;
    metrics.successful_requests += success_count;
    metrics.failed_requests += fail_count;

    for latency in latencies {
        metrics.total_latency += latency as u128;
        metrics.min_latency = metrics.min_latency.min(latency);
        metrics.max_latency = metrics.max_latency.max(latency);
    }

    Ok(())
}

// Keeper coordination state
#[account]
pub struct KeeperCoordinationPDA {
    pub leader: Pubkey,
    pub keeper_count: u8,
    pub last_election: i64,
    pub work_distribution_hash: [u8; 32],
    pub emergency_mode: bool,
}

impl KeeperCoordinationPDA {
    pub const LEN: usize = 8 + 32 + 1 + 8 + 32 + 1;
}

#[derive(Accounts)]
pub struct UpdateCoordination<'info> {
    #[account(
        mut,
        seeds = [b"keeper_coordination"],
        bump
    )]
    pub coordination: Account<'info, KeeperCoordinationPDA>,
    
    pub authority: Signer<'info>,
}

pub fn update_coordination_state(
    ctx: Context<UpdateCoordination>,
    new_leader: Pubkey,
    keeper_count: u8,
    work_distribution_hash: [u8; 32],
) -> Result<()> {
    let coordination = &mut ctx.accounts.coordination;
    
    coordination.leader = new_leader;
    coordination.keeper_count = keeper_count;
    coordination.last_election = Clock::get()?.unix_timestamp;
    coordination.work_distribution_hash = work_distribution_hash;
    
    Ok(())
}

pub fn toggle_emergency_mode(
    ctx: Context<UpdateCoordination>,
) -> Result<()> {
    let coordination = &mut ctx.accounts.coordination;
    
    coordination.emergency_mode = !coordination.emergency_mode;
    
    msg!("Emergency mode: {}", coordination.emergency_mode);
    
    Ok(())
}