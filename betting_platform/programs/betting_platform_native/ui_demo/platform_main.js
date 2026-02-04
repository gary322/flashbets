// Platform Main JavaScript - Handles all initialization and UI updates

// Global state
const platformState = {
    wallet: null,
    connection: null,
    markets: new Map(),
    positions: new Map(),
    activePositions: [], // Track user's active positions
    selectedMarket: null,
    selectedOutcome: null,
    selectedVerses: [],
    baseLeverage: 5,
    quantumMode: false,
    orderType: 'market'
};

// Load backend integration script first
const backendScript = document.createElement('script');
backendScript.src = 'backend_integration.js';
document.head.appendChild(backendScript);

// Initialize on DOM load
document.addEventListener('DOMContentLoaded', async function() {
    console.log('Platform initializing...');
    
    // Wait for backend API to load
    let retries = 0;
    while (!window.backendAPI && retries < 10) {
        await new Promise(resolve => setTimeout(resolve, 100));
        retries++;
    }
    
    try {
        // Initialize Solana connection
        await initializeSolana();
        
        // Setup all event handlers
        setupEventHandlers();
        
        // Initialize verse tree
        initializeVerseTree();
        
        // Check for saved wallet
        const savedWallet = localStorage.getItem('walletAddress');
        if (savedWallet && window.solana) {
            await connectWallet();
        }
        
        // Add search hint
        const searchInput = document.getElementById('marketSearchInput');
        if (searchInput) {
            searchInput.placeholder = 'Try "Bitcoin", "Trump", "Fed rate", or "AI"...';
        }
        
        // Setup window resize handler for verse view
        let resizeTimeout;
        window.addEventListener('resize', () => {
            clearTimeout(resizeTimeout);
            resizeTimeout = setTimeout(() => {
                if (platformState.selectedMarket) {
                    const verses = getVersesForMarket(platformState.selectedMarket);
                    updateAvailableVerses(verses);
                }
            }, 250);
        });
        
        console.log('Platform initialized successfully');
        
        // Start position price updates
        setInterval(() => {
            if (platformState.activePositions.length > 0) {
                updateActivePositionsDisplay();
            }
        }, 5000); // Update every 5 seconds
        
        // Add demo positions for testing - works without wallet
        addDemoPositions();
        
        // For demo purposes, set a fake wallet so features work
        if (!platformState.wallet) {
            platformState.wallet = { 
                publicKey: { 
                    toString: () => 'DemoWallet1111111111111111111111111111111111' 
                }
            };
            updateWalletDisplay();
        }
        
    } catch (error) {
        console.error('Initialization failed:', error);
        showError('Failed to initialize platform: ' + error.message);
    }
});

// Initialize Solana connection
async function initializeSolana() {
    // Connect to local RPC by default, fallback to devnet
    const rpcUrl = 'http://localhost:8899';
    try {
        platformState.connection = new solanaWeb3.Connection(rpcUrl, 'confirmed');
        await platformState.connection.getVersion();
        console.log('Connected to local Solana RPC');
    } catch (error) {
        console.log('Falling back to devnet');
        platformState.connection = new solanaWeb3.Connection(
            'https://api.devnet.solana.com',
            'confirmed'
        );
    }
    
    // Initialize backend API
    if (window.backendAPI) {
        setupBackendListeners();
        await loadMarketsFromBackend();
        
        // Auto-populate some markets in search if none loaded
        if (platformState.markets.size === 0) {
            // If no markets loaded, fetch directly from Polymarket
            try {
                const polymarkets = await window.backendAPI.fetchPolymarketMarkets();
                polymarkets.forEach(market => {
                    platformState.markets.set(market.id, market);
                });
                console.log('Loaded Polymarket markets:', polymarkets.length);
            } catch (error) {
                console.error('Failed to load Polymarket markets:', error);
            }
        }
    }
}

// Setup backend WebSocket listeners
function setupBackendListeners() {
    window.backendAPI.on('marketUpdate', (data) => {
        updateMarketData(data);
    });
    
    window.backendAPI.on('priceUpdate', (data) => {
        updatePriceDisplay(data);
    });
    
    window.backendAPI.on('tradeExecuted', (data) => {
        showTradeNotification(data);
        loadUserPositions();
    });
    
    window.backendAPI.on('connected', () => {
        showNotification('Connected to backend', 'success');
    });
}

// Load markets from backend
async function loadMarketsFromBackend() {
    try {
        const markets = await window.backendAPI.getMarkets();
        
        // Store markets in platform state
        markets.forEach(market => {
            platformState.markets.set(market.id, market);
        });
        
        console.log(`Loaded ${markets.length} markets from backend`);
        
        // Load verses for each market
        for (const market of markets) {
            const verses = await window.backendAPI.getMarketVerses(market.id);
            market.verses = verses;
        }
        
        return markets;
    } catch (error) {
        console.error('Error loading markets:', error);
        return [];
    }
}

// Setup event handlers
function setupEventHandlers() {
    // Wallet connection
    const connectBtn = document.getElementById('connectWalletBtn');
    if (connectBtn) {
        connectBtn.onclick = connectWallet;
    }
    
    // Quantum toggle
    const quantumToggle = document.getElementById('quantumToggle');
    if (quantumToggle) {
        quantumToggle.onclick = toggleQuantumMode;
    }
    
    // Investment amount
    const investmentInput = document.getElementById('investmentAmount');
    if (investmentInput) {
        investmentInput.oninput = updatePosition;
    }
    
    // Execute button
    const executeBtn = document.getElementById('executeBtn');
    if (executeBtn) {
        executeBtn.onclick = executeOrder;
    }
    
    // Risk controls
    const stopLossSlider = document.getElementById('stopLossSlider');
    if (stopLossSlider) {
        stopLossSlider.oninput = (e) => updateRiskControl('stopLoss', e.target.value);
    }
    
    const takeProfitSlider = document.getElementById('takeProfitSlider');
    if (takeProfitSlider) {
        takeProfitSlider.oninput = (e) => updateRiskControl('takeProfit', e.target.value);
    }
    
    // Click outside to close search results
    document.addEventListener('click', (e) => {
        const searchContainer = document.querySelector('.market-input-section');
        const searchResults = document.getElementById('marketSearchResults');
        
        if (!searchContainer.contains(e.target)) {
            searchResults.style.display = 'none';
        }
    });
}

// Connect wallet
async function connectWallet() {
    try {
        const connectBtn = document.getElementById('connectWalletBtn');
        connectBtn.textContent = 'Connecting...';
        connectBtn.disabled = true;

        // Demo mode - simulate wallet connection
        if (!window.solana) {
            console.log('No Solana wallet found, using demo mode');
            // Simulate wallet connection for demo
            platformState.wallet = { 
                publicKey: { 
                    toString: () => 'DemoWallet' + Math.random().toString(36).substring(7)
                }
            };
            updateWalletDisplay();
            connectBtn.textContent = 'Demo Mode';
            connectBtn.disabled = false;
            return;
        }

        const response = await window.solana.connect();
        platformState.wallet = response.publicKey;
        
        console.log('Wallet connected:', platformState.wallet.toString());
        
        updateWalletDisplay();
        
        // Get balance
        const balance = await platformState.connection.getBalance(platformState.wallet);
        const solBalance = balance / solanaWeb3.LAMPORTS_PER_SOL;
        document.getElementById('balanceAmount').textContent = solBalance.toFixed(4) + ' SOL';
        document.getElementById('balanceDisplay').style.display = 'block';
        
        // Hide connect button
        connectBtn.style.display = 'none';
        
        // Save wallet
        localStorage.setItem('walletAddress', platformState.wallet.toString());
        
        // Load user positions
        await loadUserPositions();
        
        // Show positions section
        document.getElementById('positionsSection').style.display = 'block';
        
    } catch (error) {
        console.error('Wallet connection failed:', error);
        showError('Failed to connect wallet: ' + error.message);
        
        const connectBtn = document.getElementById('connectWalletBtn');
        connectBtn.disabled = false;
        connectBtn.textContent = 'Connect Wallet';
    }
}

