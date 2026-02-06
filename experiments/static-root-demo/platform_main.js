// Platform Main JavaScript - Handles all initialization and UI updates

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
            // Simulate API search with mock data
            await new Promise(resolve => setTimeout(resolve, 500));
            
            const markets = getMockSearchResults(query);
            
            if (markets.length === 0) {
                resultsContainer.innerHTML = '<div class="search-empty">No markets found. Try different keywords.</div>';
            } else {
                displaySearchResults(markets);
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
    
    resultsContainer.innerHTML = markets.map(market => `
        <div class="search-result-item" onclick="selectSearchResult('${market.id}')">
            <div class="result-market-title">${market.title}</div>
            <div class="result-market-info">
                <span class="result-source ${market.source === 'kalshi' ? 'kalshi' : ''}">${market.source}</span>
                <span>${market.category}</span>
                <span>${(market.yesPrice * 100).toFixed(0)}% Yes</span>
            </div>
            <div class="result-stats">
                <span class="result-stat">Volume: <strong>$${formatNumber(market.volume24h)}</strong></span>
                <span class="result-stat">Liquidity: <strong>$${formatNumber(market.liquidity)}</strong></span>
            </div>
        </div>
    `).join('');
}

// Select search result
async function selectSearchResult(marketId) {
    const markets = getMockSearchResults('');
    const selectedMarket = markets.find(m => m.id === marketId);
    
    if (!selectedMarket) return;
    
    // Hide search results
    document.getElementById('marketSearchResults').style.display = 'none';
    document.getElementById('marketSearchInput').value = '';
    
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
        endDate: new Date(Date.now() + 90 * 24 * 60 * 60 * 1000), // 90 days from now
        category: selectedMarket.category
    };
    
    // Store and select market
    platformState.markets.set(marketData.id, marketData);
    platformState.selectedMarket = marketData;
    
    // Update display
    updateMarketDisplay(marketData);
    
    // Get available verses
    const verses = getVersesForMarket(marketData);
    updateAvailableVerses(verses);
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
function getVersesForMarket(market) {
    const verses = [];
    const marketWords = market.title.toLowerCase().split(' ');
    
    // Category-specific verses
    if (market.category === 'Crypto') {
        // Bitcoin specific
        if (market.title.includes('Bitcoin') || market.title.includes('BTC')) {
            verses.push(
                { id: 'btc-100k', name: 'BTC to $100k Club', multiplier: 2.5, level: 2, description: 'For believers in six-figure Bitcoin' },
                { id: 'btc-dominance', name: 'Bitcoin Dominance', multiplier: 1.8, level: 2, description: 'BTC market cap dominance play' },
                { id: 'hodl-gang', name: 'HODL Gang', multiplier: 1.4, level: 1, description: 'Diamond hands forever' }
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
    const verseGrid = document.getElementById('verseGrid');
    verseGrid.innerHTML = '';
    
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
            <div class="verse-description">Multiply your leverage with this verse</div>
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