//! Domain-Specific Validators
//! 
//! Provides specialized validators for betting platform domain objects

use std::sync::Arc;
use async_trait::async_trait;
use validator::{ValidationError, ValidationErrors};
use chrono::{Utc, Duration};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::{
    validation_framework::{CustomValidator, ValidationContext, ValidationResult},
    platform::Timestamp,
};

/// Position validator - validates trading positions
pub struct PositionValidator {
    pub max_position_size: u64,
    pub max_leverage: u32,
    pub min_collateral: u64,
}

#[async_trait]
impl CustomValidator for PositionValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate position size
        if let Some(size) = data.get("size").and_then(|v| v.as_u64()) {
            if size > self.max_position_size {
                let mut error = ValidationError::new("position_too_large");
                error.message = Some(format!("Position size {} exceeds maximum allowed {}", size, self.max_position_size).into());
                errors.add("size", error);
            }
            
            if size == 0 {
                let mut error = ValidationError::new("zero_position");
                error.message = Some("Position size cannot be zero".into());
                errors.add("size", error);
            }
        }
        
        // Validate leverage
        if let Some(leverage) = data.get("leverage").and_then(|v| v.as_u64()) {
            if leverage as u32 > self.max_leverage {
                let mut error = ValidationError::new("leverage_too_high");
                error.message = Some(format!("Leverage {} exceeds maximum allowed {}", leverage, self.max_leverage).into());
                errors.add("leverage", error);
            }
            
            if leverage == 0 {
                let mut error = ValidationError::new("zero_leverage");
                error.message = Some("Leverage cannot be zero".into());
                errors.add("leverage", error);
            }
        }
        
        // Validate collateral
        if let Some(collateral) = data.get("collateral").and_then(|v| v.as_u64()) {
            if collateral < self.min_collateral {
                let mut error = ValidationError::new("insufficient_collateral");
                error.message = Some(format!("Collateral {} is below minimum required {}", collateral, self.min_collateral).into());
                errors.add("collateral", error);
            }
        }
        
        // Validate risk parameters
        if let (Some(size), Some(collateral), Some(leverage)) = (
            data.get("size").and_then(|v| v.as_u64()),
            data.get("collateral").and_then(|v| v.as_u64()),
            data.get("leverage").and_then(|v| v.as_u64()),
        ) {
            let required_collateral = size / leverage;
            if collateral < required_collateral {
                let mut error = ValidationError::new("collateral_leverage_mismatch");
                error.message = Some(format!("Collateral {} insufficient for size {} at leverage {}x", collateral, size, leverage).into());
                errors.add("collateral", error);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "position_validator"
    }
    
    fn description(&self) -> &str {
        "Validates trading position parameters"
    }
}

/// Order validator - validates trading orders
pub struct OrderValidator {
    pub min_order_size: u64,
    pub max_order_size: u64,
    pub price_precision: u32,
    pub max_price_deviation: f64,
}

#[async_trait]
impl CustomValidator for OrderValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate order type
        if let Some(order_type) = data.get("order_type").and_then(|v| v.as_str()) {
            match order_type {
                "market" | "limit" | "stop" | "stop_limit" => {},
                _ => {
                    let mut error = ValidationError::new("invalid_order_type");
                    error.message = Some("Invalid order type".into());
                    errors.add("order_type", error);
                }
            }
        }
        
        // Validate order side
        if let Some(side) = data.get("side").and_then(|v| v.as_str()) {
            match side {
                "buy" | "sell" => {},
                _ => {
                    let mut error = ValidationError::new("invalid_order_side");
                    error.message = Some("Order side must be 'buy' or 'sell'".into());
                    errors.add("side", error);
                }
            }
        }
        
        // Validate order size
        if let Some(size) = data.get("size").and_then(|v| v.as_u64()) {
            if size < self.min_order_size {
                let mut error = ValidationError::new("order_too_small");
                error.message = Some(format!("Order size {} is below minimum {}", size, self.min_order_size).into());
                errors.add("size", error);
            }
            
            if size > self.max_order_size {
                let mut error = ValidationError::new("order_too_large");
                error.message = Some(format!("Order size {} exceeds maximum {}", size, self.max_order_size).into());
                errors.add("size", error);
            }
        }
        
        // Validate price for limit orders
        if let Some(order_type) = data.get("order_type").and_then(|v| v.as_str()) {
            if order_type == "limit" || order_type == "stop_limit" {
                if let Some(price) = data.get("price").and_then(|v| v.as_f64()) {
                    if price <= 0.0 {
                        let mut error = ValidationError::new("invalid_price");
                        error.message = Some("Price must be positive".into());
                        errors.add("price", error);
                    }
                    
                    // Check price precision
                    let price_str = format!("{:.8}", price);
                    let decimal_places = price_str.split('.').nth(1).map(|s| s.trim_end_matches('0').len()).unwrap_or(0);
                    if decimal_places > self.price_precision as usize {
                        let mut error = ValidationError::new("excessive_price_precision");
                        error.message = Some(format!("Price precision exceeds {} decimal places", self.price_precision).into());
                        errors.add("price", error);
                    }
                } else {
                    let mut error = ValidationError::new("missing_price");
                    error.message = Some("Limit orders require a price".into());
                    errors.add("price", error);
                }
            }
        }
        
        // Validate time in force
        if let Some(tif) = data.get("time_in_force").and_then(|v| v.as_str()) {
            match tif {
                "GTC" | "IOC" | "FOK" | "GTD" => {},
                _ => {
                    let mut error = ValidationError::new("invalid_time_in_force");
                    error.message = Some("Invalid time in force value".into());
                    errors.add("time_in_force", error);
                }
            }
            
            // If GTD, validate expiry
            if tif == "GTD" {
                if let Some(expiry) = data.get("expiry").and_then(|v| v.as_i64()) {
                    let now = Timestamp::now().as_unix();
                    if expiry <= now {
                        let mut error = ValidationError::new("invalid_expiry");
                        error.message = Some("Order expiry must be in the future".into());
                        errors.add("expiry", error);
                    }
                } else {
                    let mut error = ValidationError::new("missing_expiry");
                    error.message = Some("GTD orders require an expiry time".into());
                    errors.add("expiry", error);
                }
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "order_validator"
    }
    
    fn description(&self) -> &str {
        "Validates trading order parameters"
    }
}

