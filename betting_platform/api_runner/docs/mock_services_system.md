# Mock Services System Documentation

## Overview

The Mock Services System provides production-grade mock implementations of external services for testing the betting platform. It enables comprehensive integration testing without depending on actual external services.

## Architecture

### Core Components

1. **MockServiceManager** (`mock_service_manager.rs`)
   - Manages lifecycle of all mock services
   - Handles configuration and initialization
   - Injects mock services into AppState
   - Provides service statistics and control

2. **Mock Services** (`mock_services.rs`)
   - MockOracleProvider: Simulates oracle price feeds and resolutions
   - MockSolanaRpcClient: Simulates blockchain interactions
   - MockTradingEngine: Simulates order matching and trading
   - MockWebSocketManager: Simulates WebSocket connections
   - MockExternalApiClient: Simulates external API calls
   - MockPriceFeed: Simulates real-time price data

3. **Configuration** (`mock_config.rs`)
   - Environment-based configuration
   - Pre-defined profiles (Realistic, Fast, Chaos)
   - Customizable failure rates and delays

## Configuration

### Environment Variables

```bash
# Enable mock services
MOCK_SERVICES_ENABLED=true

# Override specific settings
MOCK_ORACLE_CONFIDENCE=0.95
MOCK_SOLANA_FAIL_RATE=0.01
```

### Configuration Profiles

#### Realistic Profile
- Simulates real-world behavior
- Occasional failures (1-3%)
- Realistic delays (100-200ms)
- Normal confidence levels

#### Fast Profile
- Optimized for test speed
- No failures
- Minimal delays (10ms)
- Perfect confidence

#### Chaos Profile
- Tests error handling
- High failure rates (20-30%)
- Variable delays
- Unstable behavior

## Mock Services Details

### MockOracleProvider

Simulates oracle price feed providers:

```rust
let oracle = MockOracleProvider::new("Chainlink".to_string())
    .with_confidence(0.95)
    .with_delay(Duration::from_millis(100))
    .with_fail_rate(0.01);

// Set market outcome
oracle.set_market_outcome(market_id, outcome).await;
```

Features:
- Configurable confidence levels
- Simulated network delays
- Controllable failure rates
- Bulk outcome updates

### MockSolanaRpcClient

Simulates blockchain RPC calls:

```rust
let rpc = MockSolanaRpcClient::new();

// Set account balance
rpc.set_account(pubkey, lamports, data, owner).await;

// Simulate failure
rpc.set_fail_next().await;
```

Features:
- Account balance management
- Transaction simulation
- Blockhash generation
- Failure injection

### MockTradingEngine

Simulates order matching engine:

```rust
let engine = MockTradingEngine::new();

// Add market
engine.add_market(market_id, title, liquidity).await;

// Place order
let order_id = engine.place_order(market_id, user, amount, buy).await?;
```

Features:
- Market management
- Order placement
- Position tracking
- Volume simulation

### MockWebSocketManager

Simulates WebSocket connections:

```rust
let ws = MockWebSocketManager::new();

// Add connection
let conn_id = ws.add_connection(id, user).await;

// Broadcast message
ws.broadcast(message).await;
```

Features:
- Connection management
- Subscription handling
- Message broadcasting
- Connection statistics

### MockExternalApiClient

Simulates external API interactions:

```rust
let api = MockExternalApiClient::new();

// Set response
api.set_response("/markets".to_string(), response).await;

// Set failure pattern
api.set_fail_pattern(Some("error".to_string())).await;
```

Features:
- Endpoint response mocking
- Request logging
- Failure simulation
- Pattern-based failures

### MockPriceFeed

Simulates real-time price feeds:

```rust
let feed = MockPriceFeed::new();

// Set price
feed.set_price("BTC".to_string(), 45000.0).await;

// Start price updates
let handle = feed.start_price_updates(symbols);
```

Features:
- Price management
- Historical data
- Automatic updates
- Volatility simulation

## API Endpoints

### Mock Service Control

```http
# Get mock service statistics
GET /api/mock/stats

# Simulate market activity
POST /api/mock/simulate/market
{
  "market_id": 1000,
  "duration_minutes": 60,
  "trades_per_minute": 10
}

# Set market outcome for testing
POST /api/mock/market/outcome
{
  "market_id": 1000,
  "outcome": 1
}
```

## Usage Examples

### Basic Setup

```rust
// Initialize mock services
let mock_config = MockConfig::default();
let mut mock_manager = MockServiceManager::new(mock_config);
mock_manager.initialize().await?;

// Inject into app state
mock_manager.inject_into_app_state(&mut state).await?;
```

### Testing Market Settlement

```rust
// Set oracle outcomes
if let Some(manager) = &state.mock_service_manager {
    manager.set_market_outcome(market_id, outcome).await?;
}

// Trigger settlement
let result = settlement_service.initiate_settlement(market_id).await?;
```

### Simulating Market Activity

```rust
// Start activity simulation
manager.simulate_market_activity(
    market_id,
    Duration::from_hours(1),
    20, // trades per minute
).await?;
```

### Network Condition Testing

```rust
// Simulate degraded network
manager.simulate_network_conditions(NetworkProfile::Degraded).await?;

// Test with network failures
manager.simulate_network_conditions(NetworkProfile::Offline).await?;
```

## Testing Strategies

### Integration Testing

1. **Happy Path Testing**
   - Use Fast profile
   - Test normal operations
   - Verify correct behavior

2. **Error Handling Testing**
   - Use Chaos profile
   - Test failure scenarios
   - Verify graceful degradation

3. **Performance Testing**
   - Use Realistic profile
   - Test under load
   - Measure response times

### Test Data Scenarios

1. **Market Creation**
   ```rust
   // Create test markets
   let services = MockServiceFactory::create_with_test_data().await;
   ```

2. **Price Volatility**
   ```rust
   // Simulate volatile markets
   feed.start_price_updates(vec!["BTC", "ETH", "SOL"]);
   ```

3. **Oracle Consensus**
   ```rust
   // Test with multiple oracles
   for provider in providers {
       provider.set_market_outcome(market_id, outcome).await;
   }
   ```

## Best Practices

1. **Isolation**
   - Use separate mock instances per test
   - Reset state between tests
   - Avoid shared mutable state

2. **Determinism**
   - Use fixed seeds for randomness
   - Control time in tests
   - Predictable failure injection

3. **Realism**
   - Match production behavior
   - Include edge cases
   - Test concurrency

4. **Performance**
   - Minimize delays in CI
   - Use Fast profile for unit tests
   - Realistic profile for integration tests

## Troubleshooting

### Common Issues

1. **Mock services not initialized**
   - Check MOCK_SERVICES_ENABLED=true
   - Verify initialization in logs
   - Check for initialization errors

2. **Unexpected failures**
   - Check failure rate configuration
   - Verify network profile settings
   - Review error logs

3. **Performance issues**
   - Use appropriate profile
   - Reduce simulation complexity
   - Check for resource leaks

### Debug Logging

Enable debug logs for mock services:

```bash
RUST_LOG=betting_platform_api::mock=debug cargo run
```

## Future Enhancements

1. **Record and Replay**
   - Capture real API responses
   - Replay in tests
   - Automated test generation

2. **Chaos Engineering**
   - More failure modes
   - Latency injection
   - Partial failures

3. **Performance Profiling**
   - Latency distribution
   - Throughput metrics
   - Resource usage

4. **Test Orchestration**
   - Scenario definitions
   - Automated test runs
   - Result aggregation