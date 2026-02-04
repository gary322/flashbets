// Minimal Yellow UI - Real Backend Integration

let currentUser = null;
let markets = [];
let selectedOutcome = null;

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    initializeApp();
});

async function initializeApp() {
    // Initialize API and WebSocket
    window.bettingAPI.initWebSocket();
    
    // Set up WebSocket listeners
    setupWebSocketListeners();
    
    // Set up UI event listeners
    setupEventListeners();
    
    // Load initial data
    await loadStats();
    await loadMarkets();
    
    // Show markets by default
    showSection('markets');
}

function setupWebSocketListeners() {
    // Market updates
    window.bettingAPI.on('marketUpdate', (data) => {
        updateMarketInUI(data);
        updateStats();
    });
    
    // Trade notifications
    window.bettingAPI.on('tradeExecuted', (data) => {
        showNotification('Trade Executed', `${data.outcome} @ ${data.price}`);
    });
}

function setupEventListeners() {
    // Navigation
    document.querySelectorAll('.nav-links a').forEach(link => {
        link.addEventListener('click', (e) => {
            e.preventDefault();
            const section = e.target.getAttribute('href').substring(1);
            showSection(section);
        });
    });
    
    // Wallet connection
    document.getElementById('connectWallet').addEventListener('click', connectWallet);
    
    // Trading buttons
    document.getElementById('yesBtn').addEventListener('click', () => selectOutcome('yes'));
    document.getElementById('noBtn').addEventListener('click', () => selectOutcome('no'));
    document.getElementById('placeTrade').addEventListener('click', placeTrade);
}

async function connectWallet() {
    try {
        const result = await window.walletAdapter.connect('demo');
        if (result.connected) {
            currentUser = result.publicKey;
            document.getElementById('connectWallet').style.display = 'none';
            document.getElementById('walletAddress').textContent = 
                result.publicKey.substring(0, 4) + '...' + result.publicKey.substring(result.publicKey.length - 4);
            document.getElementById('walletAddress').style.display = 'block';
            
            // Load user portfolio
            await loadPortfolio();
        }
    } catch (error) {
        showNotification('Error', 'Failed to connect wallet', 'error');
    }
}

async function loadStats() {
    try {
        // For now, use demo data
        document.getElementById('totalVolume').textContent = '$12.5M';
        document.getElementById('activeMarkets').textContent = '42';
        document.getElementById('totalLiquidity').textContent = '$5.2M';
        document.getElementById('activeTraders').textContent = '1,234';
    } catch (error) {
        console.error('Failed to load stats:', error);
    }
}

async function loadMarkets() {
    try {
        const marketsData = await window.bettingAPI.getMarkets();
        markets = marketsData.length > 0 ? marketsData : getDemoMarkets();
        
        // Display markets
        const grid = document.getElementById('marketsGrid');
        grid.innerHTML = markets.map(market => `
            <div class="market-card" data-market-id="${market.id}">
                <h3 class="market-title">${market.title}</h3>
                <div class="market-odds">
                    <div class="odds-button yes">
                        YES ${(market.yesPrice * 100).toFixed(0)}%
                    </div>
                    <div class="odds-button no">
                        NO ${(market.noPrice * 100).toFixed(0)}%
                    </div>
                </div>
                <div style="display: flex; justify-content: space-between; color: var(--text-secondary); font-size: 0.9rem;">
                    <span>Volume: $${(market.volume / 1000).toFixed(1)}k</span>
                    <span>Liquidity: $${(market.liquidity / 1000).toFixed(1)}k</span>
                </div>
                <button class="btn" style="width: 100%; margin-top: 1rem;" onclick="selectMarket(${market.id})">
                    Trade Now
                </button>
            </div>
        `).join('');
        
        // Populate market select
        const select = document.getElementById('marketSelect');
        select.innerHTML = '<option>Select a market...</option>' + 
            markets.map(m => `<option value="${m.id}">${m.title}</option>`).join('');
    } catch (error) {
        console.error('Failed to load markets:', error);
    }
}

function getDemoMarkets() {
    return [
        {
            id: 1,
            title: "Will Bitcoin reach $100k by 2025?",
            yesPrice: 0.34,
            noPrice: 0.66,
            volume: 1234567,
            liquidity: 500000
        },
        {
            id: 2,
            title: "US Presidential Election 2024",
            yesPrice: 0.52,
            noPrice: 0.48,
            volume: 5432100,
            liquidity: 2000000
        },
        {
            id: 3,
            title: "Will AGI be achieved by 2030?",
            yesPrice: 0.23,
            noPrice: 0.77,
            volume: 890123,
            liquidity: 750000
        }
    ];
}

function selectMarket(marketId) {
    document.getElementById('marketSelect').value = marketId;
    showSection('trading');
}

