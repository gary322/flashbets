# Polymarket Integration Documentation

## Overview

This document provides comprehensive documentation for the Polymarket integration implemented in the betting platform. The integration enables real-time trading on Polymarket's prediction markets through their CLOB (Central Limit Order Book) API, with full support for order management, market data, CTF operations, and settlement.

## Architecture

### Components

```
┌─────────────────────────────────────────────────────┐
│                   Frontend (React)                   │
│  - OrderBook.tsx    - TradingPanel.tsx              │
│  - polymarketService.ts                             │
└─────────────────────────────────────────────────────┘
                           │
                    REST API / WebSocket
                           │
┌─────────────────────────────────────────────────────┐
│                 API Server (Rust)                    │
│  ┌──────────────────────────────────────────────┐  │
│  │           Polymarket Handlers                 │  │
│  │  - Order Management                          │  │
│  │  - Market Data                               │  │
│  │  - CTF Operations                            │  │
│  └──────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────┐  │
│  │           Integration Layer                   │  │
│  │  - PolymarketAuth (L1/L2 Authentication)    │  │
│  │  - PolymarketClobClient (Order Operations)  │  │
│  │  - PolymarketWsClient (Real-time Updates)   │  │
│  │  - PolymarketCtfClient (Token Operations)   │  │
│  └──────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────┐  │
│  │           Service Layer                       │  │
│  │  - PolymarketOrderService                    │  │
│  │  - Order Lifecycle Management                │  │
│  │  - Position Tracking                         │  │
│  └──────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────┐  │
│  │           Data Layer                          │  │
│  │  - PolymarketRepository                      │  │
│  │  - PostgreSQL Database                       │  │
│  └──────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
                           │
                    Polygon Network
                           │
┌─────────────────────────────────────────────────────┐
│                     Polymarket                       │
│  - CLOB API                                         │
│  - CTF Exchange Contract                            │
│  - Conditional Tokens Framework                     │
└─────────────────────────────────────────────────────┘
```

## Configuration

### Environment Variables

```bash
# Required for Polymarket integration
POLYMARKET_API_KEY=your_api_key_here
POLYMARKET_API_SECRET=your_api_secret_here
POLYMARKET_API_PASSPHRASE=your_passphrase_here
POLYMARKET_PRIVATE_KEY=0x_your_ethereum_private_key  # For L1 auth
POLYMARKET_RPC_URL=https://polygon-rpc.com
POLYMARKET_WS_URL=wss://api.polymarket.com/ws

# Database
DATABASE_URL=postgresql://user:pass@localhost/betting_platform

# Optional
POLYMARKET_ENV=production  # or staging
ENABLE_POLYMARKET_MONITORING=true
POLYMARKET_MAX_ORDER_SIZE=10000
POLYMARKET_MIN_ORDER_SIZE=1
```

### API Key Setup

1. Visit https://docs.polymarket.com/developers/CLOB/authentication#create-api-key
2. Sign up for a Polymarket account
3. Generate API credentials (free tier available)
4. Store credentials securely in environment variables

## Database Schema

### Core Tables

```sql
-- Markets synchronized from Polymarket
CREATE TABLE polymarket_markets (
    market_id VARCHAR(66) PRIMARY KEY,
    condition_id VARCHAR(66) NOT NULL,
    question_id VARCHAR(66),
    title TEXT NOT NULL,
    description TEXT,
    outcomes JSONB NOT NULL,
    tokens JSONB NOT NULL,
    end_date TIMESTAMP WITH TIME ZONE,
    liquidity DECIMAL(20, 6),
    volume DECIMAL(20, 6),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- User orders
CREATE TABLE polymarket_orders (
    order_id VARCHAR(100) PRIMARY KEY,
    user_id VARCHAR(100) NOT NULL,
    market_id VARCHAR(66) NOT NULL,
    condition_id VARCHAR(66) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    outcome INTEGER NOT NULL,
    side VARCHAR(10) NOT NULL, -- 'buy' or 'sell'
    size DECIMAL(20, 6) NOT NULL,
    price DECIMAL(10, 6) NOT NULL,
    filled_amount DECIMAL(20, 6) DEFAULT 0,
    status VARCHAR(20) NOT NULL,
    polymarket_order_id VARCHAR(100),
    signature TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- User positions
CREATE TABLE polymarket_positions (
    position_id SERIAL PRIMARY KEY,
    user_id VARCHAR(100) NOT NULL,
    condition_id VARCHAR(66) NOT NULL,
    outcome_index INTEGER NOT NULL,
    balance DECIMAL(20, 6) NOT NULL,
    locked_balance DECIMAL(20, 6) DEFAULT 0,
    average_price DECIMAL(10, 6),
    realized_pnl DECIMAL(20, 6) DEFAULT 0,
    unrealized_pnl DECIMAL(20, 6) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(user_id, condition_id, outcome_index)
);
```

