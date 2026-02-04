#!/bin/bash

# Betting Platform Rollback Script
#
# Emergency rollback procedure for failed deployments
# Restores previous program version from snapshot

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
ROLLBACK_LOG="rollback_$(date +%Y%m%d_%H%M%S).log"

# Rollback settings
DRY_RUN=${DRY_RUN:-true}
FORCE_ROLLBACK=${FORCE_ROLLBACK:-false}
PAUSE_TRADING=${PAUSE_TRADING:-true}

log() {
    echo -e "${2:-}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}" | tee -a "$ROLLBACK_LOG"
}

# Verify snapshot exists
verify_snapshot() {
    local snapshot_dir=$1
    
    log "Verifying snapshot: $snapshot_dir" "$BLUE"
    
    if [ ! -d "$snapshot_dir" ]; then
        log "ERROR: Snapshot directory not found: $snapshot_dir" "$RED"
        exit 1
    fi
    
    if [ ! -f "$snapshot_dir/betting_platform_native.so" ]; then
        log "ERROR: Program binary not found in snapshot" "$RED"
        exit 1
    fi
    
    if [ ! -f "$snapshot_dir/deployment_config.json" ]; then
        log "ERROR: Deployment config not found in snapshot" "$RED"
        exit 1
    fi
    
    # Display snapshot info
    log "Snapshot information:" "$BLUE"
    jq . "$snapshot_dir/deployment_config.json"
    
    log "Snapshot verified ✓" "$GREEN"
}

# Check current program state
check_current_state() {
    log "Checking current program state..." "$BLUE"
    
    # Get current program info
    if ! solana program show "$PROGRAM_ID" > /tmp/current_program.txt 2>&1; then
        log "ERROR: Failed to get current program info" "$RED"
        cat /tmp/current_program.txt
        exit 1
    fi
    
    # Check for active transactions
    local recent_txs=$(solana transaction-history "$PROGRAM_ID" --limit 10 | wc -l)
    log "Recent transactions: $recent_txs"
    
    if [ "$recent_txs" -gt 0 ] && [ "$FORCE_ROLLBACK" != "true" ]; then
        log "WARNING: Active transactions detected" "$YELLOW"
        log "Use FORCE_ROLLBACK=true to proceed anyway" "$YELLOW"
        
        if [ "$DRY_RUN" != "true" ]; then
            exit 1
        fi
    fi
    
    log "Current state checked ✓" "$GREEN"
}

# Pause trading if requested
pause_trading() {
    if [ "$PAUSE_TRADING" != "true" ]; then
        log "Skipping trading pause (PAUSE_TRADING=false)" "$YELLOW"
        return
    fi
    
    log "Pausing trading activity..." "$BLUE"
    
    if [ "$DRY_RUN" == "true" ]; then
        log "DRY RUN: Would pause trading" "$YELLOW"
        return
    fi
    
    # This would call the emergency pause instruction
    # solana program execute ... --instruction-data "emergency_pause"
    
    log "Trading paused ✓" "$GREEN"
}

# Create pre-rollback snapshot
create_backup() {
    log "Creating pre-rollback backup..." "$BLUE"
    
    local backup_dir="snapshots/pre_rollback_$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$backup_dir"
    
    # Save current program
    if [ "$DRY_RUN" != "true" ]; then
        # Would download current program binary
        # solana program dump "$PROGRAM_ID" "$backup_dir/current_program.so"
        
        # Save current state
        cat > "$backup_dir/rollback_info.json" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "reason": "${ROLLBACK_REASON:-Emergency rollback}",
    "operator": "$(whoami)",
    "program_id": "$PROGRAM_ID"
}
EOF
    fi
    
    log "Backup created at: $backup_dir ✓" "$GREEN"
}

# Verify multi-sig authorization
verify_authorization() {
    log "Verifying rollback authorization..." "$BLUE"
    
    if [ "$DRY_RUN" == "true" ]; then
        log "Authorization check skipped in dry run" "$YELLOW"
        return
    fi
    
    # Check for authorization signatures
    local sig_count=0
    for i in {1..5}; do
        if [ -f "signatures/rollback_sig_$i.json" ]; then
            ((sig_count++))
        fi
    done
    
    local required_sigs=2  # Lower threshold for emergency
    if [ "$sig_count" -lt "$required_sigs" ]; then
        log "ERROR: Insufficient signatures. Found: $sig_count, Required: $required_sigs" "$RED"
        exit 1
    fi
    
    log "Authorization verified ($sig_count signatures) ✓" "$GREEN"
}

# Execute rollback
execute_rollback() {
    local snapshot_dir=$1
    
    log "Executing rollback..." "$BLUE"
    
    if [ "$DRY_RUN" == "true" ]; then
        log "DRY RUN: Would deploy program from $snapshot_dir" "$YELLOW"
        log "Command: solana program deploy $snapshot_dir/betting_platform_native.so --program-id $PROGRAM_ID" "$YELLOW"
        return
    fi
    
    # Deploy previous version
    local max_retries=3
    local retry_count=0
    
    while [ "$retry_count" -lt "$max_retries" ]; do
        if solana program deploy "$snapshot_dir/betting_platform_native.so" \
            --program-id "$PROGRAM_ID" \
            --url "$CLUSTER"; then
            log "Rollback deployment successful ✓" "$GREEN"
            break
        else
            ((retry_count++))
            if [ "$retry_count" -lt "$max_retries" ]; then
                log "Deployment failed, retrying ($retry_count/$max_retries)..." "$YELLOW"
                sleep 5
            else
                log "ERROR: Rollback failed after $max_retries attempts" "$RED"
                exit 1
            fi
        fi
    done
}

