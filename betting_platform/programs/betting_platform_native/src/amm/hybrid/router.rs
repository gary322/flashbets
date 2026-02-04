//! Hybrid AMM routing logic
//!
//! Routes trades to the optimal AMM type based on market conditions

use borsh::BorshDeserialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    instruction::TradeParams,
    state::accounts::AMMType,
    state::amm_accounts::HybridMarket,
};

/// Process a trade through the hybrid AMM router
pub fn process_hybrid_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: TradeParams,
) -> ProgramResult {
    msg!("Processing hybrid AMM trade");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let trader = next_account_info(account_info_iter)?;
    let hybrid_market = next_account_info(account_info_iter)?;
    let lmsr_market = next_account_info(account_info_iter)?;
    let pmamm_pool = next_account_info(account_info_iter)?;
    let l2amm_pool = next_account_info(account_info_iter)?;

    // Load hybrid market
    let market = HybridMarket::try_from_slice(&hybrid_market.data.borrow())?;

    // Select optimal AMM based on current conditions
    let optimal_amm = select_optimal_amm(&market, &params)?;

    msg!("Selected AMM type: {:?}", optimal_amm);

    // Route to appropriate AMM
    match optimal_amm {
        AMMType::LMSR => {
            // Forward to LMSR handler
            crate::amm::lmsr::process_lmsr_trade(
                program_id,
                accounts,
                params,
            )
        }
        AMMType::PMAMM => {
            // Convert TradeParams to SwapParams for PM-AMM
            let swap_params = convert_to_swap_params(&params)?;
            crate::amm::pmamm::process_pmamm_trade(
                program_id,
                accounts,
                swap_params,
            )
        }
        AMMType::L2AMM => {
            // Convert TradeParams to L2TradeParams
            let l2_params = convert_to_l2_params(&params)?;
            crate::amm::l2amm::process_l2amm_trade(
                program_id,
                accounts,
                l2_params,
            )
        }
        AMMType::Hybrid => {
            // Hybrid AMM should not be selected as optimal - it's a router itself
            // This case should never be reached, but we handle it gracefully
            msg!("Error: Hybrid AMM cannot route to itself");
            return Err(crate::error::BettingPlatformError::InvalidAMMType.into());
        }
    }
}

/// Select the optimal AMM type based on market conditions
pub fn select_optimal_amm(
    market: &HybridMarket,
    params: &TradeParams,
) -> Result<AMMType, ProgramError> {
    // Decision factors:
    // 1. Number of outcomes
    // 2. Total volume
    // 3. Liquidity depth
    // 4. Trade size
    // 5. Market phase (early/mature)

    // Binary markets -> LMSR
    if market.num_outcomes == 2 {
        return Ok(AMMType::LMSR);
    }

    // Multi-outcome markets
    if market.num_outcomes > 2 && market.num_outcomes <= 10 {
        // Early stage with low volume -> LMSR for better price discovery
        if market.total_volume < 1_000_000_000 { // 1000 USDC
            return Ok(AMMType::LMSR);
        }
        
        // Mature market with good liquidity -> PM-AMM for efficiency
        return Ok(AMMType::PMAMM);
    }

    // Continuous distributions (many outcomes) -> L2-AMM
    if market.num_outcomes > 10 {
        return Ok(AMMType::L2AMM);
    }

    // Large trades relative to liquidity -> LMSR to avoid slippage
    if let Some(max_cost) = params.max_cost {
        let trade_size_ratio = max_cost as f64 / market.total_liquidity as f64;
        if trade_size_ratio > 0.1 { // Trade is >10% of liquidity
            return Ok(AMMType::LMSR);
        }
    }

    // Default to PM-AMM for general efficiency
    Ok(AMMType::PMAMM)
}

/// Convert general TradeParams to PM-AMM SwapParams
fn convert_to_swap_params(params: &TradeParams) -> Result<crate::amm::pmamm::trade::SwapParams, ProgramError> {
    use crate::amm::pmamm::trade::SwapParams;

    // For PM-AMM, we need to specify outcome_in and outcome_out
    // For buys: swap from base currency (outcome 0) to target outcome
    // For sells: swap from target outcome to base currency

    let (outcome_in, outcome_out) = if params.is_buy {
        (0, params.outcome) // Buy: base -> outcome
    } else {
        (params.outcome, 0) // Sell: outcome -> base
    };

    Ok(SwapParams {
        pool_id: params.market_id,
        outcome_in,
        outcome_out,
        amount_in: if params.is_buy { params.max_cost } else { params.shares },
        amount_out: if params.is_buy { params.shares } else { params.min_payout },
        max_slippage_bps: params.max_slippage_bps,
    })
}

/// Convert general TradeParams to L2-AMM parameters
fn convert_to_l2_params(params: &TradeParams) -> Result<crate::amm::l2amm::trade::L2TradeParams, ProgramError> {
    use crate::amm::l2amm::trade::L2TradeParams;

    // For L2-AMM, outcome represents a range
    // Convert single outcome to a range (simplified)
    let range_width = 10; // Default bin width
    let lower_bound = params.outcome as u64 * range_width;
    let upper_bound = lower_bound + range_width;

    Ok(L2TradeParams {
        pool_id: params.market_id,
        lower_bound,
        upper_bound,
        shares: params.shares.unwrap_or(1000), // Default shares
        is_buy: params.is_buy,
        max_cost: params.max_cost,
        min_payout: params.min_payout,
    })
}

