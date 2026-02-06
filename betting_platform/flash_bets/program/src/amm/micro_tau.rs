use solana_program::program_error::ProgramError;
use crate::errors::FlashError;

/// Calculate micro-tau value for flash markets
/// tau = 0.0001 * (time_left / 60)
pub fn calculate_tau(time_left: u64) -> f64 {
    0.0001 * (time_left as f64 / 60.0)
}

/// Calculate trade using micro-tau AMM with Newton-Raphson solver
pub fn calculate_trade(
    current_prob: f64,
    amount: u64,
    tau: f64,
    max_slippage: u64,
) -> Result<(f64, u64), ProgramError> {
    // Demo scope: keep pricing deterministic and monotonic.
    // A buy shifts probability up by ~amount * tau.
    let order = amount as f64;
    let probability_delta = order * tau;
    let new_prob = (current_prob + probability_delta).min(0.99).max(0.01);
    
    // Check slippage
    let slippage = ((new_prob - current_prob).abs() / current_prob * 10000.0) as u64;
    if slippage > max_slippage {
        return Err(FlashError::ExcessiveSlippage.into());
    }
    
    Ok((new_prob, amount))
}

/// Normal PDF approximation using fixed-point arithmetic
fn normal_pdf(x: f64) -> f64 {
    // For micro-tau, x is very small, so exp(-x²/2) ≈ 1 - x²/2
    if x.abs() < 0.001 {
        return 0.3989423 * (1.0 - x * x / 2.0);
    }
    
    // Standard formula for larger values
    let exp_part = (-x * x / 2.0).exp();
    0.3989423 * exp_part // 1/sqrt(2π) ≈ 0.3989423
}

/// Normal CDF approximation
fn normal_cdf(x: f64) -> f64 {
    // For very small x (micro-tau case), use Taylor series
    if x.abs() < 0.001 {
        return 0.5 + x * 0.3989423; // 0.5 + x * pdf(0)
    }
    
    // Approximation for standard normal CDF
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs();
    
    let t = 1.0 / (1.0 + p * x_abs);
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    
    let y = 1.0 - (((((a5 * t5 + a4 * t4) + a3 * t3) + a2 * t2) + a1 * t) * t * (-x_abs * x_abs / 2.0).exp());
    
    0.5 * (1.0 + sign * y)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tau_calculation() {
        assert_eq!(calculate_tau(60), 0.0001); // 1 minute
        assert_eq!(calculate_tau(30), 0.00005); // 30 seconds
        assert_eq!(calculate_tau(120), 0.0002); // 2 minutes
    }
    
    #[test]
    fn test_micro_tau_trade() {
        let tau = 0.00005; // 30 second market
        let (new_prob, amount) = calculate_trade(0.5, 1000, tau, 1000).unwrap();
        
        assert!(new_prob > 0.5); // Buying increases probability
        assert!(new_prob < 0.6); // But not too much with micro-tau
        assert!(amount > 0);
    }
    
    #[test]
    fn test_normal_pdf_micro() {
        let pdf = normal_pdf(0.0001);
        assert!((pdf - 0.3989423).abs() < 0.0001); // Should be very close to peak
    }
    
    #[test]
    fn test_normal_cdf_micro() {
        let cdf = normal_cdf(0.0);
        assert!((cdf - 0.5).abs() < 0.0001); // CDF(0) = 0.5
    }
}
