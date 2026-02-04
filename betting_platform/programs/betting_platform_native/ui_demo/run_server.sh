#!/bin/bash
echo "Starting Quantum Betting Platform UI Demo..."
echo "Server will run on http://localhost:8080"
echo ""
echo "Available pages:"
echo "- Landing Page: http://localhost:8080/index.html"
echo "- Preview (All Features): http://localhost:8080/preview.html"
echo "- Dashboard: http://localhost:8080/app/dashboard.html"
echo "- Markets: http://localhost:8080/app/markets.html"
echo "- Trading Terminal: http://localhost:8080/app/trading.html"
echo "- Portfolio: http://localhost:8080/app/portfolio.html"
echo "- DeFi Hub: http://localhost:8080/app/defi.html"
echo "- Create Market: http://localhost:8080/app/create-market.html"
echo "- Verses: http://localhost:8080/app/verses.html"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""

# Try Python 3 first
if command -v python3 &> /dev/null; then
    python3 -m http.server 8080
# Fall back to Python 2
elif command -v python &> /dev/null; then
    python -m SimpleHTTPServer 8080
# Try Node.js http-server
elif command -v npx &> /dev/null; then
    npx http-server -p 8080
else
    echo "Error: No suitable HTTP server found (Python or Node.js required)"
    exit 1
fi