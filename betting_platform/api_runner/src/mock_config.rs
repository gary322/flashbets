//! Mock service configuration
//! Provides configuration for mock services in different environments

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Mock service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockConfig {
    pub enabled: bool,
    pub oracle: MockOracleConfig,
    pub solana: MockSolanaConfig,
    pub trading: MockTradingConfig,
    pub external_api: MockExternalApiConfig,
    pub price_feed: MockPriceFeedConfig,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            oracle: MockOracleConfig::default(),
            solana: MockSolanaConfig::default(),
            trading: MockTradingConfig::default(),
            external_api: MockExternalApiConfig::default(),
            price_feed: MockPriceFeedConfig::default(),
        }
    }
}

/// Mock oracle configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockOracleConfig {
    pub providers: Vec<MockOracleProviderConfig>,
    pub default_confidence: f64,
    pub consensus_threshold: f64,
}

impl Default for MockOracleConfig {
    fn default() -> Self {
        Self {
            providers: vec![
                MockOracleProviderConfig {
                    name: "Chainlink".to_string(),
                    confidence: 0.95,
                    response_delay_ms: 100,
                    fail_rate: 0.01,
                },
                MockOracleProviderConfig {
                    name: "Pyth".to_string(),
                    confidence: 0.93,
                    response_delay_ms: 150,
                    fail_rate: 0.02,
                },
                MockOracleProviderConfig {
                    name: "UMA".to_string(),
                    confidence: 0.90,
                    response_delay_ms: 200,
                    fail_rate: 0.03,
                },
            ],
            default_confidence: 0.95,
            consensus_threshold: 0.66,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockOracleProviderConfig {
    pub name: String,
    pub confidence: f64,
    pub response_delay_ms: u64,
    pub fail_rate: f64,
}

/// Mock Solana configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockSolanaConfig {
    pub initial_balance: u64,
    pub transaction_fee: u64,
    pub confirmation_time_ms: u64,
    pub fail_rate: f64,
}

impl Default for MockSolanaConfig {
    fn default() -> Self {
        Self {
            initial_balance: 1_000_000_000, // 1 SOL
            transaction_fee: 5000,           // 0.000005 SOL
            confirmation_time_ms: 400,
            fail_rate: 0.005,
        }
    }
}

/// Mock trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockTradingConfig {
    pub initial_markets: Vec<MockMarketConfig>,
    pub order_processing_time_ms: u64,
    pub slippage_rate: f64,
    pub fail_rate: f64,
}

impl Default for MockTradingConfig {
    fn default() -> Self {
        Self {
            initial_markets: vec![
                MockMarketConfig {
                    id: 1000,
                    title: "BTC to reach $100k by 2024".to_string(),
                    liquidity: 500_000,
                    initial_yes_price: 0.45,
                    volatility: 0.02,
                },
                MockMarketConfig {
                    id: 1001,
                    title: "2024 Presidential Election".to_string(),
                    liquidity: 1_000_000,
                    initial_yes_price: 0.52,
                    volatility: 0.03,
                },
            ],
            order_processing_time_ms: 50,
            slippage_rate: 0.001,
            fail_rate: 0.001,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockMarketConfig {
    pub id: u128,
    pub title: String,
    pub liquidity: u64,
    pub initial_yes_price: f64,
    pub volatility: f64,
}

/// Mock external API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockExternalApiConfig {
    pub endpoints: Vec<MockEndpointConfig>,
    pub default_latency_ms: u64,
    pub fail_rate: f64,
}

impl Default for MockExternalApiConfig {
    fn default() -> Self {
        Self {
            endpoints: vec![
                MockEndpointConfig {
                    pattern: "/markets".to_string(),
                    response_file: None,
                    latency_ms: 20,
                    status_code: 200,
                },
                MockEndpointConfig {
                    pattern: "/prices".to_string(),
                    response_file: None,
                    latency_ms: 10,
                    status_code: 200,
                },
            ],
            default_latency_ms: 50,
            fail_rate: 0.01,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEndpointConfig {
    pub pattern: String,
    pub response_file: Option<String>,
    pub latency_ms: u64,
    pub status_code: u16,
}

/// Mock price feed configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockPriceFeedConfig {
    pub symbols: Vec<MockSymbolConfig>,
    pub update_interval_ms: u64,
    pub volatility: f64,
}

impl Default for MockPriceFeedConfig {
    fn default() -> Self {
        Self {
            symbols: vec![
                MockSymbolConfig {
                    symbol: "BTC".to_string(),
                    initial_price: 45000.0,
                    min_price: 30000.0,
                    max_price: 100000.0,
                    volatility: 0.02,
                },
                MockSymbolConfig {
                    symbol: "ETH".to_string(),
                    initial_price: 2800.0,
                    min_price: 1500.0,
                    max_price: 10000.0,
                    volatility: 0.025,
                },
                MockSymbolConfig {
                    symbol: "SOL".to_string(),
                    initial_price: 95.0,
                    min_price: 20.0,
                    max_price: 500.0,
                    volatility: 0.03,
                },
            ],
            update_interval_ms: 5000,
            volatility: 0.02,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockSymbolConfig {
    pub symbol: String,
    pub initial_price: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub volatility: f64,
}

/// Load mock configuration from environment
pub fn load_mock_config() -> MockConfig {
    if std::env::var("MOCK_SERVICES_ENABLED").unwrap_or_default() == "true" {
        let mut config = MockConfig::default();
        config.enabled = true;
        
        // Override from environment variables if set
        if let Ok(oracle_confidence) = std::env::var("MOCK_ORACLE_CONFIDENCE") {
            if let Ok(conf) = oracle_confidence.parse::<f64>() {
                config.oracle.default_confidence = conf;
            }
        }
        
        if let Ok(solana_fail_rate) = std::env::var("MOCK_SOLANA_FAIL_RATE") {
            if let Ok(rate) = solana_fail_rate.parse::<f64>() {
                config.solana.fail_rate = rate;
            }
        }
        
        config
    } else {
        MockConfig::default()
    }
}

/// Mock configuration profiles
pub enum MockProfile {
    /// Realistic behavior with occasional failures
    Realistic,
    /// Fast responses, no failures
    Fast,
    /// High failure rates for testing error handling
    Chaos,
    /// Custom profile
    Custom(MockConfig),
}

impl MockProfile {
    pub fn to_config(self) -> MockConfig {
        match self {
            MockProfile::Realistic => MockConfig::default(),
            MockProfile::Fast => {
                let mut config = MockConfig::default();
                config.oracle.providers.iter_mut().for_each(|p| {
                    p.response_delay_ms = 10;
                    p.fail_rate = 0.0;
                });
                config.solana.confirmation_time_ms = 50;
                config.solana.fail_rate = 0.0;
                config.trading.order_processing_time_ms = 10;
                config.trading.fail_rate = 0.0;
                config
            }
            MockProfile::Chaos => {
                let mut config = MockConfig::default();
                config.oracle.providers.iter_mut().for_each(|p| {
                    p.fail_rate = 0.3;
                });
                config.solana.fail_rate = 0.2;
                config.trading.fail_rate = 0.25;
                config.external_api.fail_rate = 0.3;
                config
            }
            MockProfile::Custom(config) => config,
        }
    }
}