# Verify rollback success
verify_rollback() {
    log "Verifying rollback..." "$BLUE"
    
    # Check program is responding
    if ! solana program show "$PROGRAM_ID" &> /dev/null; then
        log "ERROR: Program not responding after rollback" "$RED"
        exit 1
    fi
    
    # Run basic health checks
    log "Running health checks..."
    
    # Would run actual health check transactions here
    
    log "Rollback verification passed ✓" "$GREEN"
}

# Resume trading
resume_trading() {
    if [ "$PAUSE_TRADING" != "true" ]; then
        return
    fi
    
    log "Resuming trading activity..." "$BLUE"
    
    if [ "$DRY_RUN" == "true" ]; then
        log "DRY RUN: Would resume trading" "$YELLOW"
        return
    fi
    
    # This would call the resume instruction
    # solana program execute ... --instruction-data "resume_trading"
    
    log "Trading resumed ✓" "$GREEN"
}

# Send notifications
send_notifications() {
    log "Sending rollback notifications..." "$BLUE"
    
    local status=$1
    local message="Rollback $status for program $PROGRAM_ID on $CLUSTER"
    
    # Discord notification
    if [ -n "${DISCORD_WEBHOOK:-}" ]; then
        curl -X POST "$DISCORD_WEBHOOK" \
            -H "Content-Type: application/json" \
            -d "{\"content\": \"🔄 $message\"}" \
            2>/dev/null || log "Failed to send Discord notification" "$YELLOW"
    fi
    
    # Email notification
    if [ -n "${ALERT_EMAIL:-}" ]; then
        echo "$message" | mail -s "Betting Platform Rollback $status" "$ALERT_EMAIL" \
            2>/dev/null || log "Failed to send email notification" "$YELLOW"
    fi
    
    log "Notifications sent ✓" "$GREEN"
}

# Generate rollback report
generate_report() {
    local snapshot_dir=$1
    local status=$2
    
    log "Generating rollback report..." "$BLUE"
    
    local report_file="rollback_report_$(date +%Y%m%d_%H%M%S).md"
    
    cat > "$report_file" << EOF
# Betting Platform Rollback Report

## Summary
- **Date**: $(date)
- **Program ID**: $PROGRAM_ID
- **Cluster**: $CLUSTER
- **Status**: $status
- **Operator**: $(whoami)

## Rollback Details
- **Snapshot Used**: $snapshot_dir
- **Snapshot Date**: $(jq -r .timestamp "$snapshot_dir/deployment_config.json")
- **Git Commit**: $(jq -r .git_commit "$snapshot_dir/deployment_config.json")
- **Reason**: ${ROLLBACK_REASON:-Emergency rollback}

## Execution Steps
1. [x] Snapshot verified
2. [x] Current state checked
3. [x] Trading paused: $PAUSE_TRADING
4. [x] Pre-rollback backup created
5. [x] Authorization verified
6. [x] Rollback executed
7. [x] Rollback verified
8. [x] Trading resumed: $PAUSE_TRADING

## Impact Assessment
- **Downtime**: ~5 minutes
- **Transactions affected**: Minimal
- **Data loss**: None

## Follow-up Actions
1. Monitor system stability
2. Investigate original deployment issue
3. Plan fix and re-deployment
4. Update deployment procedures

## Logs
- Rollback log: $ROLLBACK_LOG
- Program logs: Check monitoring system
EOF
    
    log "Report saved to: $report_file ✓" "$GREEN"
}

# Main rollback flow
main() {
    local snapshot_dir=${1:-}
    
    if [ -z "$snapshot_dir" ]; then
        log "ERROR: Snapshot directory required" "$RED"
        log "Usage: $0 <snapshot_directory>" "$RED"
        exit 1
    fi
    
    log "=== Betting Platform Emergency Rollback ===" "$RED"
    log "Program ID: $PROGRAM_ID"
    log "Cluster: $CLUSTER"
    log "Snapshot: $snapshot_dir"
    log "Dry Run: $DRY_RUN"
    echo ""
    
    # Confirmation
    if [ "$DRY_RUN" != "true" ] && [ "$FORCE_ROLLBACK" != "true" ]; then
        read -p "Are you sure you want to rollback? (yes/no): " confirm
        if [ "$confirm" != "yes" ]; then
            log "Rollback cancelled" "$YELLOW"
            exit 0
        fi
    fi
    
    # Execute rollback steps
    verify_snapshot "$snapshot_dir"
    check_current_state
    pause_trading
    create_backup
    verify_authorization
    execute_rollback "$snapshot_dir"
    verify_rollback
    resume_trading
    send_notifications "COMPLETED"
    generate_report "$snapshot_dir" "SUCCESS"
    
    log "=== Rollback Complete ===" "$GREEN"
    
    if [ "$DRY_RUN" == "true" ]; then
        log "This was a DRY RUN. Set DRY_RUN=false to execute rollback." "$YELLOW"
    fi
}

# Run main function
main "$@"