//! Polymarket fallback manager
//!
//! Handles fallback scenarios for Polymarket integration

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Fallback manager for handling Polymarket API failures
pub struct FallbackManager {
    pub primary_endpoint: String,
    pub fallback_endpoints: Vec<String>,
    pub current_endpoint_index: usize,
    pub max_retries: u8,
}

impl FallbackManager {
    pub fn new(primary: String, fallbacks: Vec<String>) -> Self {
        Self {
            primary_endpoint: primary,
            fallback_endpoints: fallbacks,
            current_endpoint_index: 0,
            max_retries: 3,
        }
    }
    
    pub fn get_current_endpoint(&self) -> &str {
        if self.current_endpoint_index == 0 {
            &self.primary_endpoint
        } else {
            &self.fallback_endpoints[self.current_endpoint_index - 1]
        }
    }
    
    pub fn switch_to_fallback(&mut self) -> bool {
        if self.current_endpoint_index < self.fallback_endpoints.len() {
            self.current_endpoint_index += 1;
            msg!("Switching to fallback endpoint {}", self.current_endpoint_index);
            true
        } else {
            msg!("No more fallback endpoints available");
            false
        }
    }
    
    pub fn reset(&mut self) {
        self.current_endpoint_index = 0;
    }
}