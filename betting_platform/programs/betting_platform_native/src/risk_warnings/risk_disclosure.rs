//! Risk disclosure text and hash management
//!
//! Provides the risk disclosure content that users must acknowledge

use solana_program::keccak;

/// Get the risk disclosure text
pub fn get_risk_disclosure_text() -> &'static str {
    include_str!("../../docs/RISK_DISCLOSURE.md")
}

/// Get the hash of the risk disclosure for verification
pub fn get_risk_disclosure_hash() -> [u8; 32] {
    let disclosure = get_risk_disclosure_text();
    keccak::hash(disclosure.as_bytes()).to_bytes()
}

/// Risk disclosure sections for UI display
pub struct RiskDisclosureSections {
    pub leverage_amplifies_losses: &'static str,
    pub liquidation_risk: &'static str,
    pub funding_rate_costs: &'static str,
    pub cross_margin_risks: &'static str,
    pub technical_risks: &'static str,
    pub market_risks: &'static str,
    pub chain_position_risks: &'static str,
}

impl RiskDisclosureSections {
    pub fn new() -> Self {
        Self {
            leverage_amplifies_losses: "With 100x leverage, a 1% adverse price movement results in 100% loss",
            liquidation_risk: "Positions are subject to automatic liquidation when losses exceed collateral",
            funding_rate_costs: "Funding rates are amplified by leverage amount",
            cross_margin_risks: "All positions in cross-margin are at risk if one position experiences large losses",
            technical_risks: "Network congestion may prevent timely order execution",
            market_risks: "Extreme volatility can cause immediate liquidations",
            chain_position_risks: "A single losing leg terminates the entire chain",
        }
    }
}

/// Risk levels based on leverage
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low,      // 1x - 10x
    Medium,   // 11x - 25x
    High,     // 26x - 50x
    Extreme,  // 51x - 100x
    Insane,   // 101x+
}

impl RiskLevel {
    pub fn from_leverage(leverage: u8) -> Self {
        match leverage {
            0..=10 => RiskLevel::Low,
            11..=25 => RiskLevel::Medium,
            26..=50 => RiskLevel::High,
            51..=100 => RiskLevel::Extreme,
            _ => RiskLevel::Insane,
        }
    }
    
    pub fn warning_message(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Standard risk level",
            RiskLevel::Medium => "Elevated risk - Monitor positions closely",
            RiskLevel::High => "High risk - Experienced traders only",
            RiskLevel::Extreme => "Extreme risk - Total loss likely",
            RiskLevel::Insane => "Maximum risk - Not recommended",
        }
    }
    
    pub fn color_code(&self) -> &'static str {
        match self {
            RiskLevel::Low => "green",
            RiskLevel::Medium => "yellow",
            RiskLevel::High => "orange",
            RiskLevel::Extreme => "red",
            RiskLevel::Insane => "darkred",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_risk_disclosure_hash() {
        let hash1 = get_risk_disclosure_hash();
        let hash2 = get_risk_disclosure_hash();
        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }
    
    #[test]
    fn test_risk_levels() {
        assert_eq!(RiskLevel::from_leverage(5), RiskLevel::Low);
        assert_eq!(RiskLevel::from_leverage(20), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_leverage(40), RiskLevel::High);
        assert_eq!(RiskLevel::from_leverage(75), RiskLevel::Extreme);
        assert_eq!(RiskLevel::from_leverage(150), RiskLevel::Insane);
    }
}