// Load user positions
async function loadUserPositions() {
    try {
        // For now, create mock positions for demo
        // In production, this would fetch from blockchain
        const mockPositions = [
            {
                id: 'pos_1',
                market: 'Will BTC reach $100k by 2025?',
                outcome: 'Yes',
                amount: 0.5,
                leverage: 10,
                currentPrice: 0.65,
                entryPrice: 0.60,
                pnl: 0.025,
                pnlPercent: 8.33,
                status: 'open',
                source: 'polymarket'
            },
            {
                id: 'pos_2',
                market: 'Trump GOP Nominee 2024',
                outcome: 'Yes',
                amount: 1.0,
                leverage: 5,
                currentPrice: 0.85,
                entryPrice: 0.80,
                pnl: 0.025,
                pnlPercent: 6.25,
                status: 'open',
                source: 'kalshi'
            }
        ];
        
        // Update positions state
        mockPositions.forEach(pos => {
            platformState.positions.set(pos.id, pos);
        });
        
        // Update UI
        updatePositionsDisplay();
        
    } catch (error) {
        console.error('Failed to load positions:', error);
    }
}

// Update positions display
function updatePositionsDisplay() {
    const container = document.getElementById('positionsContainer');
    if (!container) return;
    
    container.innerHTML = '';
    
    if (platformState.positions.size === 0) {
        container.innerHTML = '<div class="no-positions">No open positions</div>';
        return;
    }
    
    platformState.positions.forEach(position => {
        const posCard = document.createElement('div');
        posCard.className = 'position-card';
        posCard.innerHTML = `
            <div class="position-header">
                <div class="position-market">${position.market}</div>
                <div class="position-source ${position.source}">${position.source}</div>
            </div>
            <div class="position-details">
                <div class="position-outcome">${position.outcome}</div>
                <div class="position-leverage">${position.leverage}x</div>
            </div>
            <div class="position-metrics">
                <div class="position-metric">
                    <span class="metric-label">Entry</span>
                    <span class="metric-value">${(position.entryPrice * 100).toFixed(1)}%</span>
                </div>
                <div class="position-metric">
                    <span class="metric-label">Current</span>
                    <span class="metric-value">${(position.currentPrice * 100).toFixed(1)}%</span>
                </div>
                <div class="position-metric">
                    <span class="metric-label">P&L</span>
                    <span class="metric-value ${position.pnl >= 0 ? 'positive' : 'negative'}">
                        ${position.pnl >= 0 ? '+' : ''}${(position.pnl * position.leverage).toFixed(3)} SOL
                        (${position.pnl >= 0 ? '+' : ''}${position.pnlPercent.toFixed(2)}%)
                    </span>
                </div>
            </div>
            <div class="position-actions">
                <button class="position-btn" onclick="closePosition('${position.id}')">Close</button>
                <button class="position-btn secondary" onclick="viewPosition('${position.id}')">Details</button>
            </div>
        `;
        container.appendChild(posCard);
    });
}

// Search markets
let searchTimeout = null;
async function searchMarkets() {
    const query = document.getElementById('marketSearchInput').value.trim();
    const resultsContainer = document.getElementById('marketSearchResults');
    
    // Clear previous timeout
    if (searchTimeout) {
        clearTimeout(searchTimeout);
    }
    
    if (!query) {
        resultsContainer.style.display = 'none';
        return;
    }
    
    // Debounce search
    searchTimeout = setTimeout(async () => {
        resultsContainer.style.display = 'block';
        resultsContainer.innerHTML = '<div class="search-loading">Searching markets...</div>';
        
        try {
            // Fetch real Polymarket data
            const response = await fetch(`${API_BASE_URL}/polymarket/markets`);
            
            if (response.ok) {
                const polymarkets = await response.json();
                
                // Filter by query
                const filtered = polymarkets.filter(market => {
                    const searchStr = (
                        (market.title || '') + ' ' + 
                        (market.question || '') + ' ' + 
                        (market.description || '')
                    ).toLowerCase();
                    return searchStr.includes(query.toLowerCase());
                }).slice(0, 10);
                
                if (filtered.length > 0) {
                    displaySearchResults(filtered);
                } else {
                    resultsContainer.innerHTML = '<div class="search-empty">No markets found. Try different keywords.</div>';
                }
            } else {
                // Fallback to mock data
                await new Promise(resolve => setTimeout(resolve, 500));
                const markets = getMockSearchResults(query);
                if (markets.length === 0) {
                    resultsContainer.innerHTML = '<div class="search-empty">No markets found. Try different keywords.</div>';
                } else {
                    displaySearchResults(markets);
                }
            }
        } catch (error) {
            console.error('Search failed:', error);
            resultsContainer.innerHTML = '<div class="search-empty">Search failed. Please try again.</div>';
        }
    }, 300);
}

// Get mock search results based on query
function getMockSearchResults(query) {
    const allMarkets = [
        // Crypto markets
        {
            id: 'btc-100k-2025',
            title: 'Will Bitcoin reach $100k by end of 2025?',
            source: 'polymarket',
            category: 'Crypto',
            volume24h: 2500000,
            liquidity: 1000000,
            yesPrice: 0.65,
            outcomes: ['Yes', 'No']
        },
        {
            id: 'eth-5k-2024',
            title: 'Will Ethereum reach $5,000 in 2024?',
            source: 'polymarket',
            category: 'Crypto',
            volume24h: 1800000,
            liquidity: 750000,
            yesPrice: 0.42,
            outcomes: ['Yes', 'No']
        },
        {
            id: 'btc-halving-impact',
            title: 'Bitcoin price 6 months after halving',
            source: 'kalshi',
            category: 'Crypto',
            volume24h: 3200000,
            liquidity: 1500000,
            yesPrice: 0.58,
            outcomes: ['Above $80k', 'Below $80k']
        },
        
        // Politics markets
        {
            id: 'trump-gop-2024',
            title: 'Trump GOP Nominee 2024',
            source: 'kalshi',
            category: 'Politics',
            volume24h: 7000000,
            liquidity: 3000000,
            yesPrice: 0.85,
            outcomes: ['Yes', 'No']
        },
        {
            id: 'biden-reelection',
            title: 'Will Biden run for reelection in 2024?',
            source: 'polymarket',
            category: 'Politics',
            volume24h: 4500000,
            liquidity: 2000000,
            yesPrice: 0.72,
            outcomes: ['Yes', 'No']
        },
        {
            id: 'desantis-president',
            title: 'DeSantis wins 2024 Presidential Election',
            source: 'polymarket',
            category: 'Politics',
            volume24h: 3200000,
            liquidity: 1400000,
            yesPrice: 0.18,
            outcomes: ['Yes', 'No']
        },
        
        // Sports markets
        {
            id: 'superbowl-2024',
            title: 'Super Bowl 2024 Winner',
            source: 'kalshi',
            category: 'Sports',
            volume24h: 5500000,
            liquidity: 2500000,
            yesPrice: 0.35,
            outcomes: ['Chiefs', 'Eagles', 'Bills', 'Cowboys', 'Other']
        },
        {
            id: 'world-cup-2026',
            title: 'USA reaches World Cup 2026 semifinals',
            source: 'polymarket',
            category: 'Sports',
            volume24h: 1200000,
            liquidity: 600000,
            yesPrice: 0.28,
            outcomes: ['Yes', 'No']
        },
        
        // Tech/AI markets
        {
            id: 'agi-2025',
            title: 'AGI achieved by end of 2025',
            source: 'polymarket',
            category: 'Technology',
            volume24h: 4800000,
            liquidity: 2200000,
            yesPrice: 0.12,
            outcomes: ['Yes', 'No']
        },
        {
            id: 'gpt5-release',
            title: 'OpenAI releases GPT-5 in 2024',
            source: 'polymarket',
            category: 'Technology',
            volume24h: 3600000,
            liquidity: 1600000,
            yesPrice: 0.76,
            outcomes: ['Yes', 'No']
        },
        {
            id: 'apple-vr-sales',
            title: 'Apple Vision Pro sells 1M units in first year',
            source: 'kalshi',
            category: 'Technology',
            volume24h: 2100000,
            liquidity: 900000,
            yesPrice: 0.31,
            outcomes: ['Yes', 'No']
        }
    ];
    
    // Filter markets based on query
    const lowerQuery = query.toLowerCase();
    return allMarkets.filter(market => 
        market.title.toLowerCase().includes(lowerQuery) ||
        market.category.toLowerCase().includes(lowerQuery) ||
        market.source.toLowerCase().includes(lowerQuery)
    );
}