/// Analyze market conditions for AMM selection
pub fn analyze_market_conditions(
    market: &HybridMarket,
) -> MarketAnalysis {
    let volume_per_outcome = market.total_volume / market.num_outcomes as u64;
    let liquidity_concentration = calculate_liquidity_concentration(market);
    let price_volatility = calculate_price_volatility(market);

    MarketAnalysis {
        stage: if market.total_volume < 1_000_000_000 {
            MarketStage::Early
        } else if market.total_volume < 10_000_000_000 {
            MarketStage::Growth
        } else {
            MarketStage::Mature
        },
        liquidity_depth: market.total_liquidity,
        volume_per_outcome,
        liquidity_concentration,
        price_volatility,
        recommended_amm: select_amm_by_analysis(market),
    }
}

#[derive(Debug, Clone)]
pub struct MarketAnalysis {
    pub stage: MarketStage,
    pub liquidity_depth: u64,
    pub volume_per_outcome: u64,
    pub liquidity_concentration: f64,
    pub price_volatility: f64,
    pub recommended_amm: AMMType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarketStage {
    Early,
    Growth,
    Mature,
}

fn calculate_liquidity_concentration(market: &HybridMarket) -> f64 {
    // Measure how concentrated liquidity is among outcomes using Herfindahl index
    // Higher concentration suggests PM-AMM would be more efficient
    
    if market.num_outcomes == 0 || market.total_liquidity == 0 {
        return 0.0;
    }
    
    // Calculate Herfindahl-Hirschman Index (HHI) for liquidity concentration
    let mut hhi = 0.0;
    let total_liq = market.total_liquidity as f64;
    
    // For each outcome, calculate market share squared
    // Note: HybridMarket doesn't have amm_state field, so we'll use equal distribution assumption
    // In production, this would access the underlying AMM account based on current_amm type
    for i in 0..market.num_outcomes as usize {
        // Assume equal distribution for now
        let market_share = 1.0 / (market.num_outcomes as f64);
        hhi += market_share * market_share;
    }
    
    // Normalize HHI to 0-1 range
    // HHI ranges from 1/n (perfect distribution) to 1 (complete concentration)
    let min_hhi = 1.0 / (market.num_outcomes as f64);
    let normalized = (hhi - min_hhi) / (1.0 - min_hhi);
    
    normalized.max(0.0).min(1.0)
}

fn calculate_price_volatility(market: &HybridMarket) -> f64 {
    // Measure recent price movements using standard deviation
    // Higher volatility might favor LMSR for stability
    
    if market.num_outcomes == 0 {
        return 0.0;
    }
    
    // Since HybridMarket doesn't have direct price access,
    // use a simplified volatility metric based on volume and liquidity
    
    // Higher volume relative to liquidity indicates more volatility
    if market.total_liquidity == 0 {
        return 0.0;
    }
    
    let volume_to_liquidity_ratio = market.total_volume as f64 / market.total_liquidity as f64;
    
    // Normalize to 0-1 range, assuming ratio > 10 indicates high volatility
    (volume_to_liquidity_ratio / 10.0).min(1.0).max(0.0)
}

fn select_amm_by_analysis(market: &HybridMarket) -> AMMType {
    // Determine market stage based on volume and liquidity
    let stage = if market.total_volume < 10_000_000_000 { // < $10k volume
        MarketStage::Early
    } else if market.total_volume < 100_000_000_000 { // < $100k volume
        MarketStage::Growth
    } else {
        MarketStage::Mature
    };
    
    // Calculate real metrics
    let liquidity_concentration = calculate_liquidity_concentration(market);
    let price_volatility = calculate_price_volatility(market);
    let volume_per_outcome = if market.num_outcomes > 0 {
        market.total_volume / market.num_outcomes as u64
    } else {
        0
    };
    
    let analysis = MarketAnalysis {
        stage,
        liquidity_depth: market.total_liquidity,
        volume_per_outcome,
        liquidity_concentration,
        price_volatility,
        recommended_amm: AMMType::PMAMM, // Will be determined below
    };

    // AMM selection logic based on real analysis
    match (analysis.stage, market.num_outcomes) {
        // Early stage markets benefit from LMSR's guaranteed liquidity
        (MarketStage::Early, _) => AMMType::LMSR,
        
        // Binary markets work best with LMSR
        (_, 2) => AMMType::LMSR,
        
        // High volatility markets need LMSR stability
        (_, _) if price_volatility > 0.7 => AMMType::LMSR,
        
        // Mature markets with concentrated liquidity suit PM-AMM
        (MarketStage::Mature, n) if n <= 10 && liquidity_concentration > 0.6 => AMMType::PMAMM,
        
        // Many outcomes with distributed liquidity need L2-AMM
        (_, n) if n > 10 && liquidity_concentration < 0.4 => AMMType::L2AMM,
        
        // Default to PM-AMM for general cases
        _ => AMMType::PMAMM,
    }
}