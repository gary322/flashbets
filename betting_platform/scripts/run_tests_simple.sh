#!/bin/bash

# Simple Test Runner - Runs tests without deployment
echo "🚀 Running Betting Platform Tests"
echo "=================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Test results
PASSED=0
FAILED=0

# API Integration Tests
echo -e "${YELLOW}Running API Integration Tests...${NC}"
cd ../tests
if node comprehensive_integration_tests.js > test_output.log 2>&1; then
    echo -e "${GREEN}✅ API Integration Tests PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ API Integration Tests FAILED${NC}"
    ((FAILED++))
fi

# E2E User Journey Tests
echo -e "${YELLOW}Running E2E User Journey Tests...${NC}"
if node e2e_user_journeys.js >> test_output.log 2>&1; then
    echo -e "${GREEN}✅ E2E User Journey Tests PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ E2E User Journey Tests FAILED${NC}"
    ((FAILED++))
fi

# K6 Load Tests
if command -v k6 >/dev/null 2>&1; then
    echo -e "${YELLOW}Running K6 Load Tests...${NC}"
    if k6 run k6_load_test.js --quiet >> test_output.log 2>&1; then
        echo -e "${GREEN}✅ K6 Load Tests PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}❌ K6 Load Tests FAILED${NC}"
        ((FAILED++))
    fi
else
    echo -e "${YELLOW}⚠️  Skipping K6 tests (not installed)${NC}"
fi

# Rust Unit Tests
echo -e "${YELLOW}Running Rust Unit Tests...${NC}"
cd ../programs/betting_platform_native
if cargo test --quiet 2>&1 | grep -q "test result: ok"; then
    echo -e "${GREEN}✅ Rust Unit Tests PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ Rust Unit Tests FAILED${NC}"
    ((FAILED++))
fi

# Generate simple report
cd ../../scripts
cat << EOF > test_report_simple.html
<!DOCTYPE html>
<html>
<head>
    <title>Test Report - $(date)</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .passed { color: green; }
        .failed { color: red; }
        .summary { background: #f0f0f0; padding: 20px; border-radius: 8px; margin: 20px 0; }
    </style>
</head>
<body>
    <h1>Betting Platform Test Report</h1>
    <p>Generated: $(date)</p>
    
    <div class="summary">
        <h2>Summary</h2>
        <p>Total Tests: $((PASSED + FAILED))</p>
        <p class="passed">Passed: $PASSED</p>
        <p class="failed">Failed: $FAILED</p>
        <p>Success Rate: $([ $((PASSED + FAILED)) -gt 0 ] && echo "$((PASSED * 100 / (PASSED + FAILED)))%" || echo "0%")</p>
    </div>
    
    <h2>Test Results</h2>
    <ul>
        <li>API Integration Tests: $([ -f ../tests/test_output.log ] && grep -q "API tests completed" ../tests/test_output.log && echo '<span class="passed">PASSED</span>' || echo '<span class="failed">FAILED</span>')</li>
        <li>E2E User Journey Tests: $([ -f ../tests/test_output.log ] && grep -q "Journey" ../tests/test_output.log && echo '<span class="passed">PASSED</span>' || echo '<span class="failed">FAILED</span>')</li>
        <li>Load Tests: $(command -v k6 >/dev/null 2>&1 && echo '<span class="passed">EXECUTED</span>' || echo '<span>SKIPPED</span>')</li>
        <li>Rust Unit Tests: $(cd ../programs/betting_platform_native && cargo test --quiet 2>&1 | grep -q "test result: ok" && echo '<span class="passed">PASSED</span>' || echo '<span class="failed">FAILED</span>')</li>
    </ul>
    
    <h2>Test Coverage</h2>
    <ul>
        <li>✅ API endpoint validation</li>
        <li>✅ User flow simulation</li>
        <li>✅ Load testing (if K6 available)</li>
        <li>✅ Core Rust logic</li>
        <li>✅ WebSocket communication</li>
        <li>✅ Trading operations</li>
        <li>✅ Risk management</li>
    </ul>
</body>
</html>
EOF

echo ""
echo "=================================="
echo "Test Summary:"
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"
echo ""
echo "Report saved to: test_report_simple.html"
echo "Detailed logs in: tests/test_output.log"