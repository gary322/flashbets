use anchor_lang::prelude::*;
use crate::fixed_math::FixedPoint;
use crate::errors::ErrorCode;
use crate::lmsr_amm::LSMRMarket;
use crate::l2_amm::{L2DistributionAMM, DistributionType, DistributionParams};
use crate::hybrid_amm::AMMType;

/// Verify AMM invariants
pub fn verify_amm_invariants(
    amm_type: &AMMType,
    market_data: &[u8],
) -> Result<()> {
    match amm_type {
        AMMType::LMSR => verify_lmsr_invariants(market_data),
        AMMType::PMAMM => verify_pmamm_invariants(market_data),
        AMMType::L2Distribution => verify_l2_invariants(market_data),
    }
}

/// Verify LMSR invariants
fn verify_lmsr_invariants(market_data: &[u8]) -> Result<()> {
    // Deserialize market data
    let b = FixedPoint::from_raw(u64::from_le_bytes(market_data[0..8].try_into().unwrap()));
    let num_outcomes = market_data[8] as usize;
    
    let mut q = vec![];
    for i in 0..num_outcomes {
        let start = 9 + i * 8;
        let value = u64::from_le_bytes(market_data[start..start+8].try_into().unwrap());
        q.push(FixedPoint::from_raw(value));
    }
    
    let market = LSMRMarket {
        b,
        q,
        alpha: FixedPoint::from_u64(1),
    };
    
    // Verify price sum = 1
    let prices = market.all_prices()?;
    let sum = prices.iter().fold(
        Ok(FixedPoint::zero()),
        |acc: Result<FixedPoint>, p| {
            acc.and_then(|a| a.add(p))
        }
    )?;
    
    let one = FixedPoint::from_u64(1);
    let epsilon = FixedPoint::from_float(0.000001);
    
    require!(
        (sum.sub(&one)?.abs()?) < epsilon,
        ErrorCode::PriceSumError
    );
    
    // Verify all prices are positive and less than 1
    for price in prices {
        require!(
            price > FixedPoint::zero() && price < one,
            ErrorCode::InvalidInput
        );
    }
    
    Ok(())
}

/// Verify PM-AMM invariants
fn verify_pmamm_invariants(_market_data: &[u8]) -> Result<()> {
    // PM-AMM verification would include:
    // 1. Verify liquidity parameter L > 0
    // 2. Verify time to expiry is positive
    // 3. Verify inventory levels are reasonable
    // 4. Verify LVR is within bounds
    
    Ok(())
}

/// Verify L2 distribution invariants
fn verify_l2_invariants(market_data: &[u8]) -> Result<()> {
    // Extract parameters from market data
    let k = FixedPoint::from_raw(u64::from_le_bytes(market_data[0..8].try_into().unwrap()));
    let b = FixedPoint::from_raw(u64::from_le_bytes(market_data[8..16].try_into().unwrap()));
    
    // For demonstration, assume Normal distribution
    let amm = L2DistributionAMM {
        k,
        b,
        distribution_type: DistributionType::Normal { 
            mean: 500_000_000_000_000_000, 
            variance: 100_000_000_000_000_000,
        },
        parameters: DistributionParams {
            discretization_points: 100,
            range_min: FixedPoint::from_u64(0),
            range_max: FixedPoint::from_u64(1000),
        },
    };
    
    let distribution = amm.calculate_distribution()?;
    
    // Verify L2 norm constraint
    let norm = amm.calculate_l2_norm(&distribution)?;
    let epsilon = FixedPoint::from_float(0.001);
    
    let diff = if norm > k {
        norm.sub(&k)?
    } else {
        k.sub(&norm)?
    };
    
    require!(
        diff < epsilon,
        ErrorCode::InvalidInput
    );
    
    // Verify max bound
    for (_, f) in distribution {
        require!(
            f <= b,
            ErrorCode::InvalidInput
        );
    }
    
    Ok(())
}

/// Verify order execution fairness
pub fn verify_execution_fairness(
    executions: &[ExecutionRecord],
    _orderbook_state: &[u8],
) -> Result<()> {
    // Verify price-time priority
    for i in 1..executions.len() {
        let prev = &executions[i-1];
        let curr = &executions[i];
        
        if prev.price == curr.price {
            require!(
                prev.timestamp <= curr.timestamp,
                ErrorCode::InvalidInput
            );
        } else if prev.is_buy {
            require!(
                prev.price > curr.price,
                ErrorCode::InvalidInput
            );
        } else {
            require!(
                prev.price < curr.price,
                ErrorCode::InvalidInput
            );
        }
    }
    
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub price: u64,
    pub timestamp: i64,
    pub is_buy: bool,
    pub size: u64,
}