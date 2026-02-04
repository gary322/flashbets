#!/bin/bash

echo "🍎 Opening Quantum Betting Platform UI (John Ive Design)"
echo "=================================================="
echo ""
echo "Opening UI pages in your default browser..."
echo ""

# Get the current directory
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Open the preview page in the default browser
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    open "file://$DIR/preview.html"
    echo "✅ Opened Preview Page"
    echo ""
    echo "📱 Other pages you can open:"
    echo "• Landing: file://$DIR/index.html"
    echo "• Dashboard: file://$DIR/app/dashboard.html"
    echo "• Create Market: file://$DIR/app/create-market.html"
    echo "• Verse Management: file://$DIR/app/verses.html"
    echo "• Markets: file://$DIR/app/markets.html"
    echo "• Trading: file://$DIR/app/trading.html"
    echo "• Portfolio: file://$DIR/app/portfolio.html"
    echo "• DeFi Hub: file://$DIR/app/defi.html"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    xdg-open "file://$DIR/preview.html"
    echo "✅ Opened Preview Page"
else
    echo "Please open the following file in your browser:"
    echo "file://$DIR/preview.html"
fi

echo ""
echo "✨ Features:"
echo "• John Ive/Apple-inspired design"
echo "• Clean typography with Apple system fonts"
echo "• Professional dark theme"
echo "• Users can add verses to markets"
echo "• Complete UI implementation"