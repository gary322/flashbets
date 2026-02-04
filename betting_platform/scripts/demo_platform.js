#!/usr/bin/env node

/**
 * Quantum Betting Platform - Full Demo Script
 * This script demonstrates all platform features end-to-end
 */

const API_URL = 'http://localhost:8081';
const WS_URL = 'ws://localhost:8081/ws';

// Demo markets data
const DEMO_MARKETS = [
  {
    id: 1,
    title: "Will Bitcoin reach $100k by 2025?",
    description: "Market for Bitcoin price prediction",
    category: "Crypto",
    endTime: "2025-01-01T00:00:00Z",
    ammType: "LMSR",
    liquidity: 500000,
    volume: 1234567,
    yesPrice: 0.34,
    noPrice: 0.66,
    verse: 1
  },
  {
    id: 2,
    title: "US Presidential Election 2024",
    description: "Who will win the 2024 US Presidential Election?",
    category: "Politics",
    endTime: "2024-11-05T00:00:00Z",
    ammType: "PM-AMM",
    liquidity: 2000000,
    volume: 5432100,
    yesPrice: 0.52,
    noPrice: 0.48,
    verse: 1
  },
  {
    id: 3,
    title: "Will AGI be achieved by 2030?",
    description: "Artificial General Intelligence achievement prediction",
    category: "Technology",
    endTime: "2030-12-31T00:00:00Z",
    ammType: "Hybrid",
    liquidity: 750000,
    volume: 890123,
    yesPrice: 0.23,
    noPrice: 0.77,
    verse: 2
  },
  {
    id: 4,
    title: "SpaceX Mars Landing by 2030?",
    description: "Will SpaceX successfully land humans on Mars by 2030?",
    category: "Science",
    endTime: "2030-12-31T00:00:00Z",
    ammType: "L2-AMM",
    liquidity: 1000000,
    volume: 2345678,
    yesPrice: 0.41,
    noPrice: 0.59,
    verse: 3
  },
  {
    id: 5,
    title: "Ethereum > $10k in 2025?",
    description: "Will Ethereum price exceed $10,000 in 2025?",
    category: "Crypto",
    endTime: "2025-12-31T00:00:00Z",
    ammType: "LMSR",
    liquidity: 600000,
    volume: 1567890,
    yesPrice: 0.28,
    noPrice: 0.72,
    verse: 1
  },
  {
    id: 6,
    title: "Champions League Winner 2024",
    description: "Real Madrid to win Champions League 2024?",
    category: "Sports",
    endTime: "2024-06-01T00:00:00Z",
    ammType: "PM-AMM",
    liquidity: 300000,
    volume: 789012,
    yesPrice: 0.25,
    noPrice: 0.75,
    verse: 4
  }
];

// Demo user positions
const DEMO_POSITIONS = [
  {
    marketId: 1,
    outcome: "yes",
    amount: 1000,
    leverage: 5,
    entryPrice: 0.32,
    currentPrice: 0.34,
    pnl: 62.5,
    pnlPercent: 6.25
  },
  {
    marketId: 2,
    outcome: "no",
    amount: 500,
    leverage: 2,
    entryPrice: 0.45,
    currentPrice: 0.48,
    pnl: -33.33,
    pnlPercent: -6.66
  },
  {
    marketId: 3,
    outcome: "yes",
    amount: 2000,
    leverage: 10,
    entryPrice: 0.20,
    currentPrice: 0.23,
    pnl: 300,
    pnlPercent: 15
  }
];

// WebSocket connection
let ws;

function connectWebSocket() {
  ws = new WebSocket(WS_URL);
  
  ws.onopen = () => {
    console.log('✅ WebSocket connected');
  };
  
  ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log('📊 WebSocket update:', data);
  };
  
  ws.onerror = (error) => {
    console.error('❌ WebSocket error:', error);
  };
}

