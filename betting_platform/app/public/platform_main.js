// Platform Main JavaScript - Handles all initialization and UI updates

// API Configuration
const API_BASE_URL = '/api';

// Global state
const platformState = {
    wallet: null,
    connection: null,
    markets: new Map(),
    positions: new Map(),
    selectedMarket: null,
    selectedOutcome: null,
    selectedVerses: [],
    baseLeverage: 5,
    quantumMode: false,
    orderType: 'market'
};

// API Helper functions
async function apiCall(endpoint, options = {}) {
    try {
        const response = await fetch(`${API_BASE_URL}${endpoint}`, {
            headers: {
                'Content-Type': 'application/json',
                ...options.headers
            },
            ...options
        });
        
        if (!response.ok) {
            throw new Error(`API Error: ${response.status} ${response.statusText}`);
        }
        
        return await response.json();
    } catch (error) {
        console.error(`API call failed for ${endpoint}:`, error);
        throw error;
    }
}

// Initialize on DOM load
document.addEventListener('DOMContentLoaded', async function() {
    console.log('Platform initializing...');
    
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
        
        console.log('Platform initialized successfully');
    } catch (error) {
        console.error('Initialization failed:', error);
        showError('Failed to initialize platform: ' + error.message);
    }
});

// Initialize Solana connection
async function initializeSolana() {
    platformState.connection = new solanaWeb3.Connection(
        'https://api.devnet.solana.com',
        'confirmed'
    );
    console.log('Solana connection established');
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

        if (!window.solana) {
            throw new Error('Solana wallet not found. Please install Phantom wallet.');
        }

        const response = await window.solana.connect();
        platformState.wallet = response.publicKey;
        
        console.log('Wallet connected:', platformState.wallet.toString());
        
        // Update UI
        document.getElementById('walletIndicator').classList.add('connected');
        document.getElementById('walletStatusText').textContent = 'Connected';
        document.getElementById('walletAddress').textContent = 
            platformState.wallet.toString().slice(0, 6) + '...' + 
            platformState.wallet.toString().slice(-4);
        document.getElementById('walletAddress').style.display = 'block';
        
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
            // Search real markets via API
            console.log('Searching for:', query);
            const response = await apiCall(`/markets?search=${encodeURIComponent(query)}&limit=10`);
            console.log('API response:', response);
            
            // Handle API response format - API returns {markets: [...], count: N, total: N}
            let markets = [];
            if (response.markets && Array.isArray(response.markets)) {
                markets = response.markets;
                console.log(`API returned ${markets.length} real markets for query: "${query}"`);
            } else if (Array.isArray(response)) {
                markets = response;
                console.log(`API returned ${markets.length} markets as array`);
            } else if (response.data && Array.isArray(response.data)) {
                markets = response.data;
                console.log(`API returned ${markets.length} markets in data field`);
            }
            
            if (!markets || markets.length === 0) {
                resultsContainer.innerHTML = '<div class="search-empty">No markets found for your search. The API returned no matching markets.</div>';
                return; // Don't fall back to mock data - show real empty state
            } else {
                console.log('Displaying real API markets:', markets);
                displaySearchResults(markets);
                return; // Success - don't fall back to mock data
            }
        } catch (error) {
            console.error('API search failed:', error);
            resultsContainer.innerHTML = `<div class="search-empty">Search failed: ${error.message}<br>Please check if the backend API is running on localhost:8081</div>`;
        }
    }, 300);
}

