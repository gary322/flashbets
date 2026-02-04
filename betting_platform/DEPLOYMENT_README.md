# 🚀 Betting Platform - Complete Deployment Guide

## Overview

This is a **production-ready** betting platform deployment with:
- **Polygon Smart Contracts**: Core betting, Polymarket integration, flash betting, DeFi features
- **Solana Programs**: Native implementation with flash betting module
- **No mocks or placeholders** in production logic
- **Full ABI/IDL generation** for backend integration
- **500x effective leverage** through innovative chaining mechanisms

## 📋 Prerequisites

### Required Software
- Node.js v16+ and npm
- Solana CLI tools (v1.16+)
- Anchor framework (v0.28+)
- Rust (latest stable)
- Git

### Installation Commands
```bash
# Install Solana
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Install Anchor
cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked

# Install Node dependencies
npm install -g hardhat
```

## 🏗️ Project Structure

```
betting_platform/
├── contracts/                  # Polygon smart contracts
│   ├── polygon/
│   │   ├── core/              # Main platform contracts
│   │   │   ├── BettingPlatform.sol
│   │   │   ├── PolymarketIntegration.sol
│   │   │   └── MarketFactory.sol
│   │   ├── flash/             # Flash betting (5-60s markets)
│   │   │   └── FlashBetting.sol
│   │   ├── defi/              # DeFi features
│   │   │   ├── LeverageVault.sol (500x leverage)
│   │   │   └── LiquidityPool.sol (Multiple AMMs)
│   │   └── mocks/             # Test helpers only
│   ├── scripts/
│   │   └── deploy.js          # Polygon deployment script
│   ├── abi/                   # Generated ABIs
│   └── hardhat.config.js
│
├── programs/                   # Solana programs
│   └── betting_platform_native/
│       └── src/
│           └── lib.rs         # Main Solana program
│
├── flash_bets/                # Flash betting module
│   └── program/
│       └── src/
│           └── lib.rs         # Flash betting Solana program
│
├── idl/                       # Generated IDL files
├── backend_integration.js     # Backend connection helper
├── test_deployment.js         # Deployment verification
└── deploy_all_contracts.sh    # Main deployment script
```

## 🚀 Quick Start

### 1. One-Command Deployment

```bash
# Deploy everything locally
./deploy_all_contracts.sh
```

This will:
- ✅ Deploy all Polygon contracts to local Hardhat network
- ✅ Deploy all Solana programs to local validator
- ✅ Generate ABI files for Polygon contracts
- ✅ Generate IDL files for Solana programs
- ✅ Create backend integration files
- ✅ Set up test scripts

### 2. Verify Deployment

```bash
# Test the deployment
node test_deployment.js
```

### 3. Backend Integration

Use the generated `backend_integration.js`:

```javascript
const { 
  getPolygonContract, 
  initSolanaConnection,
  addresses 
} = require('./backend_integration');

// Use Polygon contracts
const { signer } = initPolygonProvider();
const bettingPlatform = getPolygonContract('BettingPlatform', signer);

// Use Solana programs
const connection = initSolanaConnection();
const programId = addresses.solana.bettingPlatform;
```

## 📦 Deployed Contracts

### Polygon Contracts

| Contract | Purpose | Key Features |
|----------|---------|--------------|
| **BettingPlatform** | Main betting logic | Position management, liquidations, Polymarket hooks |
| **PolymarketIntegration** | Polymarket CTF Exchange | Direct integration with 0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E |
| **MarketFactory** | Market creation | Binary, categorical, scalar, flash markets |
| **FlashBetting** | 5-60 second markets | Micro-tau AMM, 3-step chaining, ZK resolution |
| **LeverageVault** | 500x leverage | Tiered system, Aave integration, liquidations |
| **LiquidityPool** | AMM liquidity | LMSR, PM-AMM, L2-AMM, Hybrid models |

### Solana Programs

| Program | Purpose | Key Features |
|---------|---------|--------------|
| **betting_platform_native** | Main betting program | Verse PDAs, quantum states, native implementation |
| **mv_flash** | Flash betting module | Sub-minute markets, CPI to main, ZK proofs |

## 🔧 Contract Addresses & Program IDs