## API Endpoints

### Order Management

#### Create Order
```http
POST /api/polymarket/orders
Authorization: Bearer {token}
Content-Type: application/json

{
  "marketId": "0x1234...",
  "conditionId": "0xabcd...",
  "tokenId": "987654321",
  "outcome": 1,
  "side": "buy",
  "size": "100",
  "price": "0.65",
  "orderType": "gtc"
}

Response:
{
  "success": true,
  "data": {
    "orderId": "order_123",
    "status": "pending",
    "orderData": {...}
  }
}
```

#### Submit Signed Order
```http
POST /api/polymarket/orders/submit
Authorization: Bearer {token}

{
  "orderData": {...},
  "signature": "0x..."
}
```

#### Cancel Order
```http
DELETE /api/polymarket/orders/{orderId}
Authorization: Bearer {token}
```

### Market Data

#### Get Order Book
```http
GET /api/polymarket/orderbook/{tokenId}

Response:
{
  "bids": [
    {"price": "0.45", "size": "1000", "numOrders": 3},
    ...
  ],
  "asks": [
    {"price": "0.55", "size": "500", "numOrders": 2},
    ...
  ],
  "spread": "0.10",
  "midPrice": "0.50"
}
```

#### Get Market Data
```http
GET /api/polymarket/markets/{conditionId}

Response:
{
  "conditionId": "0x...",
  "liquidity": "50000",
  "volume24h": "125000",
  "lastPrice": "0.52",
  "openInterest": "75000"
}
```

### CTF Operations

#### Split Position (Mint Outcome Tokens)
```http
POST /api/polymarket/ctf/split
Authorization: Bearer {token}

{
  "conditionId": "0x...",
  "amount": "1000"
}

Response:
{
  "txHash": "0x...",
  "yesTokens": "1000",
  "noTokens": "1000",
  "gasUsed": 150000
}
```

#### Redeem Winning Positions
```http
POST /api/polymarket/ctf/redeem
Authorization: Bearer {token}

{
  "conditionId": "0x...",
  "indexSets": ["1", "2"]
}
```

## Order Lifecycle

```mermaid
graph TD
    A[User Places Order] --> B[Create Order in DB]
    B --> C[Generate Order Payload]
    C --> D[Sign with EIP-712]
    D --> E[Submit to Polymarket]
    E --> F{Accepted?}
    F -->|Yes| G[Update Status: Open]
    F -->|No| H[Update Status: Failed]
    G --> I[Monitor via WebSocket]
    I --> J{Order Event}
    J -->|Fill| K[Update Filled Amount]
    J -->|Cancel| L[Update Status: Cancelled]
    J -->|Complete| M[Update Status: Filled]
    K --> N[Update Position]
    M --> O[Final Settlement]
```

## Authentication

### L1 Authentication (Ethereum Signing)
- Uses EIP-712 typed data signing
- Required for order submission
- Signature validation on Polygon network

### L2 Authentication (API Keys)
- HMAC-SHA256 signatures
- Required headers: `POLY-API-KEY`, `POLY-SIGNATURE`, `POLY-TIMESTAMP`
- Automatic signature generation for all requests