// Display search results
function displaySearchResults(markets) {
    const resultsContainer = document.getElementById('marketSearchResults');
    
    resultsContainer.innerHTML = markets.map(market => {
        // Handle Polymarket API format
        const title = market.title || market.question || 'Unknown Market';
        const volume = market.volume24hr || market.volume24h || market.volume || 0;
        const liquidity = market.liquidityNum || market.liquidity || 0;
        
        // Get price from Polymarket format
        let yesPrice = 0.5;
        if (market.outcomePrices) {
            try {
                const prices = JSON.parse(market.outcomePrices);
                yesPrice = parseFloat(prices[0]) || 0.5;
            } catch (e) {
                yesPrice = market.lastTradePrice || 0.5;
            }
        } else if (market.lastTradePrice) {
            yesPrice = market.lastTradePrice;
        } else if (market.yesPrice) {
            yesPrice = market.yesPrice;
        }
        
        // Determine source
        const source = market.source || 'polymarket';
        
        return `
        <div class="search-result-item" onclick="selectSearchResult('${market.id}')">
            <div class="result-market-title">${title}</div>
            <div class="result-market-info">
                <span class="result-source">${source}</span>
                <span>${market.groupItemTitle || market.category || 'General'}</span>
                <span>${(yesPrice * 100).toFixed(0)}% Yes</span>
            </div>
            <div class="result-stats">
                <span class="result-stat">Volume: <strong>$${formatNumber(volume)}</strong></span>
                <span class="result-stat">Liquidity: <strong>$${formatNumber(liquidity)}</strong></span>
            </div>
            ${market.active ? '<span class="active-indicator">Active</span>' : ''}
        </div>
    `}).join('');
}

// Select search result
async function selectSearchResult(marketId) {
    // Hide search results
    document.getElementById('marketSearchResults').style.display = 'none';
    document.getElementById('marketSearchInput').value = '';
    
    try {
        // Fetch the specific market data from Polymarket
        const response = await fetch(`${API_BASE_URL}/api/polymarket/markets`);
        if (response.ok) {
            const markets = await response.json();
            const selectedMarket = markets.find(m => m.id === marketId);
            
            if (selectedMarket) {
                // Convert Polymarket format to our format
                const marketData = {
                    id: selectedMarket.id,
                    title: selectedMarket.title || selectedMarket.question,
                    description: selectedMarket.description || '',
                    category: selectedMarket.groupItemTitle || 'General',
                    outcomes: selectedMarket.outcomes ? JSON.parse(selectedMarket.outcomes) : ['Yes', 'No'],
                    volume: selectedMarket.volume24hr || selectedMarket.volume || 0,
                    liquidity: selectedMarket.liquidityNum || selectedMarket.liquidity || 0,
                    endDate: selectedMarket.endDate,
                    active: selectedMarket.active,
                    source: 'polymarket',
                    verses: selectedMarket.verses || [] // Preserve verses from API
                };
                
                // Process and display the market
                processSelectedMarket(marketData);
                return;
            }
        }
    } catch (error) {
        console.error('Error fetching market details:', error);
    }
    
    // Fallback to mock data
    const markets = getMockSearchResults('');
    const selectedMarket = markets.find(m => m.id === marketId);
    if (selectedMarket) {
        processSelectedMarket(selectedMarket);
    }
}

// Process selected market
function processSelectedMarket(marketData) {
    console.log('Selected market:', marketData);
    
    // Get price information
    let yesPrice = 0.5;
    if (marketData.yesPrice !== undefined) {
        yesPrice = marketData.yesPrice;
    } else if (marketData.lastTradePrice) {
        yesPrice = marketData.lastTradePrice;
    } else if (marketData.outcomePrices) {
        try {
            const prices = JSON.parse(marketData.outcomePrices);
            yesPrice = parseFloat(prices[0]) || 0.5;
        } catch (e) {
            yesPrice = 0.5;
        }
    }
    
    // Format market data consistently
    const formattedMarket = {
        id: marketData.id,
        source: marketData.source || 'polymarket',
        title: marketData.title,
        description: marketData.description || `Market for: ${marketData.title}`,
        outcomes: marketData.outcomes.map((outcome, index) => {
            const outcomeName = typeof outcome === 'string' ? outcome : outcome.name || outcome;
            return {
                name: outcomeName,
                price: index === 0 ? yesPrice : 1 - yesPrice,
                volume: (marketData.volume || 0) / marketData.outcomes.length,
                liquidity: (marketData.liquidity || 0) / marketData.outcomes.length
            };
        }),
        volume24h: marketData.volume || 0,
        liquidity: marketData.liquidity || 0,
        endDate: marketData.endDate || new Date(Date.now() + 90 * 24 * 60 * 60 * 1000),
        category: marketData.category || 'General',
        verses: marketData.verses || [] // Preserve verses from API
    };
    
    console.log('Market data after conversion:', formattedMarket);
    
    // Store and select market
    platformState.markets.set(formattedMarket.id, formattedMarket);
    platformState.selectedMarket = formattedMarket;
    
    // Update display
    updateMarketDisplay(formattedMarket);
}

// Create mock Polymarket data
function createMockPolymarketData(url) {
    const marketId = url.split('/').pop();
    return {
        id: marketId,
        source: 'polymarket',
        title: 'Will BTC reach $100k by end of 2025?',
        description: 'This market will resolve to "Yes" if Bitcoin (BTC) trades at or above $100,000 USD on any major exchange before December 31, 2025.',
        outcomes: [
            { name: 'Yes', price: 0.65, volume: 1250000, liquidity: 500000 },
            { name: 'No', price: 0.35, volume: 1250000, liquidity: 500000 }
        ],
        volume24h: 2500000,
        liquidity: 1000000,
        endDate: new Date('2025-12-31'),
        category: 'Crypto'
    };
}

// Create mock Kalshi data
function createMockKalshiData(url) {
    const marketId = url.split('/').pop();
    return {
        id: marketId,
        source: 'kalshi',
        title: 'Trump GOP Nominee 2024',
        description: 'This market resolves to "Yes" if Donald Trump is the official Republican nominee for the 2024 presidential election.',
        outcomes: [
            { name: 'Yes', price: 0.85, volume: 3500000, liquidity: 1500000 },
            { name: 'No', price: 0.15, volume: 3500000, liquidity: 1500000 }
        ],
        volume24h: 7000000,
        liquidity: 3000000,
        endDate: new Date('2024-07-15'),
        category: 'Politics'
    };
}

// Update market display
function updateMarketDisplay(market) {
    console.log('Updating market display:', market);
    console.log('Market outcomes:', market.outcomes);
    console.log('Outcomes type:', typeof market.outcomes, Array.isArray(market.outcomes));
    
    try {
    document.getElementById('marketTitle').textContent = market.title;
    document.getElementById('marketVolume').textContent = '$' + formatNumber(market.volume24h || market.volume || 0);
    document.getElementById('marketLiquidity').textContent = '$' + formatNumber(market.liquidity || 0);
    
    // Handle endDate - could be string or Date
    const endDate = market.endDate instanceof Date ? market.endDate : new Date(market.endDate || Date.now() + 90 * 24 * 60 * 60 * 1000);
    document.getElementById('marketEndDate').textContent = endDate.toLocaleDateString();
    
    // Update platform badge
    const badge = document.getElementById('platformBadge');
    badge.className = 'platform-badge ' + market.source;
    badge.innerHTML = `<span>${market.source === 'polymarket' ? 'Polymarket' : 'Kalshi'}</span>`;
    
    // Update outcomes
    const outcomeGrid = document.getElementById('outcomeGrid');
    if (outcomeGrid) {
        outcomeGrid.innerHTML = '';
        
        // Ensure outcomes is an array
        let outcomes = market.outcomes;
        if (!Array.isArray(outcomes)) {
            // If outcomes is not an array, create default Yes/No outcomes
            outcomes = ['Yes', 'No'];
        }
        
        outcomes.forEach((outcome, index) => {
            const card = document.createElement('div');
            card.className = 'outcome-card';
            card.onclick = () => selectOutcome(outcome.name || outcome);
            
            const price = outcome.price || (index === 0 ? 0.5 : 0.5);
            const priceChange = price > 0.5 ? '+' : '-';
            const changeClass = price > 0.5 ? '' : 'negative';
            
            card.innerHTML = `
                <div class="outcome-name">${outcome.name || outcome}</div>
                <div class="outcome-price">${(price * 100).toFixed(1)}%</div>
                <div class="outcome-change ${changeClass}">${priceChange}${Math.abs(price - 0.5) * 100}%</div>
                <div class="outcome-volume">Vol: $${formatNumber(outcome.volume || 0)}</div>
            `;
            
            outcomeGrid.appendChild(card);
        });
    }
    
    // Show market header and content
    const marketHeader = document.querySelector('.market-header');
    if (marketHeader) {
        marketHeader.style.display = 'block';
    }
    document.getElementById('marketContent').style.display = 'block';
    document.getElementById('marketLoadingState').style.display = 'none';
    
    // Hide welcome state
    const welcomeState = document.getElementById('welcomeState');
    if (welcomeState) {
        welcomeState.style.display = 'none';
    }
    
    // Update verses for this market
    const verses = getVersesForMarket(market);
    console.log('updateMarketDisplay - verses for market:', market.title, 'count:', verses ? verses.length : 0);
    
    // Only update verses if we have them
    if (verses && verses.length > 0) {
        updateAvailableVerses(verses);
        document.getElementById('versesSection').style.display = 'block';
    } else {
        console.log('No verses available for market:', market.title);
        document.getElementById('versesSection').style.display = 'none';
    }
    
    // Show verse count in market metadata
    const verseInfo = document.createElement('div');
    verseInfo.className = 'market-meta-item';
    verseInfo.innerHTML = `
        <span class="meta-label">Verses</span>
        <span class="meta-value verse-highlight">${verses.length} available</span>
    `;
    
    // Add verse info to metadata if not already there
    const metadataContainer = document.querySelector('.market-metadata');
    const existingVerseInfo = metadataContainer.querySelector('.verse-info');
    if (existingVerseInfo) {
        existingVerseInfo.replaceWith(verseInfo);
    } else {
        metadataContainer.appendChild(verseInfo);
    }
    verseInfo.classList.add('verse-info');
    
    } catch (error) {
        console.error('Error updating market display:', error);
        showError('Failed to display market: ' + error.message);
    }
}

