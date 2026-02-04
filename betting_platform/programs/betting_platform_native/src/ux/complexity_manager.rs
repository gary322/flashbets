//! Complexity Manager
//! 
//! Manages UI complexity levels and progressive disclosure of advanced features.
//! Allows users to start with simplified interface and gradually reveal more features.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    constants::LEVERAGE_PRECISION,
    error::BettingPlatformError,
    pda::UserPreferencesPDA,
    validation::validate_account_owner,
};

/// Default leverage for simple mode (10x as per spec)
pub const DEFAULT_SIMPLE_LEVERAGE: u64 = 10;

/// Maximum leverage available in each mode
pub const MAX_LEVERAGE_SIMPLE: u64 = 50;
pub const MAX_LEVERAGE_INTERMEDIATE: u64 = 200;
pub const MAX_LEVERAGE_ADVANCED: u64 = 500;

/// UI Complexity levels
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ComplexityLevel {
    /// Simple mode - Hide most complexity, basic trading only
    Simple,
    /// Intermediate mode - Show some advanced features
    Intermediate,
    /// Advanced mode - Show all features including chains, quantum, etc
    Advanced,
}

impl Default for ComplexityLevel {
    fn default() -> Self {
        ComplexityLevel::Simple
    }
}

impl ComplexityLevel {
    /// Get maximum allowed leverage for this complexity level
    pub fn max_leverage(&self) -> u64 {
        match self {
            ComplexityLevel::Simple => MAX_LEVERAGE_SIMPLE,
            ComplexityLevel::Intermediate => MAX_LEVERAGE_INTERMEDIATE,
            ComplexityLevel::Advanced => MAX_LEVERAGE_ADVANCED,
        }
    }

    /// Check if a feature is available at this complexity level
    pub fn is_feature_available(&self, feature: Feature) -> bool {
        match (self, feature) {
            // Simple mode features
            (ComplexityLevel::Simple, Feature::BasicTrading) => true,
            (ComplexityLevel::Simple, Feature::OneClickBoost) => true,
            (ComplexityLevel::Simple, Feature::StopLoss) => true,
            (ComplexityLevel::Simple, _) => false,

            // Intermediate mode features
            (ComplexityLevel::Intermediate, Feature::BasicTrading) => true,
            (ComplexityLevel::Intermediate, Feature::OneClickBoost) => true,
            (ComplexityLevel::Intermediate, Feature::StopLoss) => true,
            (ComplexityLevel::Intermediate, Feature::AdvancedOrders) => true,
            (ComplexityLevel::Intermediate, Feature::ChainTrading) => true,
            (ComplexityLevel::Intermediate, Feature::MMTStaking) => true,
            (ComplexityLevel::Intermediate, _) => false,

            // Advanced mode - all features available
            (ComplexityLevel::Advanced, _) => true,
        }
    }

    /// Get descriptive label for UI
    pub fn label(&self) -> &'static str {
        match self {
            ComplexityLevel::Simple => "Simple Trading",
            ComplexityLevel::Intermediate => "Intermediate",
            ComplexityLevel::Advanced => "Advanced/Pro",
        }
    }

    /// Get description for UI
    pub fn description(&self) -> &'static str {
        match self {
            ComplexityLevel::Simple => "Basic trading with one-click boost. Perfect for beginners.",
            ComplexityLevel::Intermediate => "Advanced orders and chain trading. For experienced traders.",
            ComplexityLevel::Advanced => "Full platform access including quantum features. For professionals.",
        }
    }
}

/// Platform features that can be hidden/shown based on complexity
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Feature {
    BasicTrading,
    OneClickBoost,
    StopLoss,
    AdvancedOrders,
    ChainTrading,
    QuantumPositions,
    DarkPool,
    CrossMargin,
    MMTStaking,
    Analytics,
}

/// User preferences for UI complexity
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UserPreferences {
    /// User's public key
    pub user: Pubkey,
    /// Selected complexity level
    pub complexity_level: ComplexityLevel,
    /// Default leverage multiplier (scaled by LEVERAGE_PRECISION)
    pub default_leverage: u64,
    /// Whether to show risk warnings
    pub show_risk_warnings: bool,
    /// Whether to show educational tooltips
    pub show_tooltips: bool,
    /// Preferred language code (e.g., "en", "es", "zh")
    pub language_code: [u8; 2],
    /// Theme preference (0 = light, 1 = dark, 2 = auto)
    pub theme: u8,
    /// Whether user has completed onboarding
    pub completed_onboarding: bool,
    /// Features explicitly enabled by user (overrides complexity level)
    pub enabled_features: u64, // Bit flags for features
    /// Reserved space for future fields
    pub reserved: [u8; 64],
}

