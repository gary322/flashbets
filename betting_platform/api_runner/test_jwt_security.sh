#!/bin/bash

echo "Testing JWT Security Implementation..."
echo "======================================"

# Test JWT generation
echo -e "\n1. Testing JWT secret generation:"
cargo test security::jwt_generator::tests::test_generate_jwt_secret --lib -- --nocapture

echo -e "\n2. Testing JWT validation:"
cargo test security::jwt_generator::tests::test_validate_jwt_secret --lib -- --nocapture

echo -e "\n3. Testing auth module with secure JWT:"
cargo test auth::tests::test_token_generation --lib -- --nocapture

echo -e "\n4. Checking if .env JWT secret is rejected:"
JWT_SECRET="your-secret-key-must-be-at-least-32-characters-long" cargo run --bin api_runner 2>&1 | head -20 | grep -E "JWT|WARNING|validation"

echo -e "\n5. Testing production JWT generation:"
rm -f .jwt_secret
unset JWT_SECRET
cargo run --bin api_runner 2>&1 | head -20 | grep -E "JWT|Server|listening"