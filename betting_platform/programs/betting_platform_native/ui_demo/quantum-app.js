// Quantum Platform - Real Backend Integration
// Connects the UI to the deployed smart contracts

// Initialize API connection
let api = null;
let ws = null;

// Initialize on load
window.addEventListener('DOMContentLoaded', () => {
    // Initialize API client
    if (window.bettingAPI) {
        api = window.bettingAPI;
        api.initWebSocket();
        setupWebSocketListeners();
    }
    
    // Add real backend integration to existing functions
    enhanceWithRealBackend();
});

// Setup WebSocket listeners
function setupWebSocketListeners() {
    if (!api) return;
    
    // Market updates
    api.on('marketUpdate', (data) => {
        updateMarketPrices(data);
    });
    
    // Trade notifications
    api.on('tradeExecuted', (data) => {
        showTradeNotification(data);
    });
}

// Enhance existing functions with real backend
function enhanceWithRealBackend() {
    // Override selectMarket to fetch real data
    const originalSelectMarket = window.selectMarket;
    window.selectMarket = async function(market) {
        originalSelectMarket(market);
        
        // Fetch real market data if available
        if (api) {
            try {
                const realMarket = await api.getMarket(market.id);
                if (realMarket) {
                    // Update with real data
                    market.volume = realMarket.volume || market.volume;
                    market.prices = realMarket.prices || market.prices;
                }
            } catch (error) {
                console.log('Using demo data for market:', market.id);
            }
        }
    };
    
    // Override executeOrders to use real blockchain
    const originalExecuteOrders = window.executeOrders;
    window.executeOrders = async function() {
        const btn = document.getElementById('finalExecuteBtn');
        btn.disabled = true;
        btn.innerHTML = '<div class="loading"></div> Executing on Blockchain...';
        
        try {
            if (api && window.walletAdapter) {
                // Connect wallet if not connected
                if (!window.walletAdapter.connected) {
                    const result = await window.walletAdapter.connect('demo');
                    if (!result.connected) {
                        throw new Error('Wallet connection failed');
                    }
                }
                
                // Get trade parameters
                const investment = parseFloat(document.getElementById('investmentAmount').value) || 0;
                const leverage = window.leverage || 5;
                
                // Execute main market trade
                if (window.selectedMarket) {
                    const tradeResult = await api.placeTrade({
                        market_id: window.selectedMarket.id,
                        amount: investment / (1 + window.selectedVerses.length),
                        outcome: window.selectedMarket.outcomes[0], // Default to first outcome
                        leverage: leverage
                    });
                    
                    console.log('Main trade executed:', tradeResult);
                }
                
                // Execute verse trades
                for (const verse of window.selectedVerses) {
                    const verseResult = await api.placeTrade({
                        market_id: verse.id,
                        amount: investment / (1 + window.selectedVerses.length),
                        outcome: 'yes',
                        leverage: leverage * verse.multiplier
                    });
                    
                    console.log('Verse trade executed:', verseResult);
                }
                
                // Show success
                btn.innerHTML = '✓ Executed on Solana Blockchain';
                btn.style.background = 'linear-gradient(135deg, #4CD964 0%, #32D74B 100%)';
                
                // Animate money flow
                animateMoneyFlow();
            } else {
                // Fallback to original demo
                originalExecuteOrders();
            }
        } catch (error) {
            console.error('Execution error:', error);
            btn.innerHTML = '❌ Execution Failed';
            btn.style.background = 'linear-gradient(135deg, #FF3B30 0%, #FF6B6B 100%)';
            btn.disabled = false;
        }
    };
}

// Update market prices from real data
function updateMarketPrices(data) {
    // Find market in the markets object
    Object.values(markets).forEach(categoryMarkets => {
        const market = categoryMarkets.find(m => m.id === data.market_id);
        if (market) {
            market.prices = data.prices || market.prices;
            market.volume = data.volume || market.volume;
            
            // Update UI if this market is displayed
            const marketCards = document.querySelectorAll('.market-card');
            marketCards.forEach(card => {
                if (card.querySelector('.market-title').textContent === market.title) {
                    const volumeElement = card.querySelector('.stat-value');
                    if (volumeElement) {
                        volumeElement.textContent = `$${(market.volume / 1000000).toFixed(1)}M`;
                    }
                }
            });
        }
    });
}

// Show trade notification
function showTradeNotification(data) {
    const notification = document.createElement('div');
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        background: linear-gradient(135deg, #4CD964 0%, #32D74B 100%);
        color: white;
        padding: 20px;
        border-radius: 10px;
        box-shadow: 0 10px 30px rgba(0,0,0,0.3);
        z-index: 10000;
        animation: slideIn 0.3s ease-out;
    `;
    
    notification.innerHTML = `
        <div style="font-weight: 600; margin-bottom: 5px;">Trade Executed!</div>
        <div>Amount: $${data.amount}</div>
        <div>Signature: ${data.signature.substring(0, 8)}...</div>
    `;
    
    document.body.appendChild(notification);
    
    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease-out';
        setTimeout(() => notification.remove(), 300);
    }, 5000);
}

// Add CSS animations
const style = document.createElement('style');
style.textContent = `
    @keyframes slideIn {
        from {
            transform: translateX(100%);
            opacity: 0;
        }
        to {
            transform: translateX(0);
            opacity: 1;
        }
    }
    
    @keyframes slideOut {
        from {
            transform: translateX(0);
            opacity: 1;
        }
        to {
            transform: translateX(100%);
            opacity: 0;
        }
    }
`;
document.head.appendChild(style);

// Export for debugging
window.quantumAPI = {
    api,
    executeRealTrade: async (marketId, amount) => {
        if (!api) {
            console.error('API not initialized');
            return;
        }
        
        try {
            const result = await api.placeTrade({
                market_id: marketId,
                amount: amount,
                outcome: 'yes',
                leverage: 5
            });
            console.log('Trade result:', result);
            return result;
        } catch (error) {
            console.error('Trade error:', error);
            throw error;
        }
    }
};