// Get verses for market
function getVersesForMarket(market) {
    // First check if market already has verses from backend
    if (market.verses && market.verses.length > 0) {
        console.log('Using API verses for market:', market.title, 'Verses:', market.verses.length);
        return market.verses;
    }
    
    // Also check platformState.selectedMarket in case verses were added later
    if (platformState.selectedMarket && 
        platformState.selectedMarket.id === market.id && 
        platformState.selectedMarket.verses && 
        platformState.selectedMarket.verses.length > 0) {
        console.log('Using verses from platformState for market:', market.title);
        return platformState.selectedMarket.verses;
    }
    
    // Otherwise generate based on market characteristics
    const verses = [];
    const marketWords = market.title?.toLowerCase().split(' ') || [];
    
    // Category-specific verses
    if (market.category === 'Crypto') {
        // Bitcoin specific
        if (market.title.includes('Bitcoin') || market.title.includes('BTC')) {
            verses.push(
                { id: 'btc-100k', name: 'BTC to $100k Club', multiplier: 2.5, level: 2, description: 'For believers in six-figure Bitcoin', category: 'Crypto', risk_tier: 'Medium', market_count: 1 },
                { id: 'btc-dominance', name: 'Bitcoin Dominance', multiplier: 1.8, level: 2, description: 'BTC market cap dominance play', category: 'Crypto', risk_tier: 'Medium', market_count: 1 },
                { id: 'hodl-gang', name: 'HODL Gang', multiplier: 1.4, level: 1, description: 'Diamond hands forever', category: 'Crypto', risk_tier: 'Low', market_count: 1 }
            );
        }
        
        // Ethereum specific
        if (market.title.includes('Ethereum') || market.title.includes('ETH')) {
            verses.push(
                { id: 'eth-flippening', name: 'The Flippening', multiplier: 3.0, level: 3, description: 'ETH overtaking BTC market cap' },
                { id: 'defi-summer', name: 'DeFi Revolution', multiplier: 2.2, level: 2, description: 'Decentralized finance takeover' },
                { id: 'eth-merge', name: 'Post-Merge Era', multiplier: 1.6, level: 1, description: 'Proof of Stake benefits' }
            );
        }
        
        // General crypto verses
        verses.push(
            { id: 'crypto-bull', name: 'Crypto Bull Market', multiplier: 1.8, level: 1, description: 'General crypto optimism' },
            { id: 'institutional', name: 'Institutional Wave', multiplier: 1.5, level: 2, description: 'Big money entering crypto' },
            { id: 'halving-cycle', name: 'Halving Cycle', multiplier: 2.0, level: 2, description: '4-year cycle believers' }
        );
        
    } else if (market.category === 'Politics') {
        // Trump specific
        if (market.title.includes('Trump')) {
            verses.push(
                { id: 'trump-factor', name: 'Trump Factor', multiplier: 2.0, level: 2, description: 'The Trump phenomenon' },
                { id: 'maga-movement', name: 'MAGA Movement', multiplier: 1.8, level: 2, description: 'Make America Great Again supporters' },
                { id: 'trump-comeback', name: 'Comeback Kid', multiplier: 2.5, level: 3, description: 'Historic political comeback' }
            );
        }
        
        // Election specific
        if (market.title.includes('2024') || market.title.includes('election')) {
            verses.push(
                { id: 'election-2024', name: '2024 Election Wave', multiplier: 1.3, level: 1, description: 'General election momentum' },
                { id: 'swing-states', name: 'Swing State Focus', multiplier: 1.7, level: 2, description: 'Battleground state strategy' },
                { id: 'youth-vote', name: 'Youth Turnout', multiplier: 2.2, level: 2, description: 'Gen Z political awakening' }
            );
        }
        
        // General politics verses
        verses.push(
            { id: 'political-upset', name: 'Political Upset', multiplier: 2.8, level: 3, description: 'Against the odds victory' },
            { id: 'establishment', name: 'Establishment Pick', multiplier: 1.2, level: 1, description: 'Safe institutional choice' }
        );
        
    } else if (market.category === 'Sports') {
        // Team specific verses
        if (market.title.includes('Super Bowl')) {
            verses.push(
                { id: 'dynasty-team', name: 'Dynasty Builder', multiplier: 1.6, level: 2, description: 'Championship legacy teams' },
                { id: 'underdog-story', name: 'Underdog Story', multiplier: 3.2, level: 3, description: 'David vs Goliath' },
                { id: 'home-advantage', name: 'Home Field', multiplier: 1.4, level: 1, description: 'Home team advantage' }
            );
        }
        
        verses.push(
            { id: 'clutch-performance', name: 'Clutch Factor', multiplier: 1.8, level: 2, description: 'Big game performers' },
            { id: 'injury-hedge', name: 'Injury Protection', multiplier: 1.3, level: 1, description: 'Hedge against key injuries' }
        );
        
    } else if (market.category === 'Technology') {
        // AI specific
        if (market.title.includes('AI') || market.title.includes('AGI') || market.title.includes('GPT')) {
            verses.push(
                { id: 'ai-singularity', name: 'AI Singularity', multiplier: 4.0, level: 3, description: 'Exponential AI progress' },
                { id: 'openai-dominance', name: 'OpenAI Dominance', multiplier: 1.7, level: 2, description: 'First mover advantage' },
                { id: 'ai-safety', name: 'AI Safety Concerns', multiplier: 2.2, level: 2, description: 'Regulatory delays possible' }
            );
        }
        
        verses.push(
            { id: 'tech-adoption', name: 'Tech Adoption Curve', multiplier: 1.5, level: 1, description: 'Mass market adoption' },
            { id: 'disruption-play', name: 'Disruption Play', multiplier: 2.5, level: 2, description: 'Industry disruption bet' }
        );
    }
    
    // Time-based verses
    const endDate = new Date(market.endDate);
    const daysUntilEnd = Math.floor((endDate - new Date()) / (1000 * 60 * 60 * 24));
    
    if (daysUntilEnd < 30) {
        verses.push(
            { id: 'short-term', name: 'Short Term Spike', multiplier: 2.0, level: 2, description: 'Quick resolution expected' }
        );
    } else if (daysUntilEnd > 180) {
        verses.push(
            { id: 'long-term', name: 'Long Term Hold', multiplier: 1.4, level: 1, description: 'Patient money verse' }
        );
    }
    
    // Probability-based verses
    if (market.outcomes && market.outcomes.length > 0) {
        const mainPrice = market.outcomes[0].price;
        if (mainPrice > 0.8 || mainPrice < 0.2) {
        verses.push(
            { id: 'high-confidence', name: 'High Confidence', multiplier: 1.2, level: 1, description: 'Strong market consensus' },
            { id: 'tail-risk', name: 'Tail Risk Hunter', multiplier: 3.5, level: 3, description: 'Betting against consensus' }
        );
    } else if (mainPrice > 0.4 && mainPrice < 0.6) {
        verses.push(
            { id: 'coin-flip', name: 'Coin Flip Special', multiplier: 1.8, level: 2, description: '50/50 market opportunity' },
            { id: 'volatility-play', name: 'Volatility Play', multiplier: 2.2, level: 2, description: 'High uncertainty = high reward' }
        );
    }
    } // Close the outcomes check
    
    // Volume-based verses
    if (market.volume24h > 5000000) {
        verses.push(
            { id: 'whale-following', name: 'Whale Tracker', multiplier: 1.6, level: 2, description: 'Follow the big money' }
        );
    } else if (market.volume24h < 1000000) {
        verses.push(
            { id: 'early-bird', name: 'Early Bird', multiplier: 2.4, level: 2, description: 'Get in before the crowd' }
        );
    }
    
    // Universal verses (always available)
    verses.push(
        { id: 'degen-mode', name: 'Full Degen', multiplier: 5.0, level: 3, description: 'Maximum risk, maximum reward' },
        { id: 'safe-play', name: 'Safe Harbor', multiplier: 1.1, level: 1, description: 'Conservative multiplier' },
        { id: 'momentum', name: 'Momentum Rider', multiplier: 1.7, level: 2, description: 'Ride the trend' }
    );
    
    // Sort by level and limit to reasonable number
    return verses
        .sort((a, b) => a.level - b.level)
        .slice(0, 12); // Max 12 verses per market
}

