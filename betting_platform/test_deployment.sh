#!/bin/bash
# test_deployment.sh

# Start local validator
solana-test-validator --reset &
VALIDATOR_PID=$!
sleep 5

# Run deployment
./deploy.sh

# Run verification
npm run verify-deployment

# Cleanup
kill $VALIDATOR_PID