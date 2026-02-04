# Quantum Betting Platform - Frontend

## Overview

This is the frontend implementation of the Quantum Betting Platform, featuring:
- Native Solana integration (no Anchor)
- Polymarket and Kalshi market integration
- Hierarchical verse system with leverage multiplication
- Quantum superposition betting
- Real-time market data and order execution
- Stop loss and take profit controls

## Architecture

### Core Modules

1. **solana_integration.js** - Native Solana blockchain interactions
   - Wallet connection (Phantom, Solflare)
   - PDA derivation for verses, markets, and positions
   - Transaction building and execution
   - Account subscriptions

2. **market_data.js** - Market data fetching and normalization
   - Polymarket API integration
   - Kalshi API integration
   - Data normalization to internal format
   - Real-time WebSocket updates

3. **verse_system.js** - Hierarchical verse management
   - Dynamic verse tree structure
   - Leverage multiplication calculations
   - Market-verse matching algorithms
   - Custom verse creation

4. **trading_interface.js** - Trading logic and order management
   - Order placement and execution
   - Position monitoring
   - Risk management (stop loss, take profit)
   - Portfolio calculations

5. **quantum_mode.js** - Quantum superposition calculations
   - Quantum state representation
   - Amplitude calculations from probabilities
   - Entanglement between positions
   - Quantum enhancement factors

6. **platform_ui.html** - Main user interface
   - Three-panel layout (wallet/verses, markets, trading)
   - Real-time updates and animations
   - Responsive design

7. **platform_styles.css** - Design system
   - John Ive-inspired minimalist design
   - Orange color scheme
   - Smooth animations and transitions

## Running the Platform

### Prerequisites
- Node.js installed
- A Solana wallet browser extension (Phantom recommended)
- SOL tokens for testing (use devnet faucet)

### Steps

1. Start the development server:
```bash
node server.js
```

2. Open your browser and navigate to:
```
http://localhost:8080
```

3. Connect your wallet and start trading!

## Using the Platform

### 1. Connect Wallet
Click "Connect Wallet" to connect your Phantom or Solflare wallet.

### 2. Import Market
Paste a Polymarket or Kalshi URL into the market input field and click "Fetch".
Example URLs:
- `https://polymarket.com/event/will-bitcoin-reach-100k-by-2025`
- `https://kalshi.com/markets/BTC-100K`

### 3. Select Verses
Choose verses from the left panel to multiply your leverage. Each verse adds a multiplier that compounds with others.

### 4. Configure Position
- Set your investment amount in SOL
- Choose between regular or quantum mode
- Set stop loss and take profit levels
- Select market or limit order

### 5. Execute Trade
Review your position summary and click "Execute Order" to place the trade on-chain.

## Quantum Mode

When enabled, quantum mode:
- Splits your position across all outcomes using quantum superposition
- Calculates amplitudes as √probability for each outcome
- Provides quantum enhancement based on uncertainty
- Allows entanglement between multiple positions

## API Integration

The platform integrates with:
- **Polymarket CLOB API**: For decentralized prediction markets
- **Kalshi API**: For regulated event contracts
- **Solana RPC**: For blockchain interactions

## Security Notes

- Never share your private keys
- Always verify transaction details before signing
- Use devnet for testing before mainnet
- The platform stores minimal data locally (only wallet address)

## Development

### File Structure
```
betting/
├── platform_ui.html         # Main UI
├── solana_integration.js    # Blockchain logic
├── market_data.js          # Market data fetching
├── verse_system.js         # Verse hierarchy
├── trading_interface.js    # Trading logic
├── quantum_mode.js         # Quantum calculations
├── platform_styles.css     # Styles
├── server.js              # Dev server
└── PLATFORM_README.md     # This file
```

### Adding New Features

1. **New Market Source**: Add to `market_data.js`
2. **New Verse Type**: Update `verse_system.js`
3. **New Order Type**: Modify `trading_interface.js`
4. **UI Changes**: Update `platform_ui.html` and `platform_styles.css`

## Troubleshooting

### Wallet Not Connecting
- Ensure wallet extension is installed
- Check that wallet is unlocked
- Try refreshing the page

### Market Not Loading
- Verify the URL is from Polymarket or Kalshi
- Check browser console for errors
- Ensure CORS is not blocking requests

### Transaction Failing
- Check wallet has sufficient SOL
- Verify you're on the correct network
- Review transaction details in wallet

## Next Steps

1. Deploy smart contracts to devnet
2. Update program ID in `solana_integration.js`
3. Test with real market data
4. Add more verse types and strategies
5. Implement historical data and analytics

## License

See main project LICENSE file.