// Update available verses
function updateAvailableVerses(verses) {
    console.log('updateAvailableVerses called with:', verses);
    console.log('Type of verses:', typeof verses);
    console.log('Is array?:', Array.isArray(verses));
    console.log('Number of verses:', verses ? verses.length : 0);
    
    if (!verses) {
        console.error('Verses is null or undefined');
        return;
    }
    
    if (!Array.isArray(verses)) {
        console.error('Verses is not an array, it is:', typeof verses);
        console.log('Verses value:', verses);
        return;
    }
    
    if (verses.length === 0) {
        console.error('Verses array is empty');
        return;
    }
    
    // Always use flow view for now since verseGrid doesn't exist
    console.log('Updating verse flow with', verses.length, 'verses');
    
    try {
        const verseFlowContainer = document.getElementById('verseFlowContainer');
        
        if (!verseFlowContainer) {
            console.error('verseFlowContainer not found');
            return;
        }
        
        verseFlowContainer.style.display = 'block';
        updateVerseFlow(verses);
    } catch (error) {
        console.error('Error in updateAvailableVerses:', error);
    }
}

// Create verse card element
function createVerseCard(verse, hasParent = false) {
    const card = document.createElement('div');
    card.className = 'verse-card' + (hasParent ? ' has-parent' : '');
    card.id = `verse-card-${verse.id}`;
    card.onclick = () => toggleVerseSelection(verse.id);
    
    // Check if selected
    if (platformState.selectedVerses.includes(verse.id)) {
        card.classList.add('selected');
    }
    
    // Add description from verse object or use default
    const description = verse.description || 'Multiply your leverage with this stage';
    
    card.innerHTML = `
        <div class="verse-header">
            <div class="verse-title">${verse.name}</div>
            <div class="verse-multiplier">${verse.multiplier}x</div>
        </div>
        <div class="verse-description">${description}</div>
        <div class="verse-stats">
            <div class="verse-stat">
                <div class="verse-stat-label">Level</div>
                <div class="verse-stat-value">L${verse.level}</div>
            </div>
            <div class="verse-stat">
                <div class="verse-stat-label">Risk</div>
                <div class="verse-stat-value">${getVerseRisk(verse.multiplier)}</div>
            </div>
        </div>
    `;
    
    return card;
}

// Get risk level based on multiplier
function getVerseRisk(multiplier) {
    if (multiplier <= 1.5) return 'Low';
    if (multiplier <= 2.5) return 'Med';
    if (multiplier <= 3.5) return 'High';
    return 'Max';
}

// Update verse flow visualization
function updateVerseFlow(verses) {
    console.log('updateVerseFlow called with', verses ? verses.length : 0, 'verses');
    
    const verseLevels = document.getElementById('verseLevels');
    const svgConnections = document.getElementById('verseConnections');
    
    if (!verseLevels || !svgConnections) {
        console.error('Required elements not found:', {verseLevels: !!verseLevels, svgConnections: !!svgConnections});
        return;
    }
    
    // Clear existing content
    verseLevels.innerHTML = '';
    // Clear only path elements, preserve defs
    const paths = svgConnections.querySelectorAll('path');
    paths.forEach(path => path.remove());
    
    // Group verses by level
    const levelGroups = {};
    verses.forEach(verse => {
        const level = verse.level || 1;
        console.log('Processing verse:', verse.name, 'Level:', level);
        if (!levelGroups[level]) {
            levelGroups[level] = [];
        }
        levelGroups[level].push(verse);
    });
    console.log('Level groups:', Object.keys(levelGroups), levelGroups);
    
    // Create level containers
    const levels = Object.keys(levelGroups).sort((a, b) => a - b);
    const levelElements = {};
    
    levels.forEach((level, levelIndex) => {
        console.log(`Creating level ${level} container with ${levelGroups[level].length} verses`);
        const levelContainer = document.createElement('div');
        levelContainer.className = 'verse-level';
        levelContainer.id = `verse-level-${level}`;
        
        // Add level label
        const levelLabel = document.createElement('div');
        levelLabel.className = 'verse-level-label';
        levelLabel.textContent = `Level ${level}`;
        levelContainer.appendChild(levelLabel);
        
        // Add verses for this level
        levelGroups[level].forEach((verse, verseIndex) => {
            console.log(`Creating verse card for: ${verse.name}, multiplier: ${verse.multiplier}x`);
            const hasParent = levelIndex > 0; // All non-first level verses have parents
            const card = createVerseCard(verse, hasParent);
            card.setAttribute('data-level', level);
            card.setAttribute('data-index', verseIndex);
            levelContainer.appendChild(card);
        });
        
        verseLevels.appendChild(levelContainer);
        levelElements[level] = levelContainer;
        console.log(`Level ${level} container added with ${levelContainer.children.length - 1} verse cards`);
    });
    
    // Draw connections between levels
    setTimeout(() => {
        drawVerseConnections(levels, levelGroups, svgConnections);
    }, 100); // Small delay to ensure DOM is ready
}

// Draw SVG connections between verse levels
function drawVerseConnections(levels, levelGroups, svgElement) {
    const container = document.getElementById('verseFlowContainer');
    const containerRect = container.getBoundingClientRect();
    
    // Set SVG dimensions
    svgElement.setAttribute('width', containerRect.width);
    svgElement.setAttribute('height', containerRect.height);
    
    // Draw connections between adjacent levels
    for (let i = 0; i < levels.length - 1; i++) {
        const currentLevel = levels[i];
        const nextLevel = levels[i + 1];
        
        const currentVerses = levelGroups[currentLevel];
        const nextVerses = levelGroups[nextLevel];
        
        currentVerses.forEach((sourceVerse, sourceIndex) => {
            nextVerses.forEach((targetVerse, targetIndex) => {
                // Create connection based on some logic (e.g., category match, multiplier range)
                if (shouldConnect(sourceVerse, targetVerse)) {
                    drawConnection(
                        svgElement,
                        containerRect,
                        currentLevel,
                        sourceIndex,
                        nextLevel,
                        targetIndex,
                        isConnectionActive(sourceVerse.id, targetVerse.id)
                    );
                }
            });
        });
    }
}

// Determine if two verses should be connected
function shouldConnect(sourceVerse, targetVerse) {
    // Connect if multipliers are progressive
    if (targetVerse.multiplier > sourceVerse.multiplier) {
        return true;
    }
    
    // Connect if names share keywords
    const sourceWords = sourceVerse.name.toLowerCase().split(' ');
    const targetWords = targetVerse.name.toLowerCase().split(' ');
    const hasCommonWord = sourceWords.some(word => 
        targetWords.includes(word) && word.length > 3
    );
    
    return hasCommonWord;
}

// Check if connection should be highlighted
function isConnectionActive(sourceId, targetId) {
    return platformState.selectedVerses.includes(sourceId) && 
           platformState.selectedVerses.includes(targetId);
}

