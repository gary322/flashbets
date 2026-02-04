const express = require('express');
const EventSource = require('eventsource');
const axios = require('axios');

const app = express();
const PORT = process.env.SSE_PORT || 3001;

// Store active SSE connections
const clients = new Set();

// Polling intervals for different providers
const POLL_INTERVALS = {
    DraftKings: 2000,  // 2s for live
    FanDuel: 2000,
    BetMGM: 1500,      // Faster for higher rate limit
    Caesars: 3000,
    PointsBet: 2500
};

// Provider endpoints
const PROVIDER_ENDPOINTS = {
    DraftKings: 'https://api.draftkings.com/v1/odds/live',
    FanDuel: 'https://api.fanduel.com/fixtures/live',
    BetMGM: 'https://api.betmgm.com/events/live',
    Caesars: 'https://api.caesars.com/sports/live',
    PointsBet: 'https://api.pointsbet.com/api/v2/events/live'
};

// Active polling tasks
const pollingTasks = new Map();

/**
 * SSE endpoint for live flash market updates
 */
app.get('/sse/flash/:sport', (req, res) => {
    const { sport } = req.params;
    
    // Set SSE headers
    res.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
        'Access-Control-Allow-Origin': '*'
    });
    
    // Send initial connection message
    res.write(`data: ${JSON.stringify({ 
        type: 'connected', 
        sport,
        timestamp: Date.now() 
    })}\n\n`);
    
    // Add client to active connections
    const client = { res, sport, id: Date.now() };
    clients.add(client);
    
    // Start polling if not already active for this sport
    if (!pollingTasks.has(sport)) {
        startPolling(sport);
    }
    
    // Handle client disconnect
    req.on('close', () => {
        clients.delete(client);
        
        // Stop polling if no more clients for this sport
        const remainingClients = Array.from(clients).filter(c => c.sport === sport);
        if (remainingClients.length === 0) {
            stopPolling(sport);
        }
    });
});

/**
 * Start polling providers for a specific sport
 */
function startPolling(sport) {
    console.log(`Starting polling for ${sport}`);
    
    const tasks = [];
    
    // Poll each provider
    for (const [provider, endpoint] of Object.entries(PROVIDER_ENDPOINTS)) {
        const interval = POLL_INTERVALS[provider];
        
        const task = setInterval(async () => {
            try {
                const data = await fetchProviderData(provider, endpoint, sport);
                if (data) {
                    broadcastToClients(sport, {
                        type: 'odds_update',
                        provider,
                        sport,
                        data,
                        timestamp: Date.now()
                    });
                }
            } catch (error) {
                console.error(`Error polling ${provider}:`, error.message);
            }
        }, interval);
        
        tasks.push(task);
    }
    
    pollingTasks.set(sport, tasks);
}

/**
 * Stop polling for a specific sport
 */
function stopPolling(sport) {
    console.log(`Stopping polling for ${sport}`);
    
    const tasks = pollingTasks.get(sport);
    if (tasks) {
        tasks.forEach(task => clearInterval(task));
        pollingTasks.delete(sport);
    }
}

/**
 * Fetch data from provider API
 */
async function fetchProviderData(provider, endpoint, sport) {
    try {
        const response = await axios.get(endpoint, {
            params: { sport, limit: 10 },
            timeout: 3000,
            headers: {
                'User-Agent': 'FlashBets-SSE/1.0'
            }
        });
        
        // Extract flash-eligible markets (<5 min)
        const flashMarkets = response.data.filter(market => {
            const timeRemaining = market.time_remaining || 
                                 market.seconds_to_start || 
                                 300;
            return timeRemaining <= 300; // 5 minutes
        });
        
        return flashMarkets;
    } catch (error) {
        // Return null on error to continue polling other providers
        return null;
    }
}

/**
 * Broadcast update to all connected clients for a sport
 */
function broadcastToClients(sport, data) {
    const sportClients = Array.from(clients).filter(c => c.sport === sport);
    
    sportClients.forEach(client => {
        try {
            client.res.write(`data: ${JSON.stringify(data)}\n\n`);
        } catch (error) {
            // Client disconnected, remove from set
            clients.delete(client);
        }
    });
}

/**
 * Aggregate endpoint for best odds across providers
 */
app.get('/api/aggregate/:gameId', async (req, res) => {
    const { gameId } = req.params;
    
    try {
        // Fetch from all providers in parallel
        const promises = Object.entries(PROVIDER_ENDPOINTS).map(async ([provider, endpoint]) => {
            try {
                const response = await axios.get(`${endpoint}/${gameId}`, {
                    timeout: 2000
                });
                return { provider, data: response.data };
            } catch {
                return null;
            }
        });
        
        const results = (await Promise.allSettled(promises))
            .filter(r => r.status === 'fulfilled' && r.value)
            .map(r => r.value);
        
        if (results.length < 3) {
            return res.status(503).json({ 
                error: 'Insufficient provider quorum',
                available: results.length,
                required: 3
            });
        }
        
        // Calculate weighted average odds
        const aggregated = aggregateOdds(results);
        
        res.json(aggregated);
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

/**
 * Aggregate odds from multiple providers
 */
function aggregateOdds(results) {
    const weights = {
        DraftKings: 0.3,
        FanDuel: 0.3,
        BetMGM: 0.2,
        Caesars: 0.1,
        PointsBet: 0.1
    };
    
    let weightedSum = 0;
    let totalWeight = 0;
    
    results.forEach(({ provider, data }) => {
        const weight = weights[provider] || 0.1;
        const probability = data.probability || data.implied_probability || 0.5;
        
        weightedSum += probability * weight;
        totalWeight += weight;
    });
    
    return {
        probability: weightedSum / totalWeight,
        providers: results.length,
        timestamp: Date.now(),
        consensus: results.length >= 3
    };
}

// Health check endpoint
app.get('/health', (req, res) => {
    res.json({
        status: 'healthy',
        clients: clients.size,
        activeSports: Array.from(pollingTasks.keys()),
        uptime: process.uptime()
    });
});

// Start SSE proxy server
app.listen(PORT, () => {
    console.log(`SSE Proxy running on port ${PORT}`);
    console.log(`SSE endpoint: http://localhost:${PORT}/sse/flash/:sport`);
    console.log(`Aggregate endpoint: http://localhost:${PORT}/api/aggregate/:gameId`);
});

module.exports = { app };