/// Settlement validator - validates settlement operations
pub struct SettlementValidator {
    pub min_settlement_delay: Duration,
    pub max_settlement_delay: Duration,
    pub allowed_oracles: Vec<String>,
}

#[async_trait]
impl CustomValidator for SettlementValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate settlement time
        if let Some(settlement_time) = data.get("settlement_time").and_then(|v| v.as_i64()) {
            let now = Utc::now().timestamp();
            let delay = settlement_time - now;
            
            if delay < self.min_settlement_delay.num_seconds() {
                let mut error = ValidationError::new("settlement_too_soon");
                error.message = Some(format!("Settlement must be at least {} minutes in the future", self.min_settlement_delay.num_minutes()).into());
                errors.add("settlement_time", error);
            }
            
            if delay > self.max_settlement_delay.num_seconds() {
                let mut error = ValidationError::new("settlement_too_late");
                error.message = Some(format!("Settlement cannot be more than {} days in the future", self.max_settlement_delay.num_days()).into());
                errors.add("settlement_time", error);
            }
        }
        
        // Validate oracle
        if let Some(oracle) = data.get("oracle").and_then(|v| v.as_str()) {
            if !self.allowed_oracles.contains(&oracle.to_string()) {
                let mut error = ValidationError::new("invalid_oracle");
                error.message = Some(format!("Oracle '{}' is not in the allowed list", oracle).into());
                errors.add("oracle", error);
            }
        }
        
        // Validate winning outcome
        if let Some(outcome_index) = data.get("winning_outcome").and_then(|v| v.as_u64()) {
            if let Some(total_outcomes) = data.get("total_outcomes").and_then(|v| v.as_u64()) {
                if outcome_index >= total_outcomes {
                    let mut error = ValidationError::new("invalid_outcome_index");
                    error.message = Some(format!("Outcome index {} exceeds total outcomes {}", outcome_index, total_outcomes).into());
                    errors.add("winning_outcome", error);
                }
            }
        }
        
        // Validate settlement proof
        if let Some(proof) = data.get("proof").and_then(|v| v.as_object()) {
            if proof.is_empty() {
                let mut error = ValidationError::new("empty_proof");
                error.message = Some("Settlement proof cannot be empty".into());
                errors.add("proof", error);
            }
            
            // Validate required proof fields
            let required_fields = vec!["source", "timestamp", "signature"];
            for field in required_fields {
                if !proof.contains_key(field) {
                    let mut error = ValidationError::new("missing_proof_field");
                    error.message = Some(format!("Proof missing required field: {}", field).into());
                    errors.add("proof", error);
                }
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "settlement_validator"
    }
    
    fn description(&self) -> &str {
        "Validates market settlement operations"
    }
}

/// Liquidity validator - validates liquidity operations
pub struct LiquidityValidator {
    pub min_liquidity_amount: u64,
    pub max_liquidity_amount: u64,
    pub min_lock_period: Duration,
    pub max_slippage: f64,
}