// Draw a single connection between verses
function drawConnection(svg, containerRect, sourceLevel, sourceIndex, targetLevel, targetIndex, isActive) {
    const sourceCard = document.querySelector(`#verse-level-${sourceLevel} .verse-card:nth-child(${sourceIndex + 2})`);
    const targetCard = document.querySelector(`#verse-level-${targetLevel} .verse-card:nth-child(${targetIndex + 2})`);
    
    if (!sourceCard || !targetCard) return;
    
    const sourceRect = sourceCard.getBoundingClientRect();
    const targetRect = targetCard.getBoundingClientRect();
    
    // Calculate positions relative to container
    const sourceX = sourceRect.right - containerRect.left;
    const sourceY = sourceRect.top + sourceRect.height / 2 - containerRect.top;
    const targetX = targetRect.left - containerRect.left;
    const targetY = targetRect.top + targetRect.height / 2 - containerRect.top;
    
    // Create curved path
    const midX = (sourceX + targetX) / 2;
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', `M ${sourceX} ${sourceY} Q ${midX} ${sourceY} ${midX} ${targetY} T ${targetX} ${targetY}`);
    path.setAttribute('class', 'verse-connection' + (isActive ? ' active' : ''));
    path.setAttribute('marker-end', isActive ? 'url(#arrowhead-active)' : 'url(#arrowhead)');
    
    svg.appendChild(path);
}

// Select outcome
function selectOutcome(outcomeName) {
    platformState.selectedOutcome = outcomeName;
    
    // Update UI
    document.querySelectorAll('.outcome-card').forEach(card => {
        card.classList.remove('selected');
        if (card.querySelector('.outcome-name').textContent === outcomeName) {
            card.classList.add('selected');
        }
    });
    
    updatePosition();
}

// Toggle verse selection
function toggleVerseSelection(verseId) {
    const index = platformState.selectedVerses.indexOf(verseId);
    
    if (index === -1) {
        platformState.selectedVerses.push(verseId);
    } else {
        platformState.selectedVerses.splice(index, 1);
    }
    
    // Update UI
    const card = document.getElementById(`verse-card-${verseId}`);
    if (card) {
        card.classList.toggle('selected');
    }
    
    // Redraw connections if in flow view
    const isFlowView = document.getElementById('verseFlowContainer').style.display !== 'none';
    if (isFlowView && platformState.selectedMarket) {
        const verses = getVersesForMarket(platformState.selectedMarket);
        updateVerseFlow(verses);
    }
    
    updateLeverageDisplay();
    updatePosition();
}

// Toggle quantum mode
function toggleQuantumMode() {
    console.log('Toggling quantum mode, current state:', platformState.quantumMode);
    platformState.quantumMode = !platformState.quantumMode;
    console.log('New quantum mode state:', platformState.quantumMode);
    
    const toggle = document.getElementById('quantumToggle');
    if (toggle) {
        toggle.classList.toggle('active');
        console.log('Toggle element classes:', toggle.className);
    } else {
        console.error('quantumToggle element not found!');
    }
    
    const quantumStates = document.getElementById('quantumStates');
    if (quantumStates) {
        quantumStates.style.display = platformState.quantumMode ? 'block' : 'none';
        console.log('Quantum states display:', quantumStates.style.display);
    } else {
        console.error('quantumStates element not found!');
    }
    
    if (platformState.quantumMode && platformState.selectedMarket) {
        console.log('Updating quantum states for market:', platformState.selectedMarket);
        updateQuantumStates();
    }
    
    updatePosition();
}

// Update quantum states
function updateQuantumStates() {
    const grid = document.getElementById('quantumStateGrid');
    grid.innerHTML = '';
    
    const amount = parseFloat(document.getElementById('investmentAmount').value) || 0;
    const outcomes = platformState.selectedMarket.outcomes;
    
    outcomes.forEach(outcome => {
        const probability = outcome.price;
        const amplitude = Math.sqrt(probability);
        const allocation = amount * amplitude;
        
        const item = document.createElement('div');
        item.className = 'quantum-state-item';
        item.innerHTML = `
            <div class="quantum-outcome">${outcome.name}</div>
            <div class="quantum-allocation">
                <div class="quantum-percentage">${(amplitude * 100).toFixed(1)}%</div>
                <div class="quantum-amount">${allocation.toFixed(3)} SOL</div>
            </div>
        `;
        
        grid.appendChild(item);
    });
}

// Update leverage display
function updateLeverageDisplay() {
    const totalLeverage = calculateTotalLeverage();
    document.getElementById('totalLeverage').textContent = totalLeverage.toFixed(1) + 'x';
    
    // Update breakdown
    const breakdown = document.getElementById('leverageBreakdown');
    breakdown.innerHTML = `
        <div class="leverage-item">
            <span>Base Leverage</span>
            <span>${platformState.baseLeverage.toFixed(1)}x</span>
        </div>
    `;
    
    // Add verse multipliers
    const verses = getVersesForMarket(platformState.selectedMarket || { category: '' });
    platformState.selectedVerses.forEach(verseId => {
        const verse = verses.find(v => v.id === verseId);
        if (verse) {
            breakdown.innerHTML += `
                <div class="leverage-item">
                    <span>${verse.name}</span>
                    <span>${verse.multiplier.toFixed(1)}x</span>
                </div>
            `;
        }
    });
}

// Calculate total leverage
function calculateTotalLeverage() {
    let total = platformState.baseLeverage;
    
    const verses = getVersesForMarket(platformState.selectedMarket || { category: '' });
    platformState.selectedVerses.forEach(verseId => {
        const verse = verses.find(v => v.id === verseId);
        if (verse) {
            total *= verse.multiplier;
        }
    });
    
    return Math.min(total, 100); // Cap at 100x
}

// Update base leverage with max limits based on outcome count
function updateBaseLeverage(value) {
    const maxAllowed = getMaxLeverageForMarket();
    const actualValue = Math.min(parseInt(value), maxAllowed);
    
    platformState.baseLeverage = actualValue;
    document.getElementById('baseLeverageValue').textContent = actualValue;
    
    // Update max leverage display
    const leverageLimit = document.getElementById('leverageLimit');
    if (leverageLimit) {
        leverageLimit.textContent = `(max: ${maxAllowed}x)`;
    }
    
    // Update slider max
    const slider = document.getElementById('baseLeverageSlider');
    if (slider) {
        slider.max = maxAllowed;
        slider.value = actualValue;
    }
    
    updateLeverageBreakdown();
    updatePosition();
}

// Get max leverage based on market outcomes
function getMaxLeverageForMarket() {
    if (!platformState.selectedMarket) return 100;
    
    const outcomeCount = platformState.selectedMarket.outcomes?.length || 2;
    
    // Leverage tiers from backend analysis
    if (outcomeCount === 1) return 100;
    if (outcomeCount === 2) return 70;
    if (outcomeCount <= 4) return 25;
    if (outcomeCount <= 7) return 15;
    if (outcomeCount <= 15) return 12;
    if (outcomeCount <= 63) return 10;
    return 5; // 64+ outcomes
}

// Update position
function updatePosition() {
    const amount = parseFloat(document.getElementById('investmentAmount')?.value) || 
                  parseFloat(document.getElementById('limitAmount')?.value) || 0;
    const leverage = calculateTotalLeverage();
    
    document.getElementById('summaryInvestment').textContent = amount.toFixed(2) + ' SOL';
    document.getElementById('summaryLeverage').textContent = leverage.toFixed(1) + 'x';
    document.getElementById('summaryExposure').textContent = (amount * leverage).toFixed(2) + ' SOL';
    
    if (platformState.quantumMode) {
        updateQuantumStates();
    }
    
    // Show positions section if wallet is connected
    if (platformState.wallet) {
        document.getElementById('positionsSection').style.display = 'block';
        updatePositionsDisplay();
    }
}

// Update risk control
function updateRiskControl(type, value) {
    if (type === 'stopLoss') {
        document.getElementById('stopLossValue').textContent = '-' + value + '%';
    } else {
        document.getElementById('takeProfitValue').textContent = '+' + value + '%';
    }
}

