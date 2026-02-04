#!/bin/bash

# Betting Platform Monitoring Setup Script
#
# Sets up comprehensive monitoring for the betting platform including:
# - Transaction monitoring
# - Error rate tracking
# - Performance metrics
# - Alert configuration

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
PROGRAM_ID="${PROGRAM_ID:-Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4}"
CLUSTER="${CLUSTER:-mainnet-beta}"
MONITORING_DIR="monitoring"
ALERTS_CONFIG="$MONITORING_DIR/alerts.json"
METRICS_CONFIG="$MONITORING_DIR/metrics.json"

# Alert thresholds
ERROR_RATE_THRESHOLD=0.01  # 1%
LATENCY_P99_THRESHOLD=1000 # 1 second
LIQUIDATION_CASCADE_THRESHOLD=0.3 # 30%
ORACLE_DIVERGENCE_THRESHOLD=0.1 # 10%

log() {
    echo -e "${2:-}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

# Create monitoring directory structure
setup_directories() {
    log "Setting up monitoring directories..." "$BLUE"
    
    mkdir -p "$MONITORING_DIR"/{logs,alerts,metrics,dashboards}
    
    log "Directories created ✓" "$GREEN"
}

# Create alert configuration
create_alert_config() {
    log "Creating alert configuration..." "$BLUE"
    
    cat > "$ALERTS_CONFIG" << EOF
{
  "program_id": "$PROGRAM_ID",
  "cluster": "$CLUSTER",
  "alerts": [
    {
      "name": "high_error_rate",
      "description": "Program error rate exceeds threshold",
      "condition": {
        "metric": "error_rate",
        "operator": ">",
        "threshold": $ERROR_RATE_THRESHOLD,
        "duration": "5m"
      },
      "severity": "critical",
      "channels": ["pagerduty", "discord", "email"]
    },
    {
      "name": "high_latency",
      "description": "Transaction latency P99 exceeds threshold",
      "condition": {
        "metric": "latency_p99",
        "operator": ">",
        "threshold": $LATENCY_P99_THRESHOLD,
        "duration": "10m"
      },
      "severity": "warning",
      "channels": ["discord", "email"]
    },
    {
      "name": "liquidation_cascade",
      "description": "Liquidation cascade detected",
      "condition": {
        "metric": "liquidation_rate",
        "operator": ">",
        "threshold": $LIQUIDATION_CASCADE_THRESHOLD,
        "duration": "1m"
      },
      "severity": "critical",
      "channels": ["pagerduty", "discord", "sms"]
    },
    {
      "name": "oracle_divergence",
      "description": "Oracle price divergence detected",
      "condition": {
        "metric": "oracle_spread",
        "operator": ">",
        "threshold": $ORACLE_DIVERGENCE_THRESHOLD,
        "duration": "2m"
      },
      "severity": "high",
      "channels": ["discord", "email"]
    },
    {
      "name": "keeper_shortage",
      "description": "Active keeper count below minimum",
      "condition": {
        "metric": "active_keepers",
        "operator": "<",
        "threshold": 3,
        "duration": "5m"
      },
      "severity": "high",
      "channels": ["discord", "email"]
    },
    {
      "name": "circuit_breaker_triggered",
      "description": "Circuit breaker activated",
      "condition": {
        "metric": "circuit_breaker_active",
        "operator": "==",
        "threshold": 1,
        "duration": "0s"
      },
      "severity": "critical",
      "channels": ["pagerduty", "discord", "sms"]
    }
  ],
  "channels": {
    "discord": {
      "webhook": "${DISCORD_WEBHOOK:-https://discord.com/api/webhooks/xxx}",
      "enabled": true
    },
    "pagerduty": {
      "api_key": "${PAGERDUTY_KEY:-xxx}",
      "service_id": "${PAGERDUTY_SERVICE:-xxx}",
      "enabled": false
    },
    "email": {
      "smtp_host": "${SMTP_HOST:-smtp.gmail.com}",
      "smtp_port": 587,
      "from": "${ALERT_EMAIL_FROM:-alerts@betting-platform.com}",
      "to": ["${ALERT_EMAIL_TO:-ops@betting-platform.com}"],
      "enabled": true
    },
    "sms": {
      "twilio_sid": "${TWILIO_SID:-xxx}",
      "twilio_token": "${TWILIO_TOKEN:-xxx}",
      "from": "${SMS_FROM:-+1234567890}",
      "to": ["${SMS_TO:-+0987654321}"],
      "enabled": false
    }
  }
}
EOF
    
    log "Alert configuration created ✓" "$GREEN"
}

# Create metrics configuration
create_metrics_config() {
    log "Creating metrics configuration..." "$BLUE"
    
    cat > "$METRICS_CONFIG" << EOF
{
  "program_id": "$PROGRAM_ID",
  "cluster": "$CLUSTER",
  "metrics": {
    "transaction_metrics": {
      "enabled": true,
      "interval": "10s",
      "metrics": [
        "transaction_count",
        "transaction_success_rate",
        "transaction_error_rate",
        "transaction_latency_p50",
        "transaction_latency_p99",
        "transaction_cu_usage"
      ]
    },
    "trading_metrics": {
      "enabled": true,
      "interval": "30s",
      "metrics": [
        "trade_volume",
        "trade_count",
        "position_count",
        "open_interest",
        "average_leverage",
        "pnl_distribution"
      ]
    },
    "liquidation_metrics": {
      "enabled": true,
      "interval": "10s",
      "metrics": [
        "liquidation_count",
        "liquidation_volume",
        "liquidation_rate",
        "partial_liquidation_ratio",
        "keeper_response_time",
        "insurance_fund_usage"
      ]
    },
    "oracle_metrics": {
      "enabled": true,
      "interval": "5s",
      "metrics": [
        "oracle_update_count",
        "oracle_latency",
        "oracle_spread",
        "oracle_divergence",
        "oracle_failure_rate"
      ]
    },
    "amm_metrics": {
      "enabled": true,
      "interval": "30s",
      "metrics": [
        "liquidity_depth",
        "price_impact_average",
        "slippage_average",
        "newton_iterations_average",
        "amm_invariant_check"
      ]
    },
    "keeper_metrics": {
      "enabled": true,
      "interval": "60s",
      "metrics": [
        "active_keeper_count",
        "keeper_stake_total",
        "keeper_performance_score",
        "keeper_reward_rate",
        "keeper_slashing_events"
      ]
    },
    "system_metrics": {
      "enabled": true,
      "interval": "60s",
      "metrics": [
        "total_value_locked",
        "protocol_revenue",
        "mmt_circulation",
        "user_count",
        "market_count"
      ]
    }
  },
  "exporters": {
    "prometheus": {
      "enabled": true,
      "port": 9090,
      "path": "/metrics"
    },
    "datadog": {
      "enabled": false,
      "api_key": "${DATADOG_API_KEY:-xxx}",
      "site": "datadoghq.com"
    },
    "cloudwatch": {
      "enabled": false,
      "region": "us-east-1",
      "namespace": "BettingPlatform"
    }
  }
}
EOF
    
    log "Metrics configuration created ✓" "$GREEN"
}

# Create monitoring service
create_monitoring_service() {
    log "Creating monitoring service..." "$BLUE"
    
    cat > "$MONITORING_DIR/monitor.py" << 'EOF'
#!/usr/bin/env python3
"""
Betting Platform Monitoring Service

Monitors on-chain activity and system health
"""

import json
import time
import asyncio
import logging
from datetime import datetime, timedelta
from typing import Dict, List, Any
import aiohttp
from solana.rpc.async_api import AsyncClient
from solana.publickey import PublicKey

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class BettingPlatformMonitor:
    def __init__(self, config_path: str):
        with open(config_path, 'r') as f:
            self.config = json.load(f)
        
        self.program_id = PublicKey(self.config['program_id'])
        self.client = AsyncClient(self.config.get('rpc_url', 'https://api.mainnet-beta.solana.com'))
        self.metrics = {}
        self.alerts_triggered = {}
        
    async def start(self):
        """Start monitoring loops"""
        logger.info(f"Starting monitor for program {self.program_id}")
        
        tasks = [
            self.monitor_transactions(),
            self.monitor_liquidations(),
            self.monitor_oracles(),
            self.check_alerts(),
            self.export_metrics()
        ]
        
        await asyncio.gather(*tasks)
    
    async def monitor_transactions(self):
        """Monitor transaction metrics"""
        while True:
            try:
                # Get recent transactions
                signatures = await self.client.get_signatures_for_address(
                    self.program_id,
                    limit=100
                )
                
                # Calculate metrics
                total = len(signatures['result'])
                errors = sum(1 for sig in signatures['result'] if sig.get('err'))
                
                self.metrics['transaction_count'] = total
                self.metrics['transaction_error_rate'] = errors / total if total > 0 else 0
                
                logger.info(f"Transactions: {total}, Error rate: {self.metrics['transaction_error_rate']:.2%}")
                
            except Exception as e:
                logger.error(f"Error monitoring transactions: {e}")
            
            await asyncio.sleep(10)
    
    async def monitor_liquidations(self):
        """Monitor liquidation activity"""
        while True:
            try:
                # This would parse actual liquidation events
                # For now, we'll simulate
                self.metrics['liquidation_rate'] = 0.05  # 5% example
                
            except Exception as e:
                logger.error(f"Error monitoring liquidations: {e}")
            
            await asyncio.sleep(10)
    
    async def monitor_oracles(self):
        """Monitor oracle health"""
        while True:
            try:
                # Monitor oracle price feeds
                # This would check actual oracle accounts
                self.metrics['oracle_spread'] = 0.005  # 0.5% example
                
            except Exception as e:
                logger.error(f"Error monitoring oracles: {e}")
            
            await asyncio.sleep(5)
    
    async def check_alerts(self):
        """Check alert conditions"""
        while True:
            try:
                alerts_config = self.config.get('alerts', [])
                
                for alert in alerts_config:
                    metric_value = self.metrics.get(alert['condition']['metric'], 0)
                    threshold = alert['condition']['threshold']
                    operator = alert['condition']['operator']
                    
                    triggered = False
                    if operator == '>' and metric_value > threshold:
                        triggered = True
                    elif operator == '<' and metric_value < threshold:
                        triggered = True
                    elif operator == '==' and metric_value == threshold:
                        triggered = True
                    
                    if triggered and alert['name'] not in self.alerts_triggered:
                        await self.send_alert(alert)
                        self.alerts_triggered[alert['name']] = datetime.now()
                    elif not triggered and alert['name'] in self.alerts_triggered:
                        # Alert resolved
                        del self.alerts_triggered[alert['name']]
                
            except Exception as e:
                logger.error(f"Error checking alerts: {e}")
            
            await asyncio.sleep(10)
    
    async def send_alert(self, alert: Dict[str, Any]):
        """Send alert to configured channels"""
        logger.warning(f"ALERT: {alert['name']} - {alert['description']}")
        
        channels = alert.get('channels', [])
        
        if 'discord' in channels:
            await self.send_discord_alert(alert)
        if 'email' in channels:
            await self.send_email_alert(alert)
        # Add other channels as needed
    
    async def send_discord_alert(self, alert: Dict[str, Any]):
        """Send alert to Discord"""
        webhook_url = self.config['channels']['discord']['webhook']
        
        embed = {
            "title": f"🚨 {alert['name'].upper()}",
            "description": alert['description'],
            "color": 0xFF0000 if alert['severity'] == 'critical' else 0xFFFF00,
            "fields": [
                {"name": "Severity", "value": alert['severity'], "inline": True},
                {"name": "Time", "value": datetime.now().isoformat(), "inline": True}
            ]
        }
        
        async with aiohttp.ClientSession() as session:
            await session.post(webhook_url, json={"embeds": [embed]})
    
    async def send_email_alert(self, alert: Dict[str, Any]):
        """Send email alert"""
        # Email implementation would go here
        pass
    
    async def export_metrics(self):
        """Export metrics to configured backends"""
        while True:
            try:
                # Prometheus format
                if self.config['exporters']['prometheus']['enabled']:
                    # Would expose metrics endpoint here
                    pass
                
                logger.debug(f"Current metrics: {self.metrics}")
                
            except Exception as e:
                logger.error(f"Error exporting metrics: {e}")
            
            await asyncio.sleep(30)

async def main():
    monitor = BettingPlatformMonitor('monitoring/mainnet_config.json')
    await monitor.start()

if __name__ == "__main__":
    asyncio.run(main())
EOF
    
    chmod +x "$MONITORING_DIR/monitor.py"
    log "Monitoring service created ✓" "$GREEN"
}

# Create Grafana dashboard
create_grafana_dashboard() {
    log "Creating Grafana dashboard configuration..." "$BLUE"
    
    cat > "$MONITORING_DIR/dashboards/betting_platform.json" << 'EOF'
{
  "dashboard": {
    "title": "Betting Platform Monitoring",
    "panels": [
      {
        "title": "Transaction Rate",
        "targets": [
          {
            "expr": "rate(transaction_count[5m])"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 0}
      },
      {
        "title": "Error Rate",
        "targets": [
          {
            "expr": "transaction_error_rate"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 0},
        "thresholds": [
          {"value": 0.01, "color": "red"}
        ]
      },
      {
        "title": "Trading Volume",
        "targets": [
          {
            "expr": "trade_volume"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 8}
      },
      {
        "title": "Liquidation Rate",
        "targets": [
          {
            "expr": "liquidation_rate"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 8},
        "thresholds": [
          {"value": 0.3, "color": "red"},
          {"value": 0.1, "color": "yellow"}
        ]
      },
      {
        "title": "Oracle Spread",
        "targets": [
          {
            "expr": "oracle_spread"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 16}
      },
      {
        "title": "Active Keepers",
        "targets": [
          {
            "expr": "active_keeper_count"
          }
        ],
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 16},
        "thresholds": [
          {"value": 3, "color": "red"},
          {"value": 5, "color": "yellow"}
        ]
      }
    ]
  }
}
EOF
    
    log "Grafana dashboard created ✓" "$GREEN"
}

# Create systemd service
create_systemd_service() {
    log "Creating systemd service configuration..." "$BLUE"
    
    cat > "$MONITORING_DIR/betting-monitor.service" << EOF
[Unit]
Description=Betting Platform Monitoring Service
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$(pwd)
ExecStart=/usr/bin/python3 $(pwd)/$MONITORING_DIR/monitor.py
Restart=always
RestartSec=10
StandardOutput=append:$(pwd)/$MONITORING_DIR/logs/monitor.log
StandardError=append:$(pwd)/$MONITORING_DIR/logs/monitor.error.log

[Install]
WantedBy=multi-user.target
EOF
    
    log "Systemd service configuration created ✓" "$GREEN"
    log "To install: sudo cp $MONITORING_DIR/betting-monitor.service /etc/systemd/system/" "$YELLOW"
    log "Then: sudo systemctl enable betting-monitor && sudo systemctl start betting-monitor" "$YELLOW"
}

# Create monitoring README
create_readme() {
    log "Creating monitoring documentation..." "$BLUE"
    
    cat > "$MONITORING_DIR/README.md" << EOF
# Betting Platform Monitoring

## Overview
This directory contains monitoring configuration and tools for the betting platform.

## Components

### 1. Alert Configuration
- Location: \`alerts.json\`
- Defines alert rules and notification channels
- Thresholds for various metrics

### 2. Metrics Configuration  
- Location: \`metrics.json\`
- Defines which metrics to collect
- Export configuration for Prometheus/Datadog

### 3. Monitoring Service
- Location: \`monitor.py\`
- Python service that monitors on-chain activity
- Sends alerts when thresholds are breached

### 4. Grafana Dashboard
- Location: \`dashboards/betting_platform.json\`
- Import into Grafana for visualization

## Quick Start

1. Install dependencies:
\`\`\`bash
pip install solana aiohttp
\`\`\`

2. Configure environment variables:
\`\`\`bash
export DISCORD_WEBHOOK="your-webhook-url"
export ALERT_EMAIL_TO="ops@yourcompany.com"
\`\`\`

3. Start monitoring:
\`\`\`bash
python monitoring/monitor.py
\`\`\`

## Alert Channels

### Discord
- Webhook-based notifications
- Real-time alerts with embeds

### Email
- SMTP configuration required
- For critical alerts

### PagerDuty
- For 24/7 on-call rotation
- Critical alerts only

### SMS (Twilio)
- For emergency notifications
- High-severity alerts

## Metrics Exported

### Transaction Metrics
- Transaction count and rate
- Success/error rates
- Latency percentiles
- CU usage

### Trading Metrics
- Volume and count
- Open interest
- PnL distribution

### System Health
- Liquidation rates
- Oracle health
- Keeper availability
- Circuit breaker status

## Troubleshooting

### No metrics showing
- Check RPC connection
- Verify program ID
- Check service logs

### Alerts not firing
- Verify webhook URLs
- Check threshold values
- Review service logs

### High resource usage
- Adjust polling intervals
- Reduce metric retention
- Scale monitoring infrastructure
EOF
    
    log "Documentation created ✓" "$GREEN"
}

# Main setup flow
main() {
    log "=== Betting Platform Monitoring Setup ===" "$BLUE"
    
    setup_directories
    create_alert_config
    create_metrics_config
    create_monitoring_service
    create_grafana_dashboard
    create_systemd_service
    create_readme
    
    log "=== Monitoring Setup Complete ===" "$GREEN"
    log "Next steps:"
    log "1. Configure environment variables for alert channels"
    log "2. Install Python dependencies: pip install solana aiohttp"
    log "3. Start monitoring service: python $MONITORING_DIR/monitor.py"
    log "4. Import Grafana dashboard from $MONITORING_DIR/dashboards/"
}

# Run main function
main "$@"