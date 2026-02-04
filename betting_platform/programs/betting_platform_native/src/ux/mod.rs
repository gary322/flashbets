//! User Experience (UX) Enhancement Modules
//! 
//! Provides simplified interfaces and features to improve user experience
//! while hiding complexity for non-advanced users.

pub mod one_click_boost;
pub mod complexity_manager;
pub mod health_bars;
pub mod interactive_tours;

pub use one_click_boost::{
    calculate_boost_preview, execute_one_click_boost, format_boost_preview,
    BoostPreview, RiskLevel, DEFAULT_BOOST_MULTIPLIER, MAX_BOOST_MULTIPLIER,
};

pub use complexity_manager::{
    initialize_user_preferences, update_complexity_level, get_feature_availability,
    get_simplified_config, ComplexityLevel, Feature, UserPreferences, SimplifiedConfig,
    DEFAULT_SIMPLE_LEVERAGE,
};

pub use health_bars::{
    process_health_check, batch_health_check, format_health_summary,
    PositionHealth, HealthStatus, HealthMonitoringConfig,
};

pub use interactive_tours::{
    process_start_tour, process_advance_tour_step, get_recommended_tour,
    TourType, TourProgress, TourStep, TourAction, TourDifficulty, TourBadge,
};