// Execute order
async function executeOrder() {
    try {
        const amount = parseFloat(document.getElementById('investmentAmount').value);
        if (!amount || amount <= 0) {
            showError('Please enter a valid amount');
            return;
        }
        
        if (!platformState.selectedMarket) {
            showError('Please select a market first');
            return;
        }
        
        if (!platformState.quantumMode && !platformState.selectedOutcome) {
            showError('Please select an outcome');
            return;
        }
        
        // Show modal
        document.getElementById('transactionModal').classList.add('active');
        
        // Create position object
        const position = {
            id: Date.now().toString(),
            marketId: platformState.selectedMarket.id,
            marketTitle: platformState.selectedMarket.title,
            outcome: platformState.selectedOutcome,
            amount: amount,
            leverage: calculateTotalLeverage(),
            verses: [...platformState.selectedVerses],
            entryPrice: platformState.selectedMarket.outcomes[0].price,
            timestamp: new Date(),
            status: 'active'
        };
        
        // Add to active positions
        platformState.activePositions.push(position);
        updateActivePositionsDisplay();
        
        // Create money flow animation
        createMoneyFlowAnimation(amount);
        
        // Execute trade via backend
        let tradeResult;
        
        if (window.backendAPI && platformState.wallet) {
            // Real backend execution
            const tradeData = {
                wallet: platformState.wallet.publicKey.toString(),
                market: platformState.selectedMarket,
                amount: amount,
                outcome: platformState.selectedOutcome,
                leverage: calculateTotalLeverage(),
                selectedVerses: platformState.selectedVerses,
                quantumMode: platformState.quantumMode,
                orderType: platformState.orderType,
                mirrorToPolymarket: platformState.selectedMarket.polymarket !== undefined
            };
            
            try {
                tradeResult = await window.backendAPI.placeTrade(tradeData);
                console.log('Trade executed:', tradeResult);
            } catch (error) {
                console.error('Trade execution failed:', error);
                showError('Trade execution failed: ' + error.message);
                document.getElementById('transactionModal').classList.remove('active');
                return;
            }
        } else {
            // Simulate order execution for demo
            await new Promise(resolve => setTimeout(resolve, 2000));
        }
        
        // Create new position record
        const newPosition = {
            id: tradeResult?.orderId || 'pos_' + Date.now(),
            market: platformState.selectedMarket.title,
            signature: tradeResult?.signature,
            outcome: platformState.selectedOutcome || 'Quantum',
            amount: amount,
            leverage: calculateTotalLeverage(),
            currentPrice: platformState.selectedMarket.outcomes[0].price,
            entryPrice: platformState.selectedMarket.outcomes[0].price,
            pnl: 0,
            pnlPercent: 0,
            status: 'open',
            source: platformState.selectedMarket.source
        };
        
        platformState.positions.set(newPosition.id, newPosition);
        
        // Update positions display
        updatePositionsDisplay();
        
        // Update status
        document.getElementById('transactionStatus').innerHTML = `
            <div class="success-message">
                Order executed successfully!<br>
                Position ID: ${position.id}
            </div>
        `;
        
    } catch (error) {
        console.error('Order execution failed:', error);
        document.getElementById('transactionStatus').innerHTML = `
            <div class="error-message">
                Order failed: ${error.message}
            </div>
        `;
    }
}

// Create money flow animation
function createMoneyFlowAnimation(amount) {
    const container = document.getElementById('moneyFlowContainer');
    container.innerHTML = '';
    
    const particle = document.createElement('div');
    particle.className = 'money-particle';
    particle.textContent = amount.toFixed(2);
    
    particle.style.left = '50%';
    particle.style.top = '80%';
    particle.style.transform = 'translate(-50%, -50%)';
    
    container.appendChild(particle);
    
    // Animate to platform
    setTimeout(() => {
        particle.style.transition = 'all 1s ease-out';
        particle.style.top = '20%';
        particle.style.opacity = '0';
    }, 100);
}

// Initialize verse tree
function initializeVerseTree() {
    // Verse tree is already in HTML
    const verseItems = document.querySelectorAll('.verse-item');
    verseItems.forEach(item => {
        item.onclick = function(e) {
            const verseId = e.currentTarget.parentElement.querySelector('.verse-children').id.replace('-children', '');
            toggleVerse(verseId);
        };
    });
}

// Toggle verse in tree
function toggleVerse(verseId) {
    const children = document.getElementById(verseId + '-children');
    const icon = event.currentTarget.querySelector('.verse-expand-icon');
    
    if (children.classList.contains('expanded')) {
        children.classList.remove('expanded');
        icon.style.transform = 'rotate(0deg)';
    } else {
        children.classList.add('expanded');
        icon.style.transform = 'rotate(90deg)';
    }
}

// Close position
async function closePosition(positionId) {
    try {
        const position = platformState.positions.get(positionId);
        if (!position) return;
        
        if (confirm(`Close position in ${position.market}?`)) {
            // Show loading state
            const positionCard = document.querySelector(`[data-position-id="${positionId}"]`);
            if (positionCard) {
                positionCard.style.opacity = '0.5';
            }
            
            if (window.backendAPI && platformState.wallet) {
                // Close position via backend
                const result = await window.backendAPI.closePosition(position.market_id || position.id, 0);
                
                if (result.success) {
                    // Remove position from state
                    platformState.positions.delete(positionId);
                    
                    // Update display
                    updatePositionsDisplay();
                    
                    // Refresh portfolio
                    await refreshPortfolio();
                    
                    // Show success
                    showSuccess(`Position closed successfully. Signature: ${result.signature}`);
                } else {
                    throw new Error(result.error || 'Failed to close position');
                }
            } else {
                // Demo mode - just remove position
                platformState.positions.delete(positionId);
                updatePositionsDisplay();
                showSuccess('Position closed successfully (demo)');
            }
        }
    } catch (error) {
        showError('Failed to close position: ' + error.message);
        
        // Restore opacity
        const positionCard = document.querySelector(`[data-position-id="${positionId}"]`);
        if (positionCard) {
            positionCard.style.opacity = '1';
        }
    }
}

// View position details
function viewPosition(positionId) {
    const position = platformState.positions.get(positionId);
    if (!position) return;
    
    // Could open a modal with detailed position info
    console.log('View position:', position);
}

// Close modal
function closeModal() {
    document.getElementById('transactionModal').classList.remove('active');
}

// Select order type
function selectOrderType(type) {
    platformState.orderType = type;
    
    // Update tabs
    document.querySelectorAll('.order-tab').forEach(tab => {
        tab.classList.remove('active');
        if (tab.textContent.toLowerCase() === type) {
            tab.classList.add('active');
        }
    });
    
    // Show/hide order content
    document.getElementById('marketOrderContent').style.display = type === 'market' ? 'block' : 'none';
    document.getElementById('limitOrderContent').style.display = type === 'limit' ? 'block' : 'none';
    
    // Update position calculation
    updatePosition();
}

// Show error message
function showError(message) {
    console.error(message);
    showNotification(message, 'error');
}

// Show success message
function showSuccess(message) {
    console.log(message);
    showNotification(message, 'success');
}

