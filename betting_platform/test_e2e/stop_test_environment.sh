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
