# Keeper Setup Guide

## Table of Contents
1. [Overview](#overview)
2. [Requirements](#requirements)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Running a Keeper](#running-a-keeper)
6. [Monitoring & Maintenance](#monitoring--maintenance)
7. [Rewards & Economics](#rewards--economics)
8. [Advanced Strategies](#advanced-strategies)
9. [Troubleshooting](#troubleshooting)

## Overview

Keepers are essential participants in the Betting Platform ecosystem who perform critical maintenance tasks:
- **Liquidations**: Execute position liquidations when coverage falls below threshold
- **Price Updates**: Relay oracle price feeds to the platform
- **Stop Loss Execution**: Trigger stop loss orders
- **State Pruning**: Clean up expired data
- **Chain Execution**: Process multi-leg chain positions

Running a keeper can be profitable through rewards but requires technical expertise and capital commitment.

## Requirements

### Technical Requirements
- **Hardware**:
  - CPU: 4+ cores (8+ recommended)
  - RAM: 16GB minimum (32GB recommended)
  - Storage: 500GB SSD
  - Network: 100Mbps+ stable connection
  - Uptime: 99%+ availability

- **Software**:
  - Ubuntu 20.04+ or similar Linux distribution
  - Docker 20.10+
  - Node.js 18+ or Rust 1.70+
  - Solana CLI tools

### Financial Requirements
- **MMT Stake**: Minimum 10,000 MMT tokens
- **Operating Capital**: 10-50 SOL for transaction fees
- **Liquidation Capital**: 100+ SOL for liquidation execution

### Knowledge Requirements
- Understanding of Solana blockchain
- Basic DevOps skills
- Risk management knowledge
- Monitoring and alerting setup

## Installation

### Option 1: Docker Installation (Recommended)

```bash
# Pull the official keeper image
docker pull bettingplatform/keeper:latest

# Create configuration directory
mkdir -p ~/betting-keeper/config
mkdir -p ~/betting-keeper/logs

# Download default configuration
wget https://raw.githubusercontent.com/betting-platform/keeper/main/config/default.yaml \
  -O ~/betting-keeper/config/keeper.yaml

# Create docker-compose.yml
cat > ~/betting-keeper/docker-compose.yml << EOF
version: '3.8'
services:
  keeper:
    image: bettingplatform/keeper:latest
    container_name: betting-keeper
    restart: unless-stopped
    volumes:
      - ./config:/app/config
      - ./logs:/app/logs
      - ~/.config/solana:/root/.config/solana:ro
    environment:
      - NODE_ENV=production
      - LOG_LEVEL=info
    ports:
      - "9090:9090"  # Metrics port
    networks:
      - keeper-network

  prometheus:
    image: prom/prometheus:latest
    container_name: keeper-prometheus
    restart: unless-stopped
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9091:9090"
    networks:
      - keeper-network

networks:
  keeper-network:
    driver: bridge
EOF
```

### Option 2: Native Installation

```bash
# Clone keeper repository
git clone https://github.com/betting-platform/keeper.git
cd keeper

# Install dependencies
npm install  # For TypeScript keeper
# OR
cargo build --release  # For Rust keeper

# Copy configuration
cp config/default.yaml config/keeper.yaml

# Install as systemd service
sudo cp scripts/betting-keeper.service /etc/systemd/system/
sudo systemctl enable betting-keeper
```

## Configuration

### Basic Configuration

Edit `~/betting-keeper/config/keeper.yaml`:

```yaml
# Keeper Configuration
keeper:
  # Your keeper keypair path
  keypair_path: ~/.config/solana/keeper-keypair.json
  
  # Stake account (must have 10k+ MMT staked)
  stake_account: "YourStakeAccountPubkey"
  
  # Operation modes
  modes:
    liquidations: true
    price_updates: true
    stop_loss: true
    state_pruning: true
    chain_execution: true

# Network Configuration
network:
  cluster: mainnet-beta
  rpc_url: https://api.mainnet-beta.solana.com
  ws_url: wss://api.mainnet-beta.solana.com
  
  # Backup RPC endpoints
  backup_rpcs:
    - https://solana-api.projectserum.com
    - https://rpc.ankr.com/solana

# Program Configuration
program:
  id: Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4
  
  # Monitoring intervals (ms)
  intervals:
    liquidation_check: 1000      # 1 second
    price_update_check: 5000     # 5 seconds
    stop_loss_check: 2000        # 2 seconds
    state_pruning_check: 60000   # 1 minute

# Performance Configuration
performance:
  # Maximum concurrent operations
  max_concurrent_liquidations: 5
  max_concurrent_updates: 10
  
  # Transaction settings
  priority_fee: 10000  # microlamports
  max_retries: 3
  retry_delay: 1000    # ms
  
  # Compute limits
  max_cu_per_tx: 400000
  preflight_commitment: confirmed

# Risk Management
risk:
  # Maximum exposure per liquidation
  max_liquidation_size: 1000000000000  # 1000 SOL
  
  # Minimum profit threshold (bps)
  min_profit_threshold: 50  # 0.5%
  
  # Gas price limits
  max_gas_price: 100000  # microlamports
  
  # Position limits
  max_positions_per_market: 10
  max_total_exposure: 10000000000000  # 10k SOL

# Monitoring
monitoring:
  # Metrics endpoint
  metrics_port: 9090
  
  # Health check
  health_check_port: 8080
  
  # Alerting
  alerts:
    discord_webhook: "https://discord.com/api/webhooks/..."
    pagerduty_key: "your-pagerduty-key"
    
  # Thresholds
  thresholds:
    min_sol_balance: 10000000000  # 10 SOL
    min_profit_rate: 0.001        # 0.1% daily
    max_error_rate: 0.01          # 1%
```

### Advanced Configuration

```yaml
# Strategy Configuration
strategies:
  liquidation:
    # Liquidation selection strategy
    selection: "highest_profit"  # or "highest_risk", "fifo"
    
    # Partial liquidation preference
    prefer_partial: true
    partial_percentage: 30  # 30% liquidation
    
    # Competition settings
    gas_bidding:
      enabled: true
      max_bid_multiplier: 2.0
      dynamic_adjustment: true
    
  price_update:
    # Oracle sources priority
    sources:
      - "polymarket"
      - "chainlink"
      - "pyth"
    
    # Aggregation method
    aggregation: "median"  # or "mean", "vwap"
    
    # Staleness threshold
    max_staleness: 300  # 5 minutes

# Performance Optimizations
optimizations:
  # Transaction batching
  batching:
    enabled: true
    max_batch_size: 5
    batch_timeout: 500  # ms
  
  # Caching
  cache:
    enabled: true
    ttl: 5000  # 5 seconds
    max_entries: 10000
  
  # Parallel processing
  parallelism:
    enabled: true
    worker_threads: 4
```

## Running a Keeper

### Starting the Keeper

```bash
# Using Docker
cd ~/betting-keeper
docker-compose up -d

# Check logs
docker logs -f betting-keeper

# Using native installation
systemctl start betting-keeper
journalctl -u betting-keeper -f
```

### Initial Setup

1. **Register as Keeper**:
```bash
# Register your keeper on-chain
keeper-cli register \
  --stake-amount 10000 \
  --keeper-keypair ~/.config/solana/keeper-keypair.json
```

2. **Verify Registration**:
```bash
keeper-cli status --keeper <your-keeper-pubkey>
```

3. **Fund Operating Wallet**:
```bash
# Transfer SOL for operations
solana transfer <keeper-address> 50 --keypair ~/.config/solana/wallet.json
```

### Monitoring Dashboard

Access the keeper dashboard:
- Metrics: http://localhost:9090/metrics
- Health: http://localhost:8080/health
- Grafana: http://localhost:3000 (if configured)

## Monitoring & Maintenance

### Key Metrics to Monitor

1. **Performance Metrics**:
   - Liquidations per hour
   - Success rate (>95% target)
   - Average response time (<2 slots)
   - Profit margin

2. **System Metrics**:
   - CPU usage (<80%)
   - Memory usage (<80%)
   - Network latency (<100ms)
   - RPC request rate

3. **Financial Metrics**:
   - SOL balance
   - MMT rewards earned
   - Gas costs
   - Net profit

### Alerts Configuration

```yaml
# prometheus-alerts.yml
groups:
  - name: keeper_alerts
    interval: 30s
    rules:
      - alert: LowSOLBalance
        expr: keeper_sol_balance < 5e9
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Keeper SOL balance low"
          
      - alert: HighErrorRate
        expr: rate(keeper_errors_total[5m]) > 0.01
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          
      - alert: MissedLiquidations
        expr: keeper_missed_liquidations_total > 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Missed liquidation opportunity"
```

### Maintenance Tasks

**Daily**:
- Check SOL balance
- Review error logs
- Verify keeper performance

**Weekly**:
- Update keeper software
- Review and optimize configuration
- Analyze profit/loss
- Clean up old logs

**Monthly**:
- Security audit
- Performance optimization
- Strategy adjustment
- Backup configuration

## Rewards & Economics

### Reward Structure

1. **Liquidation Rewards**:
   - Base: 1% of liquidation size
   - Performance bonus: Up to 0.5% extra
   - Partial liquidation: 0.3% bonus

2. **Price Update Rewards**:
   - 100 MMT per successful update
   - Accuracy bonus: 50 MMT for <0.1% deviation

3. **Stop Loss Execution**:
   - 0.5% of position size
   - Speed bonus: 0.1% if executed within 1 slot

4. **State Pruning**:
   - 10 MMT per pruned position
   - Batch bonus: 2x for 100+ positions

### Profit Calculation

```typescript
// Example daily profit calculation
const dailyStats = {
  liquidations: {
    count: 50,
    totalSize: 10000 * 1e9, // 10k SOL
    avgReward: 0.01, // 1%
  },
  priceUpdates: {
    count: 1000,
    rewardPerUpdate: 100 * 1e6, // 100 MMT
  },
  costs: {
    gas: 5 * 1e9, // 5 SOL
    infrastructure: 0.5 * 1e9, // 0.5 SOL
  }
};

const liquidationProfit = dailyStats.liquidations.totalSize * dailyStats.liquidations.avgReward;
const updateProfit = dailyStats.priceUpdates.count * dailyStats.priceUpdates.rewardPerUpdate;
const totalCosts = dailyStats.costs.gas + dailyStats.costs.infrastructure;

const netProfit = liquidationProfit - totalCosts; // In SOL
const mmtEarned = updateProfit; // In MMT
```

### ROI Optimization

1. **Stake Optimization**:
   - Higher stake = higher tier = better rewards
   - Diamond tier: 30% reward boost

2. **Strategy Optimization**:
   - Focus on high-value liquidations
   - Batch operations to reduce gas
   - Use priority fees strategically

3. **Infrastructure Optimization**:
   - Colocate with RPC nodes
   - Use dedicated RPC endpoints
   - Implement efficient caching

## Advanced Strategies

### 1. MEV Protection

```yaml
# MEV protection configuration
mev_protection:
  enabled: true
  
  # Private mempool submission
  private_mempool:
    enabled: true
    endpoint: "https://private-mempool.example.com"
  
  # Flashbots-style bundles
  bundles:
    enabled: true
    max_bundle_size: 5
  
  # Timing randomization
  timing:
    randomize_submission: true
    jitter_ms: 0-500
```

### 2. Multi-Region Setup

```yaml
# Multi-region configuration
regions:
  primary:
    location: "us-east-1"
    rpc: "https://us-east.rpc.example.com"
    
  backups:
    - location: "eu-west-1"
      rpc: "https://eu-west.rpc.example.com"
    - location: "ap-southeast-1"
      rpc: "https://ap-southeast.rpc.example.com"
      
  failover:
    enabled: true
    health_check_interval: 5000
    switch_threshold: 3  # Failed health checks
```

### 3. Liquidation Sniping

```typescript
// Advanced liquidation strategy
class LiquidationSniper {
  async evaluateOpportunity(position: Position): Promise<boolean> {
    // Calculate expected profit
    const liquidationReward = position.size * 0.01;
    const gasCost = await this.estimateGasCost();
    const competitionFactor = await this.assessCompetition(position);
    
    const expectedProfit = liquidationReward - gasCost - competitionFactor;
    
    // Dynamic threshold based on market conditions
    const threshold = this.calculateDynamicThreshold();
    
    return expectedProfit > threshold;
  }
  
  async executeWithMEV(position: Position) {
    // Build transaction
    const tx = await this.buildLiquidationTx(position);
    
    // Add priority fee based on competition
    const priorityFee = await this.calculateOptimalPriorityFee(position);
    
    // Submit via private mempool
    return await this.submitPrivate(tx, priorityFee);
  }
}
```

## Troubleshooting

### Common Issues

#### 1. "Insufficient stake" Error
```bash
# Check stake amount
keeper-cli stake-info --keeper <pubkey>

# Add more stake
keeper-cli add-stake --amount 5000
```

#### 2. High Gas Costs
- Reduce `max_concurrent_operations`
- Increase `min_profit_threshold`
- Enable transaction batching
- Use off-peak hours

#### 3. Missed Liquidations
- Decrease `liquidation_check_interval`
- Increase `priority_fee`
- Check RPC latency
- Add backup RPC endpoints

#### 4. Connection Issues
```bash
# Test RPC connection
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
  https://api.mainnet-beta.solana.com

# Use backup RPC
export SOLANA_RPC_URL=https://solana-api.projectserum.com
```

### Debug Mode

Enable detailed logging:

```yaml
logging:
  level: debug
  
  # Log categories
  categories:
    liquidation: debug
    price_update: info
    performance: debug
    errors: debug
  
  # Output
  outputs:
    - type: console
      format: json
    - type: file
      path: /var/log/keeper/debug.log
      rotate: daily
```

### Performance Tuning

```bash
# Monitor performance
keeper-cli perf --duration 1h

# Optimize based on results
keeper-cli optimize --target profit
keeper-cli optimize --target reliability
```

## Support & Resources

### Documentation
- API Docs: https://docs.betting-platform.io/keeper
- GitHub: https://github.com/betting-platform/keeper
- Examples: https://github.com/betting-platform/keeper-examples

### Community
- Discord: https://discord.gg/betting-keepers
- Telegram: https://t.me/betting_keepers
- Forum: https://forum.betting-platform.io/keepers

### Professional Support
- Enterprise: enterprise@betting-platform.io
- Technical: keeper-support@betting-platform.io
- Emergency: +1-XXX-XXX-XXXX (24/7)

---

*Note: Running a keeper involves financial risk. Start with small amounts and thoroughly test your configuration before scaling up.*