// Show notification
function showNotification(message, type = 'info') {
    const notification = document.createElement('div');
    notification.className = `notification notification-${type}`;
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        background: ${type === 'error' ? '#FF3B30' : type === 'success' ? '#4CD964' : '#007AFF'};
        color: white;
        padding: 16px 24px;
        border-radius: 12px;
        box-shadow: 0 4px 20px rgba(0,0,0,0.2);
        z-index: 10000;
        animation: slideIn 0.3s ease-out;
        font-size: 14px;
        font-weight: 500;
    `;
    notification.textContent = message;
    
    document.body.appendChild(notification);
    
    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease-out';
        setTimeout(() => notification.remove(), 300);
    }, 4000);
}

// Show trade notification
function showTradeNotification(data) {
    const message = `Trade executed: ${data.amount} SOL on ${data.market}`;
    showNotification(message, 'success');
}

// Update market data from WebSocket
function updateMarketData(data) {
    const market = platformState.markets.get(data.marketId);
    if (market) {
        Object.assign(market, data);
        // Update UI if this is the selected market
        if (platformState.selectedMarket?.id === data.marketId) {
            updateMarketDisplay(market);
        }
    }
}

// Update price display from WebSocket
function updatePriceDisplay(data) {
    // Update price displays in real-time
    const priceElements = document.querySelectorAll(`[data-market-id="${data.marketId}"] .price`);
    priceElements.forEach(el => {
        el.textContent = `${(data.price * 100).toFixed(1)}%`;
    });
}

// Load user positions from backend
async function loadUserPositions() {
    if (!platformState.wallet || !window.backendAPI) return;
    
    try {
        const positions = await window.backendAPI.getPositions(platformState.wallet.publicKey.toString());
        positions.forEach(pos => {
            platformState.positions.set(pos.id, pos);
        });
        updatePositionsDisplay();
    } catch (error) {
        console.error('Failed to load positions:', error);
    }
}

// Format number
function formatNumber(num) {
    if (num >= 1000000) {
        return (num / 1000000).toFixed(1) + 'M';
    } else if (num >= 1000) {
        return (num / 1000).toFixed(1) + 'K';
    }
    return num.toFixed(0);
}

// Export functions for global access
window.connectWallet = connectWallet;
window.searchMarkets = searchMarkets;
window.selectSearchResult = selectSearchResult;
window.toggleVerse = toggleVerse;
window.toggleQuantumMode = toggleQuantumMode;
window.selectOrderType = selectOrderType;
window.updatePosition = updatePosition;
window.updateBaseLeverage = updateBaseLeverage;
window.updateRiskControl = updateRiskControl;
window.executeOrder = executeOrder;
window.closeModal = closeModal;
window.selectOutcome = selectOutcome;
window.toggleVerseSelection = toggleVerseSelection;
window.closePosition = closePosition;
window.viewPosition = viewPosition;

// Update active positions display
function updateActivePositionsDisplay() {
    const container = document.getElementById('activePositionsContainer');
    const bar = document.getElementById('activePositionsBar');
    
    if (!container || !bar) return;
    
    // Show bar if there are positions
    if (platformState.activePositions.length > 0) {
        bar.style.display = 'block';
    } else {
        bar.style.display = 'none';
        return;
    }
    
    // Clear existing
    container.innerHTML = '';
    
    // Calculate totals
    let totalValue = 0;
    let totalPnL = 0;
    
    // Create position blocks
    platformState.activePositions.forEach(position => {
        const currentPrice = getMarketPrice(position.marketId, position.outcome);
        const pnl = ((currentPrice - position.entryPrice) / position.entryPrice) * 100;
        const currentValue = position.amount * (1 + pnl / 100);
        
        totalValue += currentValue;
        totalPnL += (currentValue - position.amount);
        
        const block = document.createElement('div');
        block.className = 'position-block';
        block.onclick = () => selectPositionMarket(position.marketId);
        
        block.innerHTML = `
            <div class="position-outcome">${position.outcome}</div>
            <div class="position-market">${position.marketTitle}</div>
            <div class="position-details">
                <div class="position-detail">
                    <div class="detail-label">Investment</div>
                    <div class="detail-value">${position.amount.toFixed(2)} SOL</div>
                </div>
                <div class="position-detail">
                    <div class="detail-label">Current</div>
                    <div class="detail-value">${currentValue.toFixed(2)} SOL</div>
                </div>
                <div class="position-detail">
                    <div class="detail-label">P&L</div>
                    <div class="detail-value ${pnl >= 0 ? 'pnl-positive' : 'pnl-negative'}">
                        ${pnl >= 0 ? '+' : ''}${pnl.toFixed(1)}%
                    </div>
                </div>
                <div class="position-detail">
                    <div class="detail-label">Entry</div>
                    <div class="detail-value">${(position.entryPrice * 100).toFixed(1)}%</div>
                </div>
            </div>
            <div class="position-leverage">${position.leverage.toFixed(1)}x Leverage</div>
        `;
        
        container.appendChild(block);
    });
    
    // Update summary
    document.getElementById('totalPositionValue').textContent = `$${(totalValue * 50).toFixed(0)}`; // Assuming 1 SOL = $50
    const totalPnLElement = document.getElementById('totalPnL');
    const pnlPercent = (totalPnL / (totalValue - totalPnL)) * 100;
    totalPnLElement.textContent = `${pnlPercent >= 0 ? '+' : ''}${pnlPercent.toFixed(1)}%`;
    totalPnLElement.className = pnlPercent >= 0 ? 'pnl-positive' : 'pnl-negative';
}

// Get current market price for position
function getMarketPrice(marketId, outcome) {
    const market = platformState.markets.get(marketId);
    if (!market) return 0.5;
    
    const outcomeIndex = market.outcomes.findIndex(o => 
        (typeof o === 'string' ? o : o.name) === outcome
    );
    
    if (outcomeIndex !== -1 && market.outcomes[outcomeIndex].price) {
        return market.outcomes[outcomeIndex].price;
    }
    
    // Return a simulated price change
    return 0.5 + (Math.random() - 0.5) * 0.1;
}

// Select market from position
function selectPositionMarket(marketId) {
    const market = platformState.markets.get(marketId);
    if (market) {
        platformState.selectedMarket = market;
        updateMarketDisplay(market);
        
        // Hide search results
        document.getElementById('marketSearchResults').style.display = 'none';
        document.getElementById('marketSearchInput').value = '';
    }
}

// Update wallet display
function updateWalletDisplay() {
    if (!platformState.wallet) return;
    
    const walletAddress = platformState.wallet.publicKey ? 
        platformState.wallet.publicKey.toString() : 
        platformState.wallet.toString();
    
    // Update UI
    document.getElementById('walletIndicator').classList.add('connected');
    document.getElementById('walletStatusText').textContent = 'Connected';
    document.getElementById('walletAddress').textContent = 
        walletAddress.slice(0, 6) + '...' + walletAddress.slice(-4);
    document.getElementById('walletAddress').style.display = 'block';
    
    // Show balance (demo balance for demo mode)
    const isDemoMode = walletAddress.includes('Demo');
    if (isDemoMode) {
        document.getElementById('balanceAmount').textContent = '100.0000 SOL';
        document.getElementById('balanceDisplay').style.display = 'block';
    }
    
    // Hide connect button
    document.getElementById('connectWalletBtn').style.display = 'none';
}

// Add demo positions for testing
function addDemoPositions() {
    // Create some demo markets if not loaded
    const demoMarkets = [
        {
            id: 'demo-btc',
            title: 'Will Bitcoin reach $100k by 2025?',
            outcomes: [
                { name: 'Yes', price: 0.65 },
                { name: 'No', price: 0.35 }
            ]
        },
        {
            id: 'demo-trump',
            title: 'Trump wins 2024 GOP nomination',
            outcomes: [
                { name: 'Yes', price: 0.85 },
                { name: 'No', price: 0.15 }
            ]
        },
        {
            id: 'demo-ai',
            title: 'OpenAI releases GPT-5 in 2024',
            outcomes: [
                { name: 'Yes', price: 0.42 },
                { name: 'No', price: 0.58 }
            ]
        }
    ];
    
    // Add markets to state
    demoMarkets.forEach(market => {
        platformState.markets.set(market.id, market);
    });
    
    // Create demo positions
    platformState.activePositions = [
        {
            id: '1',
            marketId: 'demo-btc',
            marketTitle: 'Will Bitcoin reach $100k by 2025?',
            outcome: 'Yes',
            amount: 10,
            leverage: 5.5,
            verses: ['btc-100k', 'bull-run'],
            entryPrice: 0.60,
            timestamp: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000),
            status: 'active'
        },
        {
            id: '2',
            marketId: 'demo-trump',
            marketTitle: 'Trump wins 2024 GOP nomination',
            outcome: 'Yes',
            amount: 5,
            leverage: 2.4,
            verses: ['trump-factor'],
            entryPrice: 0.82,
            timestamp: new Date(Date.now() - 5 * 24 * 60 * 60 * 1000),
            status: 'active'
        },
        {
            id: '3',
            marketId: 'demo-ai',
            marketTitle: 'OpenAI releases GPT-5 in 2024',
            outcome: 'No',
            amount: 15,
            leverage: 8.0,
            verses: ['ai-singularity', 'tech-disruption'],
            entryPrice: 0.55,
            timestamp: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000),
            status: 'active'
        }
    ];
    
    // Display positions
    updateActivePositionsDisplay();
}

// Refresh portfolio overview
async function refreshPortfolio() {
    if (!platformState.wallet) return;
    
    try {
        // Show portfolio overview
        document.getElementById('portfolioOverview').style.display = 'block';
        
        if (window.backendAPI) {
            const portfolio = await window.backendAPI.getPortfolio(platformState.wallet.publicKey.toString());
            
            // Update portfolio stats
            document.getElementById('totalValue').textContent = '$' + (portfolio.totalValue || 0).toFixed(2);
            document.getElementById('activePositionsCount').textContent = portfolio.positionCount || 0;
            document.getElementById('totalPnlOverview').textContent = '$' + (portfolio.totalPnl || 0).toFixed(2);
            document.getElementById('winRate').textContent = (portfolio.winRate || 0).toFixed(1) + '%';
            
            // Color PnL
            const pnlElement = document.getElementById('totalPnlOverview');
            if (portfolio.totalPnl > 0) {
                pnlElement.style.color = '#4CD964';
            } else if (portfolio.totalPnl < 0) {
                pnlElement.style.color = '#FF3B30';
            }
        }
    } catch (error) {
        console.error('Failed to refresh portfolio:', error);
    }
}

window.refreshPortfolio = refreshPortfolio;

// Export verse functions
window.getVersesForMarket = getVersesForMarket;
window.updateAvailableVerses = updateAvailableVerses;
window.updateVerseFlow = updateVerseFlow;
window.createVerseCard = createVerseCard;
window.toggleVerseSelection = toggleVerseSelection;