// Simulate market updates
function simulateMarketUpdates() {
  setInterval(() => {
    DEMO_MARKETS.forEach(market => {
      // Random price movement
      const change = (Math.random() - 0.5) * 0.02;
      market.yesPrice = Math.max(0.01, Math.min(0.99, market.yesPrice + change));
      market.noPrice = 1 - market.yesPrice;
      market.volume += Math.floor(Math.random() * 10000);
      
      // Broadcast update
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({
          type: 'market_update',
          data: market
        }));
      }
    });
  }, 5000);
}

// Simulate trades
function simulateTrades() {
  setInterval(() => {
    const market = DEMO_MARKETS[Math.floor(Math.random() * DEMO_MARKETS.length)];
    const trade = {
      type: 'trade',
      marketId: market.id,
      outcome: Math.random() > 0.5 ? 'yes' : 'no',
      amount: Math.floor(Math.random() * 1000) + 100,
      price: market.yesPrice,
      trader: `0x${Math.random().toString(16).substr(2, 8)}...`,
      timestamp: new Date().toISOString()
    };
    
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'trade_executed',
        data: trade
      }));
    }
    
    console.log(`💰 Trade executed: ${trade.amount} on ${trade.outcome} @ ${trade.price.toFixed(3)}`);
  }, 3000);
}

// Demo verse hierarchy
const VERSE_HIERARCHY = {
  1: { name: "Prime Verse", level: 1, multiplier: 1, parent: null },
  2: { name: "Alpha Verse", level: 2, multiplier: 1.5, parent: 1 },
  3: { name: "Beta Verse", level: 3, multiplier: 2, parent: 1 },
  4: { name: "Gamma Verse", level: 4, multiplier: 2.5, parent: 2 },
  5: { name: "Delta Verse", level: 5, multiplier: 3, parent: 2 },
  // ... up to 32 levels
};

// Quantum betting demo
function demonstrateQuantumBetting() {
  console.log('\n🌌 Quantum Betting Demo');
  console.log('========================');
  
  const quantumPosition = {
    marketId: 1,
    superposition: {
      yes: 0.7071,  // |ψ⟩ = 0.7071|yes⟩ + 0.7071|no⟩
      no: 0.7071
    },
    entangled: [2, 3],
    collapsePrice: 0.5,
    potentialPayout: {
      yes: 1500,
      no: 1500
    }
  };
  
  console.log('Quantum Position Created:');
  console.log(`|ψ⟩ = ${quantumPosition.superposition.yes}|yes⟩ + ${quantumPosition.superposition.no}|no⟩`);
  console.log(`Entangled with markets: ${quantumPosition.entangled.join(', ')}`);
}

// Main demo function
async function runDemo() {
  console.log('🚀 Starting Quantum Betting Platform Demo');
  console.log('========================================\n');
  
  // Connect WebSocket
  connectWebSocket();
  
  // Wait for connection
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  // Start simulations
  console.log('📊 Starting market simulations...');
  simulateMarketUpdates();
  simulateTrades();
  
  // Demonstrate features
  setTimeout(() => demonstrateQuantumBetting(), 2000);
  
  // Show platform stats
  setInterval(() => {
    const totalVolume = DEMO_MARKETS.reduce((sum, m) => sum + m.volume, 0);
    const totalLiquidity = DEMO_MARKETS.reduce((sum, m) => sum + m.liquidity, 0);
    
    console.log('\n📈 Platform Statistics:');
    console.log(`Total Volume: $${totalVolume.toLocaleString()}`);
    console.log(`Total Liquidity: $${totalLiquidity.toLocaleString()}`);
    console.log(`Active Markets: ${DEMO_MARKETS.length}`);
    console.log(`Active Verses: ${Object.keys(VERSE_HIERARCHY).length}`);
  }, 10000);
  
  console.log('\n✅ Demo running! Check the UI at http://localhost:8080');
  console.log('Press Ctrl+C to stop\n');
}

// Run the demo
runDemo().catch(console.error);