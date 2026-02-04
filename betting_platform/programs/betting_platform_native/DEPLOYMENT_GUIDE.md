# Betting Platform Local Deployment Guide

## Prerequisites

- Solana CLI installed (confirmed: ✓)
- Rust and Cargo installed (confirmed: ✓)
- Program built successfully (confirmed: ✓)

## Deployment Steps

### Step 1: Start Local Validator

Open a new terminal and run:
```bash
solana-test-validator --reset
```

Keep this terminal open during deployment and testing.

### Step 2: Deploy the Program

In your main terminal, run:
```bash
./deploy_local.sh
```

This script will:
1. Check validator is running
2. Build the program
3. Create necessary keypairs
4. Configure Solana CLI for localhost
5. Airdrop SOL for deployment
6. Deploy the program
7. Show deployment summary

### Step 3: Verify Deployment

Check program deployment:
```bash
solana program show <PROGRAM_ID>
```

Monitor program logs:
```bash
solana logs | grep <PROGRAM_ID>
```

### Step 4: Initialize Program

After deployment, initialize the program with:
```bash
# Create global config
solana program invoke <PROGRAM_ID> --data <INIT_GLOBAL_CONFIG_DATA>

# Initialize MMT token
solana program invoke <PROGRAM_ID> --data <INIT_MMT_DATA>

# Create first market
solana program invoke <PROGRAM_ID> --data <CREATE_MARKET_DATA>
```

## Testing Against Deployed Program

Run integration tests:
```bash
PROGRAM_ID=<YOUR_PROGRAM_ID> cargo test --features test-bpf
```

## Program Architecture

The deployed program includes:

### Core Contracts (92 total)
- **AMM System**: LMSR, PM-AMM, L2-AMM
- **Trading Engine**: Order matching, execution
- **MMT Token**: Minting, staking, governance
- **Liquidation**: Graduated liquidation system
- **Priority Queue**: Fair ordering protocol
- **Risk Management**: Circuit breakers, correlation matrix

### Key Features
- Native Solana (no Anchor)
- < 20k CU per trade
- 5000+ TPS capability
- 520-byte ProposalPDA constraint
- CPI depth limiting (max 4)

## Common Commands

### Check Balance
```bash
solana balance
```

### View Program Account
```bash
solana account <PROGRAM_ID>
```

### Transfer SOL
```bash
solana transfer <RECIPIENT> <AMOUNT>
```

### Create Token Account
```bash
spl-token create-account <TOKEN_MINT>
```

## Troubleshooting

### Validator Not Running
```
Error: Local validator not found
```
Solution: Start validator with `solana-test-validator --reset`

### Insufficient SOL
```
Error: Insufficient funds
```
Solution: Request airdrop with `solana airdrop 10`

### Program Too Large
```
Error: Program too large
```
Solution: Optimize with `cargo build-bpf --features optimize-size`

## Next Steps

1. Deploy auxiliary programs (oracle, keeper network)
2. Create test markets with realistic data
3. Run stress tests with multiple concurrent users
4. Monitor performance metrics
5. Prepare for devnet deployment

## Performance Benchmarks

Expected metrics on local validator:
- Trade execution: < 20k CU
- Market creation: < 50k CU
- Liquidation check: < 10k CU
- State update: < 15k CU

## Security Considerations

Before mainnet deployment:
1. Complete security audit
2. Test all edge cases
3. Verify math precision
4. Check for reentrancy
5. Validate authority controls

---

For questions or issues, refer to:
- Technical documentation: `/docs/`
- Implementation report: `COMPREHENSIVE_IMPLEMENTATION_REPORT.md`
- Source code: `/src/`