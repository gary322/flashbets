#!/bin/bash

# Verification script to check deployment readiness

set -e

echo "====================================="
echo "🔍 DEPLOYMENT VERIFICATION"
echo "====================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Counters
TOTAL_CHECKS=0
PASSED_CHECKS=0

check_item() {
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    if [ "$2" = "true" ]; then
        echo -e "${GREEN}✅ $1${NC}"
        PASSED_CHECKS=$((PASSED_CHECKS + 1))
    else
        echo -e "${RED}❌ $1${NC}"
    fi
}

# Check Polygon contracts
echo "📦 Checking Polygon Contracts..."
check_item "BettingPlatform.sol exists" "$(test -f contracts/polygon/core/BettingPlatform.sol && echo true || echo false)"
check_item "PolymarketIntegration.sol exists" "$(test -f contracts/polygon/core/PolymarketIntegration.sol && echo true || echo false)"
check_item "MarketFactory.sol exists" "$(test -f contracts/polygon/core/MarketFactory.sol && echo true || echo false)"
check_item "FlashBetting.sol exists" "$(test -f contracts/polygon/flash/FlashBetting.sol && echo true || echo false)"
check_item "LeverageVault.sol exists" "$(test -f contracts/polygon/defi/LeverageVault.sol && echo true || echo false)"
check_item "LiquidityPool.sol exists" "$(test -f contracts/polygon/defi/LiquidityPool.sol && echo true || echo false)"
check_item "MockUSDC.sol exists" "$(test -f contracts/polygon/mocks/MockUSDC.sol && echo true || echo false)"
check_item "MockAavePool.sol exists" "$(test -f contracts/polygon/mocks/MockAavePool.sol && echo true || echo false)"
echo ""

# Check Solana programs
echo "📦 Checking Solana Programs..."
check_item "Main program Cargo.toml exists" "$(test -f programs/betting_platform_native/Cargo.toml && echo true || echo false)"
check_item "Main program lib.rs exists" "$(test -f programs/betting_platform_native/src/lib.rs && echo true || echo false)"
check_item "Flash program Cargo.toml exists" "$(test -f flash_bets/program/Cargo.toml && echo true || echo false)"
echo ""

# Check deployment infrastructure
echo "🔧 Checking Deployment Infrastructure..."
check_item "Hardhat config exists" "$(test -f contracts/hardhat.config.js && echo true || echo false)"
check_item "Package.json exists" "$(test -f contracts/package.json && echo true || echo false)"
check_item "Deploy script exists" "$(test -f contracts/scripts/deploy.js && echo true || echo false)"
check_item "Main deployment script exists" "$(test -f deploy_all_contracts.sh && echo true || echo false)"
check_item "Main deployment script is executable" "$(test -x deploy_all_contracts.sh && echo true || echo false)"
echo ""

# Check documentation
echo "📚 Checking Documentation..."
check_item "Deployment README exists" "$(test -f DEPLOYMENT_README.md && echo true || echo false)"
check_item "CLAUDE.md exists" "$(test -f ../CLAUDE.md && echo true || echo false)"
echo ""

# Check system dependencies
echo "🔨 Checking System Dependencies..."
check_item "Node.js installed" "$(command -v node &> /dev/null && echo true || echo false)"
check_item "npm installed" "$(command -v npm &> /dev/null && echo true || echo false)"
check_item "Solana CLI installed" "$(command -v solana &> /dev/null && echo true || echo false)"
check_item "Anchor installed" "$(command -v anchor &> /dev/null && echo true || echo false)"
check_item "Rust installed" "$(command -v rustc &> /dev/null && echo true || echo false)"
check_item "Cargo installed" "$(command -v cargo &> /dev/null && echo true || echo false)"
echo ""

# Summary
echo "====================================="
echo "📊 VERIFICATION SUMMARY"
echo "====================================="
echo "Total checks: $TOTAL_CHECKS"
echo "Passed: $PASSED_CHECKS"
echo "Failed: $((TOTAL_CHECKS - PASSED_CHECKS))"
echo ""

if [ $PASSED_CHECKS -eq $TOTAL_CHECKS ]; then
    echo -e "${GREEN}✅ ALL CHECKS PASSED!${NC}"
    echo ""
    echo "🚀 Ready to deploy! Run:"
    echo "   ./deploy_all_contracts.sh"
else
    echo -e "${YELLOW}⚠️  Some checks failed. Please review above.${NC}"
    echo ""
    echo "Missing dependencies can be installed:"
    echo "  - Node.js: https://nodejs.org/"
    echo "  - Solana: sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\""
    echo "  - Anchor: cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked"
    echo "  - Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi
echo ""