After deployment, find addresses in:
- **Polygon**: `contracts/deployments/localhost-deployment.json`
- **Solana**: Console output and `idl/*.json` files

## 💻 Using the ABIs and IDLs

### Polygon (Web3/Ethers.js)

```javascript
const BettingPlatformABI = require('./contracts/abi/BettingPlatform.json');
const contract = new ethers.Contract(address, BettingPlatformABI, signer);

// Open a position
await contract.openPosition(
  marketId,
  collateral,
  leverage,
  isLong
);
```

### Solana (Web3.js/Anchor)

```javascript
const IDL = require('./idl/betting_platform.json');
const program = new Program(IDL, programId, provider);

// Create a verse
await program.methods.createVerse(title, category, odds)
  .accounts({
    authority: wallet.publicKey,
    verse: versePDA,
    systemProgram: SystemProgram.programId
  })
  .rpc();
```

## 🎯 Key Features

### 500x Effective Leverage
- Base leverage: 100x
- Chaining multiplier: 5x
- Formula: `base * ∏(mult_i * (1 + (mult_i - 1) * tau))`

### Flash Markets (5-60 seconds)
- Micro-tau pricing: `tau = 0.0001 * (time_left / 60)`
- Sport-specific adjustments
- ZK proof resolution < 10 seconds

### Multiple AMM Models
1. **LMSR**: Logarithmic Market Scoring Rule
2. **PM-AMM**: Polynomial Market AMM
3. **L2-AMM**: Layer 2 optimized
4. **Hybrid**: Weighted combination

### Polymarket Integration
- Direct CTF Exchange integration
- Market mapping and price discovery
- Order placement and settlement

## 🧪 Testing

### Run Contract Tests
```bash
cd contracts
npm test
```

### Run Integration Tests
```bash
node test_deployment.js
```

## 🔐 Security Considerations

- All contracts use OpenZeppelin security primitives
- ReentrancyGuard on all state-changing functions
- Access control with role-based permissions
- Emergency pause functionality
- Liquidation mechanisms for risk management

## 📊 Performance Metrics

- **Transaction Cost**: < 50k CU per trade (Solana)
- **Resolution Time**: < 10 seconds (flash markets)
- **State Size**: ~83KB for 1k flash PDAs
- **Uptime Target**: 99.9%

## 🛠️ Troubleshooting

### Common Issues

1. **"Solana CLI not found"**
   ```bash
   export PATH="/home/user/.local/share/solana/install/active_release/bin:$PATH"
   ```

2. **"Insufficient funds" on Solana**
   ```bash
   solana airdrop 100
   ```

3. **"Contract size too large"**
   - Optimizer is already configured in hardhat.config.js
   - Consider splitting large contracts

4. **Port conflicts**
   - Hardhat: 8545 (change in hardhat.config.js)
   - Solana: 8899 (change with --rpc-port flag)

## 📝 Environment Variables

Create `.env` file:
```env
PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
POLYGON_RPC=http://localhost:8545
SOLANA_RPC=http://localhost:8899
```

## 🚢 Production Deployment

### Polygon Mainnet
```bash
npx hardhat run scripts/deploy.js --network polygon
```

### Solana Mainnet
```bash
solana program deploy target/deploy/betting_platform_native.so --url mainnet-beta
```

## 📚 Additional Resources

- [Polymarket CTF Docs](https://docs.polymarket.com)
- [Solana Program Docs](https://docs.solana.com/developing/on-chain-programs/overview)
- [Hardhat Documentation](https://hardhat.org/docs)
- [Anchor Framework](https://www.anchor-lang.com)

## ⚠️ Important Notes

1. **No Mocks in Production**: Mock contracts are only for local testing
2. **Immutability**: Consider burning upgrade authority in production
3. **Auditing**: Get security audits before mainnet deployment
4. **Rate Limits**: Implement proper rate limiting for API endpoints
5. **Geographic Restrictions**: Handle compliance at the backend level

## 🤝 Support

For issues or questions:
1. Check the troubleshooting section
2. Review test files for usage examples
3. Examine the backend_integration.js for connection patterns

---

**Ready to deploy! Run `./deploy_all_contracts.sh` to begin.**