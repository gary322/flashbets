#!/bin/bash

# Comprehensive Test Environment Setup Script
# Sets up all required services for end-to-end testing

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
LOG_DIR="$SCRIPT_DIR/logs"
PID_DIR="$SCRIPT_DIR/pids"

# Create necessary directories
mkdir -p "$LOG_DIR" "$PID_DIR"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Betting Platform Test Environment Setup${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Start Time: $(date)"
echo ""

# Function to check if a service is running
check_service() {
    local service=$1
    local port=$2
    local max_attempts=30
    local attempt=0
    
    echo -n "Checking $service on port $port..."
    
    while [ $attempt -lt $max_attempts ]; do
        if nc -z localhost $port 2>/dev/null; then
            echo -e " ${GREEN}✓${NC}"
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    
    echo -e " ${RED}✗${NC}"
    return 1
}

# Function to start a service
start_service() {
    local name=$1
    local cmd=$2
    local port=$3
    local log_file="$LOG_DIR/${name}.log"
    local pid_file="$PID_DIR/${name}.pid"
    
    echo -e "${YELLOW}Starting $name...${NC}"
    
    # Check if already running
    if [ -f "$pid_file" ] && kill -0 $(cat "$pid_file") 2>/dev/null; then
        echo -e "${GREEN}$name is already running${NC}"
        return 0
    fi
    
    # Start the service
    eval "$cmd" > "$log_file" 2>&1 &
    local pid=$!
    echo $pid > "$pid_file"
    
    # Wait for service to be ready
    if check_service "$name" "$port"; then
        echo -e "${GREEN}$name started successfully (PID: $pid)${NC}"
        return 0
    else
        echo -e "${RED}Failed to start $name${NC}"
        tail -n 20 "$log_file"
        return 1
    fi
}

# Step 1: Start PostgreSQL
echo -e "\n${BLUE}Step 1: Starting PostgreSQL${NC}"
if command -v pg_ctl > /dev/null 2>&1; then
    # Check if PostgreSQL is already running
    if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
        echo -e "${GREEN}PostgreSQL is already running${NC}"
    else
        # Try to start PostgreSQL
        if [ -d "/usr/local/var/postgres" ]; then
            pg_ctl -D /usr/local/var/postgres start -l "$LOG_DIR/postgresql.log" > /dev/null 2>&1 || true
        fi
        
        if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
            echo -e "${GREEN}PostgreSQL started successfully${NC}"
        else
            echo -e "${YELLOW}Warning: PostgreSQL not running. Install and start manually.${NC}"
        fi
    fi
else
    echo -e "${YELLOW}PostgreSQL not installed. Please install it first.${NC}"
fi

# Step 2: Start Redis
echo -e "\n${BLUE}Step 2: Starting Redis${NC}"
if command -v redis-server > /dev/null 2>&1; then
    start_service "Redis" "redis-server --port 6379" 6379 || {
        echo -e "${YELLOW}Warning: Redis failed to start${NC}"
    }
else
    echo -e "${YELLOW}Redis not installed. Please install it first.${NC}"
fi

# Step 3: Start Solana Test Validator
echo -e "\n${BLUE}Step 3: Starting Solana Test Validator${NC}"
if command -v solana-test-validator > /dev/null 2>&1; then
    start_service "Solana" "solana-test-validator --reset --quiet" 8899 || {
        echo -e "${YELLOW}Warning: Solana test validator failed to start${NC}"
    }
else
    echo -e "${YELLOW}Solana not installed. Please install Solana CLI first.${NC}"
fi

# Step 4: Setup Database
echo -e "\n${BLUE}Step 4: Setting up Database${NC}"
if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
    # Create database if it doesn't exist
    createdb betting_platform 2>/dev/null || echo "Database already exists"
    
    # Create user if it doesn't exist
    psql -d postgres -c "CREATE USER betting_user WITH PASSWORD 'betting_pass';" 2>/dev/null || echo "User already exists"
    psql -d postgres -c "GRANT ALL PRIVILEGES ON DATABASE betting_platform TO betting_user;" 2>/dev/null || true
    
    echo -e "${GREEN}Database setup complete${NC}"
else
    echo -e "${YELLOW}Skipping database setup - PostgreSQL not running${NC}"
fi

# Step 5: Build API Server
echo -e "\n${BLUE}Step 5: Building API Server${NC}"
cd "$PROJECT_ROOT/api_runner"
if cargo build --release 2>&1 | tee "$LOG_DIR/api_build.log" | grep -E "(error|warning)" | tail -20; then
    echo -e "${GREEN}API server built successfully${NC}"
else
    echo -e "${YELLOW}API server build completed with warnings${NC}"
fi

# Step 6: Start API Server
echo -e "\n${BLUE}Step 6: Starting API Server${NC}"
cd "$PROJECT_ROOT/api_runner"

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "Creating .env file..."
    cat > .env << EOF
# Test Environment Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8081
CORS_ORIGINS=http://localhost:3000,http://localhost:3001

# Database
DATABASE_URL=postgresql://betting_user:betting_pass@localhost:5432/betting_platform
DB_MAX_CONNECTIONS=50

# Redis
REDIS_URL=redis://localhost:6379
CACHE_ENABLED=true
QUEUE_ENABLED=true

# Solana
SOLANA_RPC_URL=http://localhost:8899
SOLANA_WS_URL=ws://localhost:8900
PROGRAM_ID=HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca

# Security (TEST ONLY - DO NOT USE IN PRODUCTION)
JWT_SECRET=test-secret-key-minimum-32-characters-long-for-testing
SECURITY_LOG_TO_FILE=true
SECURITY_LOG_PATH=logs/security.log

# External Services
POLYMARKET_ENABLED=false
KALSHI_ENABLED=false

# Auto Funding for Tests
ENABLE_AUTO_FUNDING=true

# Logging
LOG_LEVEL=debug
EOF
fi

start_service "API Server" "cargo run --release" 8081

# Step 7: Check Frontend
echo -e "\n${BLUE}Step 7: Checking Frontend${NC}"
cd "$PROJECT_ROOT"
if [ -d "app" ]; then
    cd app
    echo "Frontend directory found at $PROJECT_ROOT/app"
    
    # Install dependencies if needed
    if [ ! -d "node_modules" ]; then
        echo "Installing frontend dependencies..."
        npm install > "$LOG_DIR/frontend_install.log" 2>&1 || {
            echo -e "${YELLOW}Warning: Frontend dependency installation failed${NC}"
        }
    fi
    
    # Note: We'll start frontend separately to avoid blocking
    echo -e "${YELLOW}To start frontend, run: cd app && npm run dev${NC}"
else
    echo -e "${YELLOW}Frontend directory not found${NC}"
fi

# Step 8: Status Summary
echo -e "\n${BLUE}========================================${NC}"
echo -e "${BLUE}Environment Status Summary${NC}"
echo -e "${BLUE}========================================${NC}"

# Check all services
echo -e "\nService Status:"
check_service "PostgreSQL" 5432 && echo -e "PostgreSQL: ${GREEN}✓ Running${NC}" || echo -e "PostgreSQL: ${RED}✗ Not Running${NC}"
check_service "Redis" 6379 && echo -e "Redis: ${GREEN}✓ Running${NC}" || echo -e "Redis: ${RED}✗ Not Running${NC}"
check_service "Solana" 8899 && echo -e "Solana: ${GREEN}✓ Running${NC}" || echo -e "Solana: ${RED}✗ Not Running${NC}"
check_service "API Server" 8081 && echo -e "API Server: ${GREEN}✓ Running${NC}" || echo -e "API Server: ${RED}✗ Not Running${NC}"

echo -e "\nLog Files:"
echo "- PostgreSQL: $LOG_DIR/postgresql.log"
echo "- Redis: $LOG_DIR/Redis.log"
echo "- Solana: $LOG_DIR/Solana.log"
echo "- API Server: $LOG_DIR/API Server.log"

echo -e "\nPID Files:"
echo "- Located in: $PID_DIR"

echo -e "\n${BLUE}========================================${NC}"
echo -e "${GREEN}Test environment setup complete!${NC}"
echo -e "${BLUE}========================================${NC}"

# Create stop script
cat > "$SCRIPT_DIR/stop_test_environment.sh" << 'EOF'
#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PID_DIR="$SCRIPT_DIR/pids"

echo "Stopping test environment services..."

# Stop services using PID files
for pid_file in "$PID_DIR"/*.pid; do
    if [ -f "$pid_file" ]; then
        service_name=$(basename "$pid_file" .pid)
        pid=$(cat "$pid_file")
        
        if kill -0 $pid 2>/dev/null; then
            echo "Stopping $service_name (PID: $pid)..."
            kill $pid
            rm "$pid_file"
        else
            echo "$service_name not running"
            rm "$pid_file"
        fi
    fi
done

# Stop Solana test validator
pkill -f solana-test-validator 2>/dev/null || true

echo "All services stopped."
EOF

chmod +x "$SCRIPT_DIR/stop_test_environment.sh"

echo -e "\nTo stop all services, run: ${YELLOW}./stop_test_environment.sh${NC}"
echo -e "\nNext step: Run ${YELLOW}./deploy_test_contracts.sh${NC} to deploy smart contracts"