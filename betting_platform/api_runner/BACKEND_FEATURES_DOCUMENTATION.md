# Backend Features and UI Integration Documentation

## Overview
This document details all backend features that require user input and their current implementation status in the betting platform.

## 1. Maximum Base Leverage Limits

The platform implements tiered leverage limits based on the number of outcomes in a market:

| Outcomes | Max Base Leverage |
|----------|-------------------|
| 1        | 100x             |
| 2        | 70x              |
| 3-4      | 25x              |
| 5-7      | 15x              |
| 8-15     | 12x              |
| 16-63    | 10x              |
| 64+      | 5x               |

### Implementation
- **Backend**: Leverage tiers defined in `/programs/betting_platform/src/validation.rs` and processor files
- **UI**: Dynamic leverage slider that adjusts max value based on selected market outcomes
- **Location**: `platform_main.js:1060-1097` - `getMaxLeverageForMarket()` function

## 2. Verse System and Total Leverage

### Specifications
- **Maximum Verse Depth**: 32 levels
- **Verse Multipliers**: Each level can add ~1.5x multiplier
- **Maximum Total Leverage**: Base leverage × Verse multipliers (can exceed 500x)

### Current Status
- **Backend**: Verse structure defined but multiplier calculation not fully implemented
- **UI**: Verse selection UI present, multiplier display ready

## 3. Order Types and Features

### Implemented in UI
1. **Market Orders**
   - Investment amount input
   - Real-time position calculation
   - Leverage selection

2. **Limit Orders** 
   - Limit price input (0-100%)
   - Amount input
   - Optional stop loss price
   - UI fully implemented in `index.html:1529-1576`

### NOT Implemented in Backend
These features show "not yet implemented" in handlers:
- Stop limit orders
- Market creation
- Position closing
- Quantum positions creation
- MMT staking
- Liquidity pools (returns mock data only)

## 4. Portfolio and Position Management

### Implemented Features
1. **Portfolio Overview**
   - Total value display
   - Active positions count
   - Total P&L tracking
   - Win rate calculation
   - UI: `index.html` - Portfolio Overview section

2. **Position Display**
   - Market name
   - Outcome
   - Leverage
   - P&L tracking
   - Entry/current price
   - Close/view position buttons

### Backend Endpoints
- `GET /portfolio/:wallet` - Returns portfolio summary
- `GET /positions/:wallet` - Returns user positions
- `GET /balance/:wallet` - Returns wallet balances

## 5. User Input Requirements Summary

### Trading Inputs
1. **Market Selection**
   - Search functionality
   - Market selection from results
   - Verse selection (optional)

2. **Order Configuration**
   - Order type (Market/Limit)
   - Investment amount
   - Base leverage (1-100x, market dependent)
   - Outcome selection
   - Quantum mode toggle

3. **Limit Order Specific**
   - Limit price (0-100%)
   - Stop loss price (optional)

4. **Risk Management**
   - Stop loss slider (5-50%)
   - Take profit slider (10-200%)

### Account Management
1. **Wallet Connection**
   - Phantom wallet integration
   - Balance display
   - Auto-reconnect on refresh

2. **Demo Account**
   - Backend supports demo account creation
   - Endpoint: `POST /demo-account`

## 6. Real-time Features

### WebSocket Updates
- Market price updates
- Position P&L updates
- Order book changes

### Integration Status
- WebSocket connection established in `backend_integration.js`
- Real-time price updates functional
- Position updates implemented

## 7. External Market Integration

### Polymarket
- Proxy endpoint: `/api/polymarket/markets`
- Search functionality implemented
- Market data fetching working

### Kalshi
- UI supports Kalshi market display
- Backend integration pending

## 8. Feature Implementation Priority

### Critical Features Missing
1. **Position Closing** - Users cannot close positions
2. **Stop Limits** - Risk management incomplete
3. **Quantum Positions** - Core feature not implemented

### Nice-to-Have Features
1. Market creation
2. MMT staking
3. Liquidity pool participation

## 9. Code Locations

### Backend
- **API Handlers**: `/api_runner/src/handlers.rs`
- **RPC Client**: `/api_runner/src/rpc_client.rs`
- **Types**: `/api_runner/src/types.rs`
- **Leverage Logic**: `/programs/betting_platform/src/tests/leverage_tests.rs`

### Frontend
- **Main UI**: `/ui_demo/index.html`
- **Platform Logic**: `/ui_demo/platform_main.js`
- **Backend Integration**: `/ui_demo/backend_integration.js`
- **Styles**: `/ui_demo/platform_styles.css`

## 10. Testing Requirements

### User Journey Tests Needed
1. Connect wallet → Search market → Place market order
2. Connect wallet → Search market → Place limit order with stop loss
3. View positions → Track P&L → Close position (when implemented)
4. Select multiple verses → Calculate total leverage → Place order
5. Enable quantum mode → Split position → Track outcomes

### Integration Tests
1. WebSocket real-time updates
2. Polymarket API proxy
3. Leverage calculations with verses
4. Portfolio value calculations

## Conclusion

The UI has been updated to support all backend features, including:
- Dynamic leverage limits based on market outcomes
- Limit orders with stop loss
- Portfolio overview with metrics
- Real-time position tracking

However, several critical backend features remain unimplemented:
- Position closing
- Stop limit execution
- Quantum position creation
- Market creation

These should be prioritized for a complete trading experience.