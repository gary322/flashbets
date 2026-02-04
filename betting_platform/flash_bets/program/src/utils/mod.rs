use solana_program::{
    clock::Clock,
    sysvar::Sysvar,
    program_error::ProgramError,
};
use sha2::{Sha256, Digest};

/// Find parent verse based on title and sport type
pub fn find_parent_verse(title: &str, sport_type: u8) -> Result<u128, ProgramError> {
    // Extract parent info from title patterns
    let parent_id = if title.contains("Q1") || title.contains("Quarter 1") {
        // Link to quarter verse
        generate_verse_id("Quarter 1", sport_type)
    } else if title.contains("Half") {
        // Link to half verse
        generate_verse_id("Half Time", sport_type)
    } else if title.contains("Game") || title.contains("Match") {
        // Link to game verse
        generate_verse_id("Full Game", sport_type)
    } else {
        // No parent - root flash verse
        0u128
    };
    
    Ok(parent_id)
}

/// Generate deterministic verse ID from title and sport
pub fn generate_verse_id(title: &str, sport_type: u8) -> u128 {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(&[sport_type]);
    
    let hash = hasher.finalize();
    u128::from_le_bytes(hash[..16].try_into().unwrap())
}

/// Generate unique position ID
pub fn generate_position_id() -> u128 {
    let clock = Clock::get().unwrap();
    let mut hasher = Sha256::new();
    hasher.update(clock.slot.to_le_bytes());
    hasher.update(clock.unix_timestamp.to_le_bytes());
    
    let hash = hasher.finalize();
    u128::from_le_bytes(hash[..16].try_into().unwrap())
}

/// Calculate micro-tau value for flash markets (adjusted for duration)
pub fn calculate_tau(time_left: u64) -> f64 {
    // Adaptive tau based on duration
    // Shorter times get higher tau for more concentrated liquidity
    if time_left <= 60 {
        // Ultra-flash: very concentrated
        0.0001 * (time_left as f64 / 60.0)
    } else if time_left <= 600 {
        // Quick-flash: concentrated
        0.00008 * (time_left as f64 / 600.0)
    } else if time_left <= 3600 {
        // Hour-flash: moderate concentration
        0.00006 * (time_left as f64 / 3600.0)
    } else {
        // Match-long: lower concentration
        0.00004 * (time_left as f64 / 7200.0)
    }
}

/// Sport-specific tau values
pub fn get_sport_tau(sport_type: u8, time_left: u64) -> f64 {
    match sport_type {
        1 => 0.0001 * (60.0 / 45.0),  // Soccer: 45s average
        2 => 0.0001 * (60.0 / 24.0),  // Basketball: 24s shot clock
        3 => 0.0001 * (60.0 / 40.0),  // Football: 40s play clock
        4 => 0.0001 * (60.0 / 20.0),  // Baseball: 20s pitch clock
        5 => 0.0001 * (60.0 / 30.0),  // Tennis: 30s between points
        _ => calculate_tau(time_left),  // Default formula
    }
}

/// Map sport name to type ID
pub fn map_sport_type(sport: &str) -> u8 {
    match sport.to_lowercase().as_str() {
        "soccer" | "football" => 1,
        "basketball" | "nba" => 2,
        "american_football" | "nfl" => 3,
        "baseball" | "mlb" => 4,
        "tennis" => 5,
        _ => 0,
    }
}

/// Validate flash market eligibility (updated for full matches)
pub fn is_flash_eligible(time_left: u64) -> bool {
    time_left <= 14400 // Up to 4 hours (full cricket/baseball match)
}

/// Calculate effective leverage with chaining
pub fn calculate_effective_leverage(
    base_leverage: u8,
    chain_steps: usize,
    tau: f64,
) -> f64 {
    let mut multiplier = base_leverage as f64;
    
    // Apply chaining multipliers
    let chain_mults = [1.5, 1.2, 1.1, 1.05, 1.02];
    for i in 0..chain_steps.min(5) {
        multiplier *= chain_mults[i];
    }
    
    // Apply micro-tau efficiency bonus
    let tau_bonus = 1.0 + tau * 1500.0;
    multiplier *= tau_bonus;
    
    // Cap based on duration
    let cap = match base_leverage {
        1..=100 => 500.0,   // Ultra-flash cap
        101..=150 => 250.0, // Quick-flash cap
        151..=200 => 150.0, // Hour-flash cap
        _ => 100.0,         // Match-long cap
    };
    multiplier.min(cap)
}

/// Format universal market ID
pub fn generate_universal_id(
    provider: &str,
    sport: &str,
    event_id: &str,
    market_id: &str,
) -> String {
    let timestamp = Clock::get()
        .map(|c| c.unix_timestamp)
        .unwrap_or(0);
    
    format!(
        "{}:{}:{}:{}:{}",
        provider.to_uppercase(),
        sport.to_uppercase(),
        event_id,
        market_id,
        timestamp
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tau_calculation() {
        assert_eq!(calculate_tau(60), 0.0001);
        assert_eq!(calculate_tau(30), 0.00005);
        assert_eq!(calculate_tau(120), 0.0002);
    }
    
    #[test]
    fn test_sport_tau() {
        let soccer_tau = get_sport_tau(1, 45);
        assert!(soccer_tau > 0.0001); // Should be higher for shorter time
        
        let basketball_tau = get_sport_tau(2, 24);
        assert!(basketball_tau > soccer_tau); // Even higher for shorter shot clock
    }
    
    #[test]
    fn test_leverage_calculation() {
        let leverage = calculate_effective_leverage(100, 3, 0.0001);
        assert!(leverage > 190.0 && leverage < 250.0); // With 3 steps
        
        let max_leverage = calculate_effective_leverage(200, 5, 0.0001);
        assert_eq!(max_leverage, 500.0); // Capped at 500x
    }
}