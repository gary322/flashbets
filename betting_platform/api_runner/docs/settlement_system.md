# Settlement System Documentation

## Overview

The settlement system handles the resolution of prediction markets and the distribution of payouts to winning positions. It integrates with oracle providers to determine market outcomes and ensures fair and transparent settlement.

## Architecture

### Core Components

1. **SettlementService** (`src/settlement_service.rs`)
   - Main service that orchestrates the settlement process
   - Manages oracle providers and consensus logic
   - Handles on-chain settlement transactions
   - Updates database records

2. **Settlement Endpoints** (`src/settlement_endpoints.rs`)
   - REST API endpoints for settlement operations
   - Admin endpoints for initiating settlement
   - User endpoints for viewing settlement history
   - Oracle query endpoints

3. **Oracle Integration**
   - Pluggable oracle provider system
   - HTTP-based oracle providers
   - Consensus mechanism for multiple oracle results
   - Minimum 66% consensus required for settlement

## Settlement Process

### 1. Oracle Query Phase
```rust
// Query all registered oracles for market resolution
let oracle_results = settlement_service.query_oracles(&market).await?;
```

### 2. Consensus Determination
- Collects results from multiple oracle providers
- Weights results by oracle confidence scores
- Requires 66% consensus threshold
- Admin can override in disputed cases

### 3. Settlement Execution
1. Create settlement batch with all affected positions
2. Build and send on-chain settlement transaction
3. Update position records in database
4. Broadcast settlement events via WebSocket

### 4. Payout Calculation
- Winners receive their shares at 1.0 value
- 1% settlement fee deducted from payouts
- Losers receive nothing
- All calculations done atomically

## API Endpoints

### Admin Endpoints

#### POST /api/settlement/initiate
Initiates settlement for a market (admin only)

Request:
```json
{
  "market_id": 12345,
  "oracle_results": [
    {
      "oracle_name": "Chainlink",
      "outcome": 0,
      "confidence": 0.95,
      "timestamp": "2024-01-15T10:00:00Z",
      "proof_url": "https://oracle.com/proof/12345"
    }
  ],
  "admin_override": null,
  "reason": null
}
```

#### GET /api/settlement/history
View settlement history across all markets

### User Endpoints

#### GET /api/settlement/oracles/:market_id
Query oracle results for a specific market

Response:
```json
{
  "market_id": 12345,
  "oracle_results": [...],
  "consensus_outcome": 0,
  "consensus_confidence": 0.85,
  "can_settle": true,
  "reason": null
}
```

#### GET /api/settlement/status/:market_id
Get settlement status for a market

#### GET /api/settlement/user
Get user's settlement history

## Oracle Provider Interface

Oracle providers must implement the following trait:

```rust
#[async_trait]
pub trait OracleProvider: Send + Sync {
    async fn get_resolution(&self, market: &Market) -> Result<OracleResult>;
    async fn verify_resolution(&self, market_id: u128, outcome: u8) -> Result<bool>;
}
```

### Registering Oracle Providers

```rust
let chainlink_oracle = HttpOracleProvider::new(
    "Chainlink".to_string(),
    "https://api.chainlink.com".to_string(),
    Some(api_key),
);

settlement_service.register_oracle("Chainlink".to_string(), Box::new(chainlink_oracle)).await;
```

## Database Schema

### Settlement Tables

#### settlement_batches
- settlement_id (UUID, primary key)
- market_id (bigint)
- winning_outcome (smallint)
- oracle_consensus (float)
- total_positions (bigint)
- total_payout (bigint)
- settled_at (timestamp)
- transaction_signature (text)

#### position_settlements
- settlement_id (UUID)
- position_id (text)
- wallet (text)
- market_id (bigint)
- outcome (smallint)
- shares (bigint)
- payout (bigint)
- pnl (bigint)
- fees (bigint)
- settlement_price (float)
- settled_at (timestamp)

## Security Considerations

1. **Oracle Security**
   - Multiple oracle sources for redundancy
   - Consensus mechanism prevents single oracle manipulation
   - Admin override requires explicit reason logging

2. **Settlement Authority**
   - Dedicated keypair for settlement transactions
   - Environment variable: `SETTLEMENT_KEYPAIR`
   - Should be separate from other operation keys

3. **Access Control**
   - Only admin role can initiate settlement
   - Users can only view their own settlement history
   - Oracle queries require authentication

## Configuration

### Environment Variables

```bash
# Settlement authority keypair (base58 encoded)
SETTLEMENT_KEYPAIR=<base58_keypair>

# Oracle API keys
CHAINLINK_API_KEY=<api_key>
PYTH_API_KEY=<api_key>
```

### Oracle Configuration

Oracles are registered programmatically during service initialization:

```rust
// In main.rs
let settlement_service = Arc::new(SettlementService::new(...));

// Register oracle providers
let chainlink = HttpOracleProvider::new(...);
settlement_service.register_oracle("Chainlink", Box::new(chainlink)).await;
```

## Error Handling

The settlement system uses typed errors with context:

- `ValidationError`: Invalid settlement request
- `ExternalServiceError`: Oracle communication failure  
- `ProcessingError`: Consensus calculation failure
- `BlockchainError`: On-chain settlement failure
- `DatabaseError`: Database update failure

## WebSocket Events

Settlement events are broadcast via the enhanced WebSocket system:

```json
{
  "type": "market_settled",
  "market_id": 12345,
  "title": "Will BTC reach $50k?",
  "winning_outcome": 0,
  "total_positions": 150,
  "total_payout": 1000000,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Future Enhancements

1. **Decentralized Oracle Network**
   - Integration with Chainlink, Pyth, and other oracle networks
   - On-chain oracle aggregation

2. **Dispute Resolution**
   - Time-locked settlement period
   - Community voting mechanism
   - Appeal process

3. **Automated Settlement**
   - Cron job for automatic settlement checks
   - Smart contract triggers
   - Event-based settlement

4. **Advanced Oracle Features**
   - Weighted oracle voting
   - Reputation-based confidence scores
   - Historical accuracy tracking

## Testing

### Unit Tests
```bash
cargo test settlement_service
```

### Integration Tests
```bash
cargo test --test settlement_integration
```

### Manual Testing
1. Create a test market
2. Place some positions
3. Query oracles: `GET /api/settlement/oracles/:market_id`
4. Initiate settlement: `POST /api/settlement/initiate`
5. Check status: `GET /api/settlement/status/:market_id`
6. View history: `GET /api/settlement/user`