#[async_trait]
impl CustomValidator for LiquidityValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate liquidity amount
        if let Some(amount) = data.get("amount").and_then(|v| v.as_u64()) {
            if amount < self.min_liquidity_amount {
                let mut error = ValidationError::new("insufficient_liquidity");
                error.message = Some(format!("Liquidity amount {} is below minimum {}", amount, self.min_liquidity_amount).into());
                errors.add("amount", error);
            }
            
            if amount > self.max_liquidity_amount {
                let mut error = ValidationError::new("excessive_liquidity");
                error.message = Some(format!("Liquidity amount {} exceeds maximum {}", amount, self.max_liquidity_amount).into());
                errors.add("amount", error);
            }
        }
        
        // Validate lock period
        if let Some(lock_period) = data.get("lock_period").and_then(|v| v.as_i64()) {
            if lock_period < self.min_lock_period.num_seconds() {
                let mut error = ValidationError::new("lock_period_too_short");
                error.message = Some(format!("Lock period must be at least {} days", self.min_lock_period.num_days()).into());
                errors.add("lock_period", error);
            }
        }
        
        // Validate slippage tolerance
        if let Some(slippage) = data.get("max_slippage").and_then(|v| v.as_f64()) {
            if slippage < 0.0 || slippage > self.max_slippage {
                let mut error = ValidationError::new("invalid_slippage");
                error.message = Some(format!("Slippage tolerance must be between 0 and {:.2}%", self.max_slippage * 100.0).into());
                errors.add("max_slippage", error);
            }
        }
        
        // Validate pool ratios for liquidity provision
        if let Some(ratios) = data.get("pool_ratios").and_then(|v| v.as_array()) {
            let sum: f64 = ratios.iter()
                .filter_map(|v| v.as_f64())
                .sum();
            
            if (sum - 1.0).abs() > 0.001 {
                let mut error = ValidationError::new("invalid_pool_ratios");
                error.message = Some("Pool ratios must sum to 1.0".into());
                errors.add("pool_ratios", error);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "liquidity_validator"
    }
    
    fn description(&self) -> &str {
        "Validates liquidity provision operations"
    }
}

/// Quantum position validator - validates quantum trading positions
pub struct QuantumPositionValidator {
    pub max_entanglement_degree: u32,
    pub min_coherence: f64,
    pub max_superposition_states: u32,
}

#[async_trait]
impl CustomValidator for QuantumPositionValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate entanglement degree
        if let Some(entanglement) = data.get("entanglement_degree").and_then(|v| v.as_u64()) {
            if entanglement as u32 > self.max_entanglement_degree {
                let mut error = ValidationError::new("excessive_entanglement");
                error.message = Some(format!("Entanglement degree {} exceeds maximum {}", entanglement, self.max_entanglement_degree).into());
                errors.add("entanglement_degree", error);
            }
        }
        
        // Validate coherence
        if let Some(coherence) = data.get("coherence").and_then(|v| v.as_f64()) {
            if coherence < self.min_coherence {
                let mut error = ValidationError::new("insufficient_coherence");
                error.message = Some(format!("Coherence {:.2} is below minimum {:.2}", coherence, self.min_coherence).into());
                errors.add("coherence", error);
            }
            
            if coherence < 0.0 || coherence > 1.0 {
                let mut error = ValidationError::new("invalid_coherence");
                error.message = Some("Coherence must be between 0 and 1".into());
                errors.add("coherence", error);
            }
        }
        
        // Validate superposition states
        if let Some(states) = data.get("superposition_states").and_then(|v| v.as_array()) {
            if states.len() > self.max_superposition_states as usize {
                let mut error = ValidationError::new("too_many_states");
                error.message = Some(format!("Number of superposition states {} exceeds maximum {}", states.len(), self.max_superposition_states).into());
                errors.add("superposition_states", error);
            }
            
            // Validate probability amplitudes sum to 1
            let amplitude_sum: f64 = states.iter()
                .filter_map(|s| s.get("amplitude").and_then(|v| v.as_f64()))
                .map(|a| a * a) // Square for probability
                .sum();
            
            if (amplitude_sum - 1.0).abs() > 0.001 {
                let mut error = ValidationError::new("invalid_amplitudes");
                error.message = Some("Probability amplitudes must sum to 1".into());
                errors.add("superposition_states", error);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "quantum_position_validator"
    }
    
    fn description(&self) -> &str {
        "Validates quantum trading position parameters"
    }
}

/// Transaction validator - validates Solana transactions
pub struct TransactionValidator {
    pub max_accounts: usize,
    pub max_data_size: usize,
    pub max_compute_units: u64,
    pub allowed_programs: Vec<Pubkey>,
}

