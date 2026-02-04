# Betting Platform Deployment Guide

## Table of Contents
1. [Overview](#overview)
2. [Pre-Deployment Checklist](#pre-deployment-checklist)
3. [Deployment Process](#deployment-process)
4. [Monitoring Setup](#monitoring-setup)
5. [Rollback Procedures](#rollback-procedures)
6. [Post-Deployment Verification](#post-deployment-verification)
7. [Operational Runbook](#operational-runbook)
8. [Troubleshooting](#troubleshooting)

## Overview

This guide covers the complete deployment process for the Betting Platform to Solana mainnet. The deployment includes safety checks, monitoring setup, and rollback procedures.

### Key Components
- **Program ID**: `Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4`
- **Network**: Solana Mainnet Beta
- **Required Balance**: Minimum 10 SOL for deployment
- **Multi-sig Required**: 3/5 for deployment, 2/3 for emergency actions

## Pre-Deployment Checklist

### Technical Requirements
- [ ] Solana CLI installed (v1.17+)
- [ ] Rust toolchain (latest stable)
- [ ] Node.js 18+ (for monitoring)
- [ ] Python 3.8+ (for monitoring scripts)
- [ ] 10+ SOL in deployment wallet

### Security Checklist
- [ ] External security audit completed
- [ ] All tests passing (unit, integration, stress)
- [ ] Multi-sig wallets configured
- [ ] Bug bounty program live
- [ ] Incident response plan documented

### Operational Readiness
- [ ] Team on-call schedule defined
- [ ] Monitoring alerts configured
- [ ] Rollback procedures tested
- [ ] Communication channels ready
- [ ] User documentation published

## Deployment Process

### 1. Dry Run Deployment

Always start with a dry run to verify the deployment process:

```bash
cd betting_platform
DRY_RUN=true ./deployment/scripts/deploy_mainnet.sh
```

### 2. Production Deployment

Once dry run succeeds, proceed with actual deployment:

```bash
# Set environment variables
export KEYPAIR_PATH=~/.config/solana/mainnet-deployer.json
export DISCORD_WEBHOOK="https://discord.com/api/webhooks/xxx"
export PAGERDUTY_KEY="your-pagerduty-key"

# Run deployment
DRY_RUN=false \
SKIP_TESTS=false \
SKIP_AUDIT=false \
./deployment/scripts/deploy_mainnet.sh
```

### 3. Multi-Signature Process

For mainnet deployment, collect signatures from authorized signers:

```bash
# Each signer runs:
solana sign-offchain-message deployment_request.json \
    --keypair ~/.config/solana/signer_N.json \
    > signatures/deploy_sig_N.json
```

### 4. Deployment Verification

The deployment script automatically verifies:
- Program deployed successfully
- Correct program ID
- Initialization transactions succeed
- Smoke tests pass

## Monitoring Setup

### 1. Install Monitoring

Set up comprehensive monitoring after deployment:

```bash
./deployment/scripts/setup_monitoring.sh
```

### 2. Configure Alerts

Edit `monitoring/alerts.json` to set thresholds:

```json
{
  "alerts": [
    {
      "name": "high_error_rate",
      "threshold": 0.01,
      "channels": ["pagerduty", "discord"]
    }
  ]
}
```

### 3. Start Monitoring Service

```bash
# Install dependencies
pip install -r monitoring/requirements.txt

# Start service
python monitoring/monitor.py

# Or use systemd
sudo systemctl start betting-monitor
```

### 4. Import Grafana Dashboards

1. Access Grafana at http://localhost:3000
2. Import dashboard from `monitoring/dashboards/betting_platform.json`
3. Configure data source to Prometheus

## Rollback Procedures

### Emergency Rollback

If issues are detected post-deployment:

```bash
# Find the snapshot to rollback to
ls -la snapshots/

# Execute rollback (dry run first)
DRY_RUN=true ./deployment/scripts/rollback_deployment.sh snapshots/20240119_120000

# Execute actual rollback
DRY_RUN=false \
ROLLBACK_REASON="High error rate detected" \
./deployment/scripts/rollback_deployment.sh snapshots/20240119_120000
```

### Rollback Authorization

Emergency rollback requires 2/3 multi-sig:

```bash
# Each signer
solana sign-offchain-message rollback_request.json \
    --keypair ~/.config/solana/emergency_signer_N.json \
    > signatures/rollback_sig_N.json
```

## Post-Deployment Verification

### 1. Initial Health Checks

```bash
# Check program is live
solana program show Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4

# Verify global config initialized
solana account <GLOBAL_CONFIG_PDA>

# Test basic transaction
npm run test:mainnet:smoke
```

### 2. Performance Verification

Monitor initial metrics:
- Transaction success rate > 99%
- Latency P99 < 1 second
- No circuit breakers triggered
- Oracle feeds active

### 3. Progressive Rollout

1. **Hour 1-4**: Monitor closely, limited marketing
2. **Hour 4-24**: Gradual increase in limits
3. **Day 2-7**: Full platform features enabled
4. **Week 2+**: Normal operations

## Operational Runbook

### Daily Operations

1. **Morning Checks** (9 AM UTC)
   - Review overnight alerts
   - Check system metrics
   - Verify oracle health
   - Review keeper performance

2. **Midday Review** (2 PM UTC)
   - Check trading volumes
   - Monitor liquidation rates
   - Review error logs
   - Verify backup systems

3. **End of Day** (9 PM UTC)
   - Daily metrics summary
   - Backup verification
   - Next day preparation

### Weekly Tasks

- [ ] Security scan
- [ ] Performance analysis
- [ ] Capacity planning review
- [ ] Team sync meeting
- [ ] User feedback review

### Emergency Procedures

#### Circuit Breaker Triggered
1. Acknowledge alert immediately
2. Assess market conditions
3. Determine if manual intervention needed
4. Document in incident log

#### Oracle Failure
1. System auto-switches to cache
2. Monitor cache age (<5 minutes)
3. Contact oracle provider
4. Consider market pause if prolonged

#### Cascade Liquidation
1. Circuit breaker auto-activates
2. Partial liquidations preferred
3. Monitor insurance fund usage
4. Prepare market update

## Troubleshooting

### Common Issues

#### Deployment Fails
```bash
# Check error logs
tail -100 deployment_*.log

# Common fixes:
- Insufficient SOL balance
- Program too large (use optimizer)
- RPC rate limits (retry)
```

#### Monitoring Not Working
```bash
# Check service status
systemctl status betting-monitor

# View logs
journalctl -u betting-monitor -f

# Test RPC connection
curl https://api.mainnet-beta.solana.com
```

#### High Error Rate
1. Check specific error types in logs
2. Verify oracle feeds active
3. Check keeper availability
4. Consider emergency pause

### Support Channels

- **Engineering**: #platform-oncall (Slack)
- **Security**: security@betting-platform.com
- **24/7 Hotline**: +1-XXX-XXX-XXXX
- **Escalation**: CTO, Head of Engineering

## Appendix

### Environment Variables

```bash
# Deployment
KEYPAIR_PATH          # Deployer keypair location
PROGRAM_ID            # Program address
CLUSTER               # Network (mainnet-beta)

# Monitoring
DISCORD_WEBHOOK       # Discord alerts
PAGERDUTY_KEY        # PagerDuty integration
ALERT_EMAIL_TO       # Email alerts
DATADOG_API_KEY      # Datadog metrics (optional)

# Operations
ROLLBACK_AUTHORIZED   # Emergency rollback approval
EMERGENCY_PAUSE_KEY   # Pause authority keypair
```

### Key Files

```
deployment/
├── scripts/
│   ├── deploy_mainnet.sh      # Main deployment
│   ├── setup_monitoring.sh    # Monitoring setup
│   └── rollback_deployment.sh # Emergency rollback
├── monitoring/
│   ├── alerts.json           # Alert configuration
│   ├── metrics.json          # Metrics configuration
│   └── monitor.py            # Monitoring service
└── snapshots/                # Deployment snapshots
```

### Security Contacts

- **Security Lead**: security@betting-platform.com
- **Bug Bounty**: https://immunefi.com/bounty/bettingplatform
- **Emergency Multi-sig**: 2/3 required
  - Signer 1: CEO
  - Signer 2: CTO
  - Signer 3: Security Lead

---

**Last Updated**: January 19, 2025  
**Version**: 1.0.0  
**Status**: Production Ready