function selectOutcome(outcome) {
    selectedOutcome = outcome;
    document.getElementById('yesBtn').style.opacity = outcome === 'yes' ? '1' : '0.5';
    document.getElementById('noBtn').style.opacity = outcome === 'no' ? '1' : '0.5';
}

async function placeTrade() {
    if (!currentUser) {
        showNotification('Error', 'Please connect wallet first', 'error');
        return;
    }
    
    const marketId = document.getElementById('marketSelect').value;
    const amount = parseFloat(document.getElementById('tradeAmount').value);
    const leverage = parseInt(document.getElementById('leverageSelect').value);
    
    if (!marketId || !selectedOutcome || !amount) {
        showNotification('Error', 'Please fill all fields', 'error');
        return;
    }
    
    try {
        const result = await window.bettingAPI.placeTrade({
            market_id: parseInt(marketId),
            amount: amount,
            outcome: selectedOutcome,
            leverage: leverage
        });
        
        showNotification('Success', 'Trade placed successfully!', 'success');
        
        // Clear form
        document.getElementById('tradeAmount').value = '';
        selectedOutcome = null;
        
        // Reload portfolio
        await loadPortfolio();
    } catch (error) {
        showNotification('Error', 'Failed to place trade', 'error');
    }
}

async function loadPortfolio() {
    if (!currentUser) return;
    
    try {
        const positions = await window.bettingAPI.getPositions(currentUser);
        
        const grid = document.getElementById('positionsGrid');
        if (positions.length === 0) {
            grid.innerHTML = '<p style="text-align: center; color: var(--text-secondary);">No positions yet</p>';
            return;
        }
        
        grid.innerHTML = positions.map(pos => `
            <div class="market-card">
                <h3 class="market-title">${pos.marketTitle}</h3>
                <div style="margin: 1rem 0;">
                    <div style="display: flex; justify-content: space-between;">
                        <span>Position:</span>
                        <span>${pos.outcome.toUpperCase()} @ ${pos.entryPrice}</span>
                    </div>
                    <div style="display: flex; justify-content: space-between;">
                        <span>Amount:</span>
                        <span>$${pos.amount}</span>
                    </div>
                    <div style="display: flex; justify-content: space-between;">
                        <span>Leverage:</span>
                        <span>${pos.leverage}x</span>
                    </div>
                    <div style="display: flex; justify-content: space-between; margin-top: 1rem; font-weight: bold;">
                        <span>P&L:</span>
                        <span style="color: ${pos.pnl >= 0 ? 'var(--success)' : 'var(--error)'}">
                            ${pos.pnl >= 0 ? '+' : ''}$${pos.pnl.toFixed(2)}
                        </span>
                    </div>
                </div>
            </div>
        `).join('');
    } catch (error) {
        console.error('Failed to load portfolio:', error);
    }
}

function updateMarketInUI(data) {
    const marketCard = document.querySelector(`[data-market-id="${data.market_id}"]`);
    if (marketCard) {
        const market = markets.find(m => m.id === data.market_id);
        if (market) {
            market.yesPrice = data.yes_price;
            market.noPrice = data.no_price;
            marketCard.querySelector('.yes').textContent = `YES ${(data.yes_price * 100).toFixed(0)}%`;
            marketCard.querySelector('.no').textContent = `NO ${(data.no_price * 100).toFixed(0)}%`;
        }
    }
}

function showSection(sectionName) {
    // Hide all sections
    document.querySelectorAll('section').forEach(s => s.style.display = 'none');
    
    // Show selected section
    const section = document.getElementById(sectionName);
    if (section) {
        section.style.display = 'block';
    }
    
    // Keep hero visible
    document.querySelector('.hero').style.display = 'block';
}

function showNotification(title, message, type = 'info') {
    // Simple notification (you can enhance this)
    const notification = document.createElement('div');
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        background: ${type === 'error' ? 'var(--error)' : type === 'success' ? 'var(--success)' : 'var(--primary)'};
        color: white;
        padding: 1rem 2rem;
        border-radius: 8px;
        box-shadow: 0 4px 12px rgba(0,0,0,0.3);
        z-index: 1000;
    `;
    notification.innerHTML = `<strong>${title}</strong><br>${message}`;
    document.body.appendChild(notification);
    
    setTimeout(() => notification.remove(), 3000);
}

// Update stats periodically
setInterval(updateStats, 30000);

async function updateStats() {
    // Update with real-time data
    const stats = await window.bettingAPI.getStats();
    if (stats) {
        document.getElementById('totalVolume').textContent = `$${(stats.totalVolume / 1000000).toFixed(1)}M`;
        document.getElementById('activeMarkets').textContent = stats.activeMarkets;
        document.getElementById('totalLiquidity').textContent = `$${(stats.totalLiquidity / 1000000).toFixed(1)}M`;
        document.getElementById('activeTraders').textContent = stats.activeTraders.toLocaleString();
    }
}