#[async_trait]
impl CustomValidator for TransactionValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate number of accounts
        if let Some(accounts) = data.get("accounts").and_then(|v| v.as_array()) {
            if accounts.len() > self.max_accounts {
                let mut error = ValidationError::new("too_many_accounts");
                error.message = Some(format!("Transaction has {} accounts, maximum is {}", accounts.len(), self.max_accounts).into());
                errors.add("accounts", error);
            }
            
            // Validate each account
            for (i, account) in accounts.iter().enumerate() {
                if let Some(pubkey) = account.get("pubkey").and_then(|v| v.as_str()) {
                    if Pubkey::from_str(pubkey).is_err() {
                        let mut error = ValidationError::new("invalid_account_pubkey");
                        error.message = Some(format!("Invalid pubkey at index {}", i).into());
                        errors.add("accounts", error);
                    }
                }
            }
        }
        
        // Validate instruction data size
        if let Some(instructions) = data.get("instructions").and_then(|v| v.as_array()) {
            let total_data_size: usize = instructions.iter()
                .filter_map(|i| i.get("data").and_then(|d| d.as_str()))
                .map(|d| d.len() / 2) // Hex encoding
                .sum();
            
            if total_data_size > self.max_data_size {
                let mut error = ValidationError::new("excessive_data_size");
                error.message = Some(format!("Total instruction data size {} exceeds maximum {}", total_data_size, self.max_data_size).into());
                errors.add("instructions", error);
            }
            
            // Validate program IDs
            for (i, instruction) in instructions.iter().enumerate() {
                if let Some(program_id) = instruction.get("program_id").and_then(|v| v.as_str()) {
                    match Pubkey::from_str(program_id) {
                        Ok(pubkey) => {
                            if !self.allowed_programs.contains(&pubkey) {
                                let mut error = ValidationError::new("unauthorized_program");
                                error.message = Some(format!("Program {} not in allowed list at instruction {}", program_id, i).into());
                                errors.add("instructions", error);
                            }
                        }
                        Err(_) => {
                            let mut error = ValidationError::new("invalid_program_id");
                            error.message = Some(format!("Invalid program ID at instruction {}", i).into());
                            errors.add("instructions", error);
                        }
                    }
                }
            }
        }
        
        // Validate compute units
        if let Some(compute_units) = data.get("compute_units").and_then(|v| v.as_u64()) {
            if compute_units > self.max_compute_units {
                let mut error = ValidationError::new("excessive_compute_units");
                error.message = Some(format!("Compute units {} exceeds maximum {}", compute_units, self.max_compute_units).into());
                errors.add("compute_units", error);
            }
        }
        
        // Validate signatures
        if let Some(signatures) = data.get("signatures").and_then(|v| v.as_array()) {
            if signatures.is_empty() {
                let mut error = ValidationError::new("no_signatures");
                error.message = Some("Transaction must have at least one signature".into());
                errors.add("signatures", error);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "transaction_validator"
    }
    
    fn description(&self) -> &str {
        "Validates Solana transaction parameters"
    }
}

/// Create default domain validators
pub fn create_default_validators() -> Vec<Arc<dyn CustomValidator>> {
    vec![
        Arc::new(PositionValidator {
            max_position_size: 1_000_000_000_000, // 1M tokens
            max_leverage: 20,
            min_collateral: 100_000_000, // 0.1 SOL
        }),
        Arc::new(OrderValidator {
            min_order_size: 1_000_000, // 0.001 tokens
            max_order_size: 100_000_000_000, // 100k tokens
            price_precision: 6,
            max_price_deviation: 0.5, // 50%
        }),
        Arc::new(SettlementValidator {
            min_settlement_delay: Duration::minutes(5),
            max_settlement_delay: Duration::days(365),
            allowed_oracles: vec![
                "polymarket".to_string(),
                "chainlink".to_string(),
                "pyth".to_string(),
            ],
        }),
        Arc::new(LiquidityValidator {
            min_liquidity_amount: 100_000_000, // 0.1 SOL
            max_liquidity_amount: 100_000_000_000_000, // 100k SOL
            min_lock_period: Duration::days(1),
            max_slippage: 0.1, // 10%
        }),
        Arc::new(QuantumPositionValidator {
            max_entanglement_degree: 5,
            min_coherence: 0.5,
            max_superposition_states: 10,
        }),
        Arc::new(TransactionValidator {
            max_accounts: 35,
            max_data_size: 1232,
            max_compute_units: 1_400_000,
            allowed_programs: vec![
                // Add allowed program IDs here
            ],
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_position_validator() {
        let validator = PositionValidator {
            max_position_size: 1000,
            max_leverage: 10,
            min_collateral: 100,
        };
        
        let valid_data = serde_json::json!({
            "size": 500,
            "leverage": 5,
            "collateral": 100
        });
        
        let context = ValidationContext::default();
        assert!(validator.validate(&valid_data, &context).await.is_ok());
        
        let invalid_data = serde_json::json!({
            "size": 2000,
            "leverage": 20,
            "collateral": 50
        });
        
        let result = validator.validate(&invalid_data, &context).await;
        assert!(result.is_err());
    }
}