## WebSocket Integration

### Connection
```typescript
const ws = polymarketService.connectWebSocket(
  (message) => {
    switch(message.type) {
      case 'order_update':
        handleOrderUpdate(message.data);
        break;
      case 'market_update':
        handleMarketUpdate(message.data);
        break;
    }
  },
  (error) => console.error('WebSocket error:', error)
);
```

### Subscriptions
- `orders` - User's order updates
- `market:{id}` - Market data updates
- `book:{tokenId}` - Order book updates

## Error Handling

### Common Errors

| Error Code | Description | Resolution |
|------------|-------------|------------|
| `INSUFFICIENT_BALANCE` | Not enough USDC | Deposit funds |
| `INVALID_SIGNATURE` | Order signature invalid | Check private key |
| `MARKET_CLOSED` | Market has ended | Cannot place orders |
| `RATE_LIMITED` | Too many requests | Implement backoff |
| `ORDER_TOO_SMALL` | Below minimum size | Increase order size |

### Retry Logic
```rust
// Automatic retry with exponential backoff
let retry_policy = ExponentialBackoff::default()
    .max_retries(3)
    .initial_interval(Duration::from_millis(500))
    .max_interval(Duration::from_secs(5));
```

## Monitoring

### Metrics Exposed

```prometheus
# Order metrics
polymarket_orders_total{status="success",side="buy",order_type="gtc"}
polymarket_order_latency_seconds{operation="submit"}
polymarket_order_success_rate{timeframe="all_time"}

# API health
polymarket_api_errors_total{endpoint="clob",error_type="timeout"}
polymarket_websocket_status{channel="orders"}

# CTF operations
polymarket_ctf_operations_total{operation="split",status="success"}

# Position values
polymarket_position_values{user="0x...",market="0x..."}
```

### Health Check
```http
GET /api/polymarket/health

Response:
{
  "clobConnected": true,
  "websocketConnected": true,
  "databaseConnected": true,
  "lastSync": "2024-01-15T10:30:00Z",
  "pendingOrders": 5,
  "activePositions": 12
}
```

## Testing

### Unit Tests
```bash
cargo test --test polymarket_integration_test
```

### Integration Tests
```bash
# Requires running API server
cargo test --test polymarket_e2e_test
```

### Load Testing
```bash
./test_polymarket_integration.sh
```

## Security Considerations

1. **Private Key Storage**: Never commit private keys. Use secure key management.
2. **API Key Rotation**: Rotate API keys regularly.
3. **Rate Limiting**: Implement client-side rate limiting.
4. **Input Validation**: Validate all order parameters.
5. **Signature Verification**: Always verify signatures before submission.

## Troubleshooting

### WebSocket Disconnections
- Automatic reconnection with exponential backoff
- Check `POLYMARKET_WS_URL` configuration
- Monitor `polymarket_websocket_status` metric

### Order Failures
- Check account balance on Polygon
- Verify market is still open
- Ensure price is within valid range (0-1)
- Check order size meets minimum requirements

### Database Issues
- Run migrations: `psql -d betting_platform < migrations/002_polymarket_integration.sql`
- Check connection pool settings
- Monitor `polymarket_database_status` metric

## Performance Optimization

1. **Order Caching**: Recent orders cached for 30 seconds
2. **Batch Operations**: Submit multiple orders in parallel
3. **Connection Pooling**: Reuse HTTP connections
4. **WebSocket Multiplexing**: Single connection for all subscriptions

## Future Enhancements

- [ ] Advanced order types (stop-loss, trailing)
- [ ] Portfolio analytics dashboard
- [ ] Automated market making strategies
- [ ] Cross-market arbitrage detection
- [ ] Mobile app integration
- [ ] Historical data analysis tools

## Support

For issues or questions:
1. Check the troubleshooting section
2. Review error logs: `RUST_LOG=debug`
3. Monitor health endpoints
4. Contact Polymarket support for API issues

## License

This integration is provided as-is for use with the betting platform. Ensure compliance with Polymarket's terms of service and local regulations.