impl UserPreferences {
    pub const LEN: usize = 32 + 1 + 8 + 1 + 1 + 2 + 1 + 1 + 8 + 64;

    /// Create default preferences for new user
    pub fn new(user: Pubkey) -> Self {
        Self {
            user,
            complexity_level: ComplexityLevel::Simple,
            default_leverage: DEFAULT_SIMPLE_LEVERAGE * LEVERAGE_PRECISION,
            show_risk_warnings: true,
            show_tooltips: true,
            language_code: [b'e', b'n'],
            theme: 2, // Auto
            completed_onboarding: false,
            enabled_features: 0,
            reserved: [0; 64],
        }
    }

    /// Check if a specific feature is enabled for this user
    pub fn is_feature_enabled(&self, feature: Feature) -> bool {
        // First check if explicitly enabled
        let feature_bit = feature as u64;
        if self.enabled_features & (1 << feature_bit) != 0 {
            return true;
        }

        // Otherwise check complexity level
        self.complexity_level.is_feature_available(feature)
    }

    /// Enable a specific feature regardless of complexity level
    pub fn enable_feature(&mut self, feature: Feature) {
        let feature_bit = feature as u64;
        self.enabled_features |= 1 << feature_bit;
    }

    /// Disable a specific feature
    pub fn disable_feature(&mut self, feature: Feature) {
        let feature_bit = feature as u64;
        self.enabled_features &= !(1 << feature_bit);
    }
}

/// Initialize user preferences
pub fn initialize_user_preferences(
    user_preferences_account: &AccountInfo,
    user_account: &AccountInfo,
    program_id: &Pubkey,
) -> ProgramResult {
    // Validate user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate PDA
    let (expected_pda, _bump) = UserPreferencesPDA::derive(program_id, user_account.key);
    if user_preferences_account.key != &expected_pda {
        return Err(BettingPlatformError::InvalidPDA.into());
    }

    // Initialize preferences
    let preferences = UserPreferences::new(*user_account.key);

    // Serialize to account
    preferences.serialize(&mut &mut user_preferences_account.data.borrow_mut()[..])?;

    msg!("Initialized user preferences with Simple complexity level");

    Ok(())
}

/// Update user complexity level
pub fn update_complexity_level(
    user_preferences_account: &AccountInfo,
    user_account: &AccountInfo,
    new_level: ComplexityLevel,
) -> ProgramResult {
    // Validate user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Deserialize preferences
    let mut preferences = UserPreferences::try_from_slice(&user_preferences_account.data.borrow())?;

    // Validate user owns preferences
    if preferences.user != *user_account.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }

    // Update complexity level
    let old_level = preferences.complexity_level;
    preferences.complexity_level = new_level;

    // Adjust default leverage if needed
    if preferences.default_leverage > new_level.max_leverage() * LEVERAGE_PRECISION {
        preferences.default_leverage = new_level.max_leverage() * LEVERAGE_PRECISION;
        msg!(
            "Adjusted default leverage to {} to match {} mode",
            new_level.max_leverage(),
            new_level.label()
        );
    }

    // Serialize updated preferences
    preferences.serialize(&mut &mut user_preferences_account.data.borrow_mut()[..])?;

    msg!(
        "Updated complexity level from {} to {}",
        old_level.label(),
        new_level.label()
    );

    Ok(())
}

/// Get feature availability for UI rendering
pub fn get_feature_availability(
    preferences: &UserPreferences,
) -> Vec<(Feature, bool, &'static str)> {
    let features = vec![
        (Feature::BasicTrading, "Basic Trading"),
        (Feature::OneClickBoost, "One-Click Boost"),
        (Feature::StopLoss, "Stop Loss Orders"),
        (Feature::AdvancedOrders, "Advanced Orders (Iceberg/TWAP)"),
        (Feature::ChainTrading, "Chain Trading"),
        (Feature::QuantumPositions, "Quantum Positions"),
        (Feature::DarkPool, "Dark Pool"),
        (Feature::CrossMargin, "Cross Margin"),
        (Feature::MMTStaking, "MMT Staking"),
        (Feature::Analytics, "Advanced Analytics"),
    ];

    features
        .into_iter()
        .map(|(feature, name)| {
            (feature, preferences.is_feature_enabled(feature), name)
        })
        .collect()
}