// Get mock search results based on query
function getMockSearchResults(query) {
    const allMarkets = [
        // Crypto markets (using real API IDs)
        {
            id: 5,
            title: 'Bitcoin Above $100k by 2025',
            source: 'seeded_data',
            category: 'Crypto',
            volume24h: 12000000,
            liquidity: 5000000,
            yesPrice: 0.70,
            outcomes: ['Yes', 'No']
        },
        {
            id: 7,
            title: 'S&P 500 Above 6000 in 2024',
            source: 'seeded_data',
            category: 'Finance',
            volume24h: 5500000,
            liquidity: 4000000,
            yesPrice: 0.30,
            outcomes: ['Yes', 'No']
        },
        
        // Politics markets (using real API IDs)
        {
            id: 1,
            title: '2024 US Presidential Election Winner',
            source: 'seeded_data',
            category: 'Politics',
            volume24h: 8500000,
            liquidity: 5000000,
            yesPrice: 0.50,
            outcomes: ['Biden', 'Trump', 'Other']
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
        
        // Sports markets (using real API IDs)
        {
            id: 3,
            title: 'Super Bowl 2025 Winner',
            source: 'seeded_data',
            category: 'Sports',
            volume24h: 7500000,
            liquidity: 5000000,
            yesPrice: 0.30,
            outcomes: ['Chiefs', '49ers', 'Bills', 'Eagles', 'Other']
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
        // Handle real API data format
        const title = market.title || 'Unknown Market';
        const source = 'seeded_data'; // API always returns seeded_data source
        const category = getMarketCategory(market);
        
        // Calculate yes price from outcomes - API format: [{name: "Yes", total_stake: N}, ...]
        let yesPrice = 0.5; // default
        if (market.outcomes && Array.isArray(market.outcomes) && market.outcomes.length > 0) {
            const totalStake = market.outcomes.reduce((sum, outcome) => sum + (outcome.total_stake || 0), 0);
            if (totalStake > 0) {
                const yesOutcome = market.outcomes.find(o => o.name === 'Yes');
                if (yesOutcome) {
                    yesPrice = yesOutcome.total_stake / totalStake;
                }
            }
        }
        
        const volume = market.total_volume || 0;
        const liquidity = market.total_liquidity || 0;
        
        return `
            <div class="search-result-item" onclick="selectSearchResult('${market.id}')">
                <div class="result-market-title">${title}</div>
                <div class="result-market-info">
                    <span class="result-source">${source}</span>
                    <span>${category}</span>
                    <span>${(yesPrice * 100).toFixed(0)}% Yes</span>
                </div>
                <div class="result-stats">
                    <span class="result-stat">Volume: <strong>$${formatNumber(volume)}</strong></span>
                    <span class="result-stat">Liquidity: <strong>$${formatNumber(liquidity)}</strong></span>
                </div>
            </div>
        `;
    }).join('');
}

// Get market category from verse_id or title
function getMarketCategory(market) {
    // Map verse_id to category based on common patterns
    const verseIdCategoryMap = {
        2: 'Crypto', // Crypto markets (Bitcoin, Ethereum, etc)
        20: 'Crypto', // Alternative crypto verse ID
        21: 'Crypto', // Alternative crypto verse ID  
        1: 'Politics', // Election markets
        3: 'Sports', // Sports markets
        4: 'Entertainment', // Entertainment markets
        5: 'Business', // Business/Finance markets
        6: 'Space', // Space exploration markets
        9: 'Technology', // Technology markets
        10: 'Sports', // Alternative sports verse ID
        11: 'Environmental', // Environmental markets
        30: 'Finance' // Financial markets
    };
    
    if (market.verse_id && verseIdCategoryMap[market.verse_id]) {
        return verseIdCategoryMap[market.verse_id];
    }
    
    // Fallback to title-based categorization
    const title = market.title?.toLowerCase() || '';
    if (title.includes('bitcoin') || title.includes('btc') || title.includes('crypto') || title.includes('ethereum')) {
        return 'Crypto';
    } else if (title.includes('election') || title.includes('president') || title.includes('political')) {
        return 'Politics';
    } else if (title.includes('sports') || title.includes('super bowl') || title.includes('nfl') || title.includes('nba')) {
        return 'Sports';
    } else if (title.includes('s&p') || title.includes('stock') || title.includes('market') || title.includes('finance')) {
        return 'Finance';
    }
    
    return 'General';
}

// Select search result
async function selectSearchResult(marketId) {
    try {
        // Hide search results first
        document.getElementById('marketSearchResults').style.display = 'none';
        document.getElementById('marketSearchInput').value = '';
        
        // Show loading state
        document.getElementById('marketLoadingState').style.display = 'block';
        document.getElementById('marketContent').style.display = 'none';
        
        // Fetch market details from API
        console.log('Fetching market details for ID:', marketId);
        const marketData = await apiCall(`/markets/${marketId}`);
        
        // Ensure outcomes array exists and has proper format
        if (!marketData.outcomes || !Array.isArray(marketData.outcomes)) {
            marketData.outcomes = [
                { name: 'Yes', price: 0.5, volume: marketData.total_volume / 2 || 0, liquidity: marketData.total_liquidity / 2 || 0 },
                { name: 'No', price: 0.5, volume: marketData.total_volume / 2 || 0, liquidity: marketData.total_liquidity / 2 || 0 }
            ];
        } else {
            // Process existing outcomes to add missing fields
            const totalStake = marketData.outcomes.reduce((sum, outcome) => sum + (outcome.total_stake || 0), 0);
            marketData.outcomes = marketData.outcomes.map(outcome => ({
                name: outcome.name,
                price: totalStake > 0 ? (outcome.total_stake || 0) / totalStake : 0.5,
                volume: outcome.total_stake || 0,
                liquidity: (outcome.total_stake || 0) * 0.8 // Assume 80% of stake is liquid
            }));
        }
        
        // Ensure required fields
        marketData.title = marketData.title || marketData.description || 'Unknown Market';
        marketData.volume24h = marketData.total_volume || 0;
        marketData.liquidity = marketData.total_liquidity || 0;
        marketData.endDate = marketData.resolution_time ? new Date(marketData.resolution_time) : new Date(Date.now() + 90 * 24 * 60 * 60 * 1000);
        marketData.category = marketData.verse_id || 'General';
        marketData.source = marketData.source || 'seeded_data';
        
        // Store and select market
        platformState.markets.set(marketData.id, marketData);
        platformState.selectedMarket = marketData;
        
        // Hide loading and show content
        document.getElementById('marketLoadingState').style.display = 'none';
        document.getElementById('marketContent').style.display = 'block';
        
        // Update display
        updateMarketDisplay(marketData);
        
        // Get available verses
        console.log('Fetching verses for market:', marketData);
        const verses = await getVersesForMarket(marketData);
        console.log('Got verses:', verses);
        updateAvailableVerses(verses);
        
    } catch (error) {
        console.error('Failed to load market:', error);
        
        // Hide loading state
        document.getElementById('marketLoadingState').style.display = 'none';
        
        // Fallback to mock data
        const markets = getMockSearchResults('');
        const selectedMarket = markets.find(m => m.id === marketId);
        
        if (selectedMarket) {
            // Convert to full market data
            const marketData = {
                id: selectedMarket.id,
                source: selectedMarket.source,
                title: selectedMarket.title,
                description: `Market for: ${selectedMarket.title}`,
                outcomes: selectedMarket.outcomes.map((name, index) => ({
                    name,
                    price: index === 0 ? selectedMarket.yesPrice : (1 - selectedMarket.yesPrice) / (selectedMarket.outcomes.length - 1),
                    volume: selectedMarket.volume24h / selectedMarket.outcomes.length,
                    liquidity: selectedMarket.liquidity / selectedMarket.outcomes.length
                })),
                volume24h: selectedMarket.volume24h,
                liquidity: selectedMarket.liquidity,
                endDate: new Date(Date.now() + 90 * 24 * 60 * 60 * 1000),
                category: selectedMarket.category
            };
            
            // Store and select market
            platformState.markets.set(marketData.id, marketData);
            platformState.selectedMarket = marketData;
            
            // Show content
            document.getElementById('marketContent').style.display = 'block';
            
            // Update display
            updateMarketDisplay(marketData);
            
            // Get available verses
            const verses = getVersesForMarket(marketData);
            updateAvailableVerses(verses);
        }
    }
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
    document.getElementById('marketTitle').textContent = market.title;
    document.getElementById('marketVolume').textContent = '$' + formatNumber(market.volume24h);
    document.getElementById('marketLiquidity').textContent = '$' + formatNumber(market.liquidity);
    document.getElementById('marketEndDate').textContent = market.endDate.toLocaleDateString();
    
    // Update platform badge
    const badge = document.getElementById('platformBadge');
    badge.className = 'platform-badge ' + market.source;
    badge.innerHTML = `<span>${market.source === 'polymarket' ? 'Polymarket' : 'Kalshi'}</span>`;
    
    // Update outcomes
    const outcomeGrid = document.getElementById('outcomeGrid');
    outcomeGrid.innerHTML = '';
    
    market.outcomes.forEach((outcome, index) => {
        const card = document.createElement('div');
        card.className = 'outcome-card';
        card.onclick = () => selectOutcome(outcome.name);
        
        const priceChange = outcome.price > 0.5 ? '+' : '-';
        const changeClass = outcome.price > 0.5 ? '' : 'negative';
        
        card.innerHTML = `
            <div class="outcome-name">${outcome.name}</div>
            <div class="outcome-price">${(outcome.price * 100).toFixed(1)}%</div>
            <div class="outcome-change ${changeClass}">${priceChange}${Math.abs(outcome.price - 0.5) * 100}%</div>
            <div class="outcome-volume">Vol: $${formatNumber(outcome.volume)}</div>
        `;
        
        outcomeGrid.appendChild(card);
    });
}

// Get verses for market
async function getVersesForMarket(market) {
    try {
        // Fetch verses from API
        console.log('Fetching verses for market:', market.title, 'category:', getMarketCategory(market));
        const response = await apiCall(`/verses?limit=400`); // Get all verses
        const allVerses = Array.isArray(response) ? response : (response.data || response.verses || []);
        
        if (allVerses && allVerses.length > 0) {
            console.log(`Fetched ${allVerses.length} total verses from API`);
            
            // Filter verses by market category and content
            const marketCategory = getMarketCategory(market).toLowerCase();
            const marketTitle = market.title?.toLowerCase() || '';
            
            const relevantVerses = allVerses.filter(verse => {
                const verseCategory = verse.category?.toLowerCase() || '';
                const verseName = (verse.name || '').toLowerCase();
                const verseDesc = (verse.description || '').toLowerCase();
                
                // Exact category match (highest priority)
                if (verseCategory === marketCategory) return true;
                
                // Bitcoin/Crypto specific matching
                if (marketTitle.includes('bitcoin') || marketTitle.includes('btc')) {
                    return verseCategory === 'crypto' || 
                           verseName.includes('btc') || 
                           verseName.includes('bitcoin') ||
                           verseDesc.includes('bitcoin') ||
                           String(verse.id || '').includes('btc');
                }
                
                // Ethereum specific matching
                if (marketTitle.includes('ethereum') || marketTitle.includes('eth')) {
                    return verseCategory === 'crypto' || 
                           verseName.includes('eth') || 
                           verseName.includes('ethereum') ||
                           verseDesc.includes('ethereum');
                }
                
                // S&P 500 / Finance specific matching
                if (marketTitle.includes('s&p') || marketTitle.includes('500')) {
                    return verseCategory === 'economics' || 
                           verseCategory === 'finance' ||
                           verseName.includes('s&p') ||
                           verseName.includes('500') ||
                           verseName.includes('spy');
                }
                
                // Election/Politics matching
                if (marketTitle.includes('election') || marketTitle.includes('president')) {
                    return verseCategory === 'politics' ||
                           verseName.includes('election') ||
                           verseName.includes('president') ||
                           verseName.includes('trump') ||
                           verseName.includes('biden');
                }
                
                // Sports matching
                if (marketTitle.includes('super bowl') || marketTitle.includes('nfl')) {
                    return verseCategory === 'sports' ||
                           verseName.includes('super bowl') ||
                           verseName.includes('nfl') ||
                           verseName.includes('superbowl');
                }
                
                // General category matching
                if (marketCategory === 'crypto' && verseCategory === 'crypto') return true;
                if (marketCategory === 'politics' && verseCategory === 'politics') return true;
                if (marketCategory === 'finance' && (verseCategory === 'economics' || verseCategory === 'finance')) return true;
                if (marketCategory === 'sports' && verseCategory === 'sports') return true;
                
                return false;
            });
            
            console.log(`Filtered ${relevantVerses.length} relevant verses for ${marketCategory} market: "${market.title}"`);
            
            if (relevantVerses.length > 0) {
                // Sort by relevance (category match first, then level)
                const sortedVerses = relevantVerses.sort((a, b) => {
                    const aCategory = (a.category || '').toLowerCase();
                    const bCategory = (b.category || '').toLowerCase();
                    
                    // Exact category match comes first
                    if (aCategory === marketCategory && bCategory !== marketCategory) return -1;
                    if (bCategory === marketCategory && aCategory !== marketCategory) return 1;
                    
                    // Then sort by level (lower levels first for accessibility)
                    return (a.level || 1) - (b.level || 1);
                });
                
                // Return top 12 verses
                return sortedVerses.slice(0, 12).map(verse => ({
                    id: verse.id,
                    name: verse.name || 'Unnamed Verse',
                    multiplier: verse.multiplier || 1.5,
                    level: verse.level || 1,
                    description: verse.description || 'Verse multiplier',
                    category: verse.category || 'General'
                }));
            } else {
                console.log('No relevant verses found in API, will return empty array');
                return [];
            }
        } else {
            console.log('No verses received from API');
            return [];
        }
    } catch (error) {
        console.error('Failed to fetch verses from API:', error);
        return [];
    }
}

// Update available verses
function updateAvailableVerses(verses) {
    console.log('updateAvailableVerses called with', verses ? verses.length : 0, 'verses');
    const verseGrid = document.getElementById('verseGrid');
    if (!verseGrid) {
        console.error('verseGrid element not found!');
        return;
    }
    verseGrid.innerHTML = '';
    
    // Handle undefined or invalid verses
    if (!verses || !Array.isArray(verses)) {
        console.warn('No valid verses provided, showing placeholder');
        verseGrid.innerHTML = '<div style="color: #999; padding: 20px;">No verses available for this market</div>';
        return;
    }
    
    verses.forEach(verse => {
        const card = document.createElement('div');
        card.className = 'verse-card';
        card.id = `verse-card-${verse.id}`;
        card.onclick = () => toggleVerseSelection(verse.id);
        
        card.innerHTML = `
            <div class="verse-header">
                <div class="verse-title">${verse.name}</div>
                <div class="verse-multiplier">${verse.multiplier}x</div>
            </div>
            <div class="verse-description">${verse.description || 'Multiply your leverage with this verse'}</div>
            <div class="verse-stats">
                <div class="verse-stat">
                    <div class="verse-stat-label">Level</div>
                    <div class="verse-stat-value">L${verse.level}</div>
                </div>
                <div class="verse-stat">
                    <div class="verse-stat-label">Active</div>
                    <div class="verse-stat-value">Yes</div>
                </div>
            </div>
        `;
        
        verseGrid.appendChild(card);
    });
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
    
    updateLeverageDisplay();
    updatePosition();
}

// Toggle quantum mode
function toggleQuantumMode() {
    platformState.quantumMode = !platformState.quantumMode;
    
    const toggle = document.getElementById('quantumToggle');
    toggle.classList.toggle('active');
    
    const quantumStates = document.getElementById('quantumStates');
    quantumStates.style.display = platformState.quantumMode ? 'block' : 'none';
    
    if (platformState.quantumMode && platformState.selectedMarket) {
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

// Update position
function updatePosition() {
    const amount = parseFloat(document.getElementById('investmentAmount').value) || 0;
    const leverage = calculateTotalLeverage();
    
    document.getElementById('summaryInvestment').textContent = amount.toFixed(2) + ' SOL';
    document.getElementById('summaryLeverage').textContent = leverage.toFixed(1) + 'x';
    document.getElementById('summaryExposure').textContent = (amount * leverage).toFixed(2) + ' SOL';
    
    if (platformState.quantumMode) {
        updateQuantumStates();
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
        
        // Create money flow animation
        createMoneyFlowAnimation(amount);
        
        // Simulate order execution
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        // Create new position
        const position = {
            id: 'pos_' + Date.now(),
            market: platformState.selectedMarket.title,
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
        
        platformState.positions.set(position.id, position);
        
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

// Test function to add sample verses (for debugging)
function testAddVerses() {
    console.log('Testing verse display...');
    const testVerses = [
        { id: 'test-1', name: 'Test Verse 1', multiplier: 2.0, level: 1, description: 'Test verse for debugging' },
        { id: 'test-2', name: 'Test Verse 2', multiplier: 3.0, level: 2, description: 'Another test verse' },
        { id: 'test-3', name: 'Test Verse 3', multiplier: 1.5, level: 1, description: 'Third test verse' }
    ];
    updateAvailableVerses(testVerses);
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
            // Remove position
            platformState.positions.delete(positionId);
            updatePositionsDisplay();
            showSuccess('Position closed successfully');
        }
    } catch (error) {
        showError('Failed to close position: ' + error.message);
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
    document.querySelectorAll('.order-tab').forEach(tab => {
        tab.classList.remove('active');
    });
    event.target.classList.add('active');
}

// Show error message
function showError(message) {
    console.error(message);
    // TODO: Add toast notification
    alert('Error: ' + message);
}

// Show success message
function showSuccess(message) {
    console.log(message);
    // TODO: Add toast notification
}

// Format number
function formatNumber(num) {
    // Handle undefined, null, or non-numeric values
    if (num === undefined || num === null || isNaN(num)) {
        return '0';
    }
    
    // Convert to number if it's a string
    num = Number(num);
    
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
window.updateRiskControl = updateRiskControl;
window.executeOrder = executeOrder;
window.closeModal = closeModal;
window.selectOutcome = selectOutcome;
window.toggleVerseSelection = toggleVerseSelection;
window.closePosition = closePosition;
window.viewPosition = viewPosition;
window.testAddVerses = testAddVerses;
