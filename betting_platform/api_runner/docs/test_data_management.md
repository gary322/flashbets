# Test Data Management System Documentation

## Overview

The test data management system provides a comprehensive solution for creating, managing, and cleaning up test data in the betting platform. It ensures consistent test environments and automates test data lifecycle management.

## Architecture

### Core Components

1. **TestDataManager** (`src/test_data_manager.rs`)
   - Central service for test data operations
   - Manages test data lifecycle
   - Provides automatic cleanup
   - Supports data relationships and references

2. **Test Data Endpoints** (`src/test_data_endpoints.rs`)
   - REST API for test data operations
   - Admin-only access control
   - Comprehensive data creation and management

3. **Test Data Categories**
   - Users
   - Markets
   - Positions
   - Orders
   - Transactions
   - Wallets
   - Oracle
   - Quantum
   - Settlement

## Features

### 1. Test Data Creation

#### Create Test Users
```bash
curl -X POST http://localhost:3000/api/test-data/create \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "users": 10,
    "markets": 0,
    "positions_per_user": 0
  }'
```

Response:
```json
{
  "success": true,
  "message": "Test data created successfully",
  "data": {
    "users": 10,
    "data": {
      "users": [
        {
          "id": "uuid",
          "email": "test_user_0@test.com",
          "wallet": "TestWa11et0000000000000000000000000000000000",
          "role": "admin",
          "jwt_token": "eyJ...",
          "balance": 1000000
        }
      ]
    }
  }
}
```

#### Create Complete Test Scenario
```bash
curl -X POST http://localhost:3000/api/test-data/create \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "scenario_name": "e2e_testing",
    "users": 10,
    "markets": 20,
    "positions_per_user": 5,
    "settled_markets": 3
  }'
```

### 2. Test Data Lifecycle

#### Lifecycle Stages
- **Created**: Initial creation
- **Active**: Currently in use
- **Used**: Has been accessed
- **Cleanup**: Marked for deletion
- **Deleted**: Removed from system

#### Automatic Cleanup
- Configurable cleanup intervals
- Expiry-based deletion
- Force cleanup option

### 3. Test Data Querying

#### List Test Data by Category
```bash
curl -X GET "http://localhost:3000/api/test-data/list?category=users&limit=10" \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

#### Search by Tags
```bash
curl -X GET "http://localhost:3000/api/test-data/list?tags=admin,seed" \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

#### Get Specific Test Data
```bash
curl -X GET "http://localhost:3000/api/test-data/{id}" \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### 4. Test Token Generation

Create test JWT tokens for authentication testing:
```bash
curl -X POST http://localhost:3000/api/test-data/tokens \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"count": 5}'
```

Response includes ready-to-use JWT tokens with different roles and permissions.

### 5. Test Data Cleanup

#### Manual Cleanup
```bash
curl -X POST http://localhost:3000/api/test-data/cleanup \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"force": false}'
```

#### Force Cleanup All
```bash
curl -X POST http://localhost:3000/api/test-data/cleanup \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"force": true}'
```

### 6. Test Database Reset

Complete reset with fresh test data:
```bash
curl -X POST http://localhost:3000/api/test-data/reset \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### 7. Test Data Reports

Get comprehensive test data statistics:
```bash
curl -X GET http://localhost:3000/api/test-data/report \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

## Configuration

### Environment Variables
```bash
# Enable test data manager (development only)
ENVIRONMENT=development

# Test data configuration
TEST_DATA_AUTO_CLEANUP=true
TEST_DATA_CLEANUP_INTERVAL_MINUTES=30
TEST_DATA_DEFAULT_EXPIRY_MINUTES=120
TEST_DATA_DATABASE_PREFIX=test_
```

### TestDataConfig
```rust
pub struct TestDataConfig {
    pub auto_cleanup: bool,              // Enable automatic cleanup
    pub cleanup_interval_minutes: u64,    // Cleanup check interval
    pub default_expiry_minutes: u64,      // Default data expiry time
    pub database_prefix: String,          // Prefix for test data in DB
    pub seed_data_path: Option<PathBuf>,  // Optional seed data file
}
```

## Test Data Builder API

The test data builder provides a fluent API for creating complex test scenarios:

```rust
let dataset = TestDataBuilder::new(manager)
    .with_users(10).await?
    .with_markets(20).await?
    .with_positions(5).await?
    .with_settled_markets(3).await?
    .build();
```

## Database Schema

Test data is stored with proper relationships:

### Test Users
- Unique IDs with test prefix
- Pre-generated JWT tokens
- Configurable balances
- Role assignments

### Test Markets
- Realistic market data
- Various categories
- Configurable liquidity
- Settlement support

### Test Positions
- Linked to users and markets
- Realistic position sizes
- Leverage support
- PnL tracking

## Security Considerations

1. **Admin Only Access**
   - All test data endpoints require admin role
   - JWT validation on all requests

2. **Environment Restriction**
   - Test data manager only initializes in development
   - Production environments disable test data features

3. **Data Isolation**
   - Test data uses prefixes
   - Easy identification and cleanup
   - No mixing with production data

## Best Practices

1. **Use Scenarios for Complex Tests**
   - Create named scenarios for repeatability
   - Document scenario purposes

2. **Regular Cleanup**
   - Enable auto-cleanup in CI/CD
   - Force cleanup after test runs

3. **Tag Your Data**
   - Use descriptive tags
   - Tag by test suite or feature

4. **Monitor Test Data Volume**
   - Check reports regularly
   - Adjust expiry times as needed

## Integration with Tests

### Unit Tests
```rust
#[tokio::test]
async fn test_market_creation() {
    let manager = create_test_manager().await;
    let markets = manager.create_test_markets(5).await.unwrap();
    assert_eq!(markets.len(), 5);
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_full_trading_flow() {
    let manager = create_test_manager().await;
    let dataset = TestDataBuilder::new(manager)
        .with_users(2).await?
        .with_markets(1).await?
        .build();
    
    // Use dataset.users[0].jwt_token for authenticated requests
    // Trade on dataset.markets[0].id
}
```

### E2E Tests
```javascript
describe('Trading Flow E2E', () => {
  let testData;
  
  beforeAll(async () => {
    const response = await api.post('/api/test-data/create', {
      scenario_name: 'trading_e2e',
      users: 5,
      markets: 10,
      positions_per_user: 3
    });
    testData = response.data.data;
  });
  
  afterAll(async () => {
    await api.post('/api/test-data/cleanup', { force: true });
  });
  
  it('should complete trade successfully', async () => {
    const token = testData.users[0].jwt_token;
    // Run tests with real test data
  });
});
```

## Troubleshooting

### Common Issues

1. **Test Data Not Cleaning Up**
   - Check auto_cleanup setting
   - Verify cleanup task is running
   - Use force cleanup if needed

2. **Database Conflicts**
   - Ensure unique test prefixes
   - Check for existing test data
   - Reset database if needed

3. **Memory Usage**
   - Monitor test data volume
   - Adjust expiry times
   - Increase cleanup frequency

### Debug Commands

```bash
# Check test data manager status
curl -X GET http://localhost:3000/api/test-data/report

# View cleanup logs
grep "test_data_manager" app.log | grep cleanup

# Force immediate cleanup
curl -X POST http://localhost:3000/api/test-data/cleanup -d '{"force": true}'
```

## Future Enhancements

1. **Data Snapshots**
   - Save/restore test scenarios
   - Version control test data

2. **Performance Testing**
   - Generate large datasets
   - Stress test with realistic data

3. **Data Relationships**
   - Complex relationship graphs
   - Dependency management

4. **Export/Import**
   - Export test scenarios
   - Share between environments