/// Get simplified interface configuration
pub fn get_simplified_config(complexity_level: ComplexityLevel) -> SimplifiedConfig {
    SimplifiedConfig {
        show_leverage_slider: true,
        max_leverage: complexity_level.max_leverage(),
        default_leverage: match complexity_level {
            ComplexityLevel::Simple => DEFAULT_SIMPLE_LEVERAGE,
            ComplexityLevel::Intermediate => 25,
            ComplexityLevel::Advanced => 50,
        },
        show_chain_options: complexity_level != ComplexityLevel::Simple,
        show_quantum_tab: complexity_level == ComplexityLevel::Advanced,
        show_advanced_orders: complexity_level != ComplexityLevel::Simple,
        show_analytics: complexity_level != ComplexityLevel::Simple,
        simplified_terminology: complexity_level == ComplexityLevel::Simple,
    }
}

/// Configuration for simplified interface
#[derive(Debug, Clone)]
pub struct SimplifiedConfig {
    pub show_leverage_slider: bool,
    pub max_leverage: u64,
    pub default_leverage: u64,
    pub show_chain_options: bool,
    pub show_quantum_tab: bool,
    pub show_advanced_orders: bool,
    pub show_analytics: bool,
    pub simplified_terminology: bool,
}

impl SimplifiedConfig {
    /// Get terminology based on complexity level
    pub fn get_term<'a>(&self, technical_term: &'a str) -> &'a str {
        if !self.simplified_terminology {
            return technical_term;
        }

        // Simplified terminology mappings
        match technical_term {
            "Verse" => "Market Group",
            "Chain" => "Auto-Boost",
            "Quantum Position" => "Test Position",
            "Liquidation" => "Auto-Close",
            "Funding Rate" => "Holding Cost",
            "Mark Price" => "Current Price",
            "Open Interest" => "Total Bets",
            "Slippage" => "Price Impact",
            _ => technical_term,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_levels() {
        assert_eq!(ComplexityLevel::Simple.max_leverage(), 50);
        assert_eq!(ComplexityLevel::Intermediate.max_leverage(), 200);
        assert_eq!(ComplexityLevel::Advanced.max_leverage(), 500);
    }

    #[test]
    fn test_feature_availability() {
        // Simple mode
        assert!(ComplexityLevel::Simple.is_feature_available(Feature::BasicTrading));
        assert!(ComplexityLevel::Simple.is_feature_available(Feature::OneClickBoost));
        assert!(!ComplexityLevel::Simple.is_feature_available(Feature::QuantumPositions));

        // Advanced mode has everything
        assert!(ComplexityLevel::Advanced.is_feature_available(Feature::BasicTrading));
        assert!(ComplexityLevel::Advanced.is_feature_available(Feature::QuantumPositions));
        assert!(ComplexityLevel::Advanced.is_feature_available(Feature::DarkPool));
    }

    #[test]
    fn test_user_preferences() {
        let user = Pubkey::new_unique();
        let mut prefs = UserPreferences::new(user);

        // Default is simple mode
        assert_eq!(prefs.complexity_level, ComplexityLevel::Simple);
        assert_eq!(prefs.default_leverage, DEFAULT_SIMPLE_LEVERAGE * LEVERAGE_PRECISION);

        // Test feature enabling
        assert!(!prefs.is_feature_enabled(Feature::QuantumPositions));
        prefs.enable_feature(Feature::QuantumPositions);
        assert!(prefs.is_feature_enabled(Feature::QuantumPositions));
    }

    #[test]
    fn test_simplified_terminology() {
        let config = get_simplified_config(ComplexityLevel::Simple);
        assert_eq!(config.get_term("Verse"), "Market Group");
        assert_eq!(config.get_term("Chain"), "Auto-Boost");
        assert_eq!(config.get_term("Quantum Position"), "Test Position");

        let advanced_config = get_simplified_config(ComplexityLevel::Advanced);
        assert_eq!(advanced_config.get_term("Verse"), "Verse");
    }
}