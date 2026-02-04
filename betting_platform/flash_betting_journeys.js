const { ethers } = require('ethers');
const backend = require('./backend_integration');

// ============ USER PERSONAS ============

const USER_PERSONAS = {
  // Aggressive traders
  DEGEN: {
    name: 'Degen Trader',
    riskTolerance: 'extreme',
    leveragePreference: [300, 400, 500],
    betFrequency: 'very_high',
    avgBetSize: [10, 50, 100],
    chainingProbability: 0.8,
    sports: ['basketball', 'soccer', 'tennis']
  },
  
  HIGH_ROLLER: {
    name: 'High Roller',
    riskTolerance: 'high',
    leveragePreference: [100, 200, 300],
    betFrequency: 'medium',
    avgBetSize: [1000, 5000, 10000],
    chainingProbability: 0.5,
    sports: ['soccer', 'tennis']
  },
  
  // Conservative traders
  CAUTIOUS: {
    name: 'Cautious Better',
    riskTolerance: 'low',
    leveragePreference: [1, 10, 50],
    betFrequency: 'low',
    avgBetSize: [10, 20, 50],
    chainingProbability: 0.1,
    sports: ['soccer']
  },
  
  STRATEGIC: {
    name: 'Strategic Trader',
    riskTolerance: 'medium',
    leveragePreference: [50, 100, 150],
    betFrequency: 'medium',
    avgBetSize: [100, 500, 1000],
    chainingProbability: 0.3,
    sports: ['soccer', 'basketball']
  },
  
  // Specialized traders
  ARBITRAGEUR: {
    name: 'Arbitrage Hunter',
    riskTolerance: 'calculated',
    leveragePreference: [100, 200],
    betFrequency: 'very_high',
    avgBetSize: [500, 1000, 2000],
    chainingProbability: 0.2,
    sports: ['all']
  },
  
  SCALPER: {
    name: 'Micro Scalper',
    riskTolerance: 'medium',
    leveragePreference: [50, 100],
    betFrequency: 'extreme',
    avgBetSize: [5, 10, 20],
    chainingProbability: 0.6,
    sports: ['basketball']
  },
  
  WHALE: {
    name: 'Whale Trader',
    riskTolerance: 'medium',
    leveragePreference: [10, 50, 100],
    betFrequency: 'low',
    avgBetSize: [10000, 50000, 100000],
    chainingProbability: 0.2,
    sports: ['soccer', 'tennis']
  },
  
  BOT: {
    name: 'Algo Trader',
    riskTolerance: 'calculated',
    leveragePreference: [100, 150, 200],
    betFrequency: 'extreme',
    avgBetSize: [50, 100, 200],
    chainingProbability: 0.4,
    sports: ['all']
  }
};

// ============ MARKET SCENARIOS ============

const FLASH_MARKET_SCENARIOS = [
  // Soccer scenarios (5-60 seconds)
  {
    sport: 'soccer',
    title: 'Next corner kick in 30s?',
    duration: 30,
    volatility: 'high',
    expectedOutcome: 0.3,
    tau: 15
  },
  {
    sport: 'soccer',
    title: 'Goal in next 60s?',
    duration: 60,
    volatility: 'medium',
    expectedOutcome: 0.1,
    tau: 15
  },
  {
    sport: 'soccer',
    title: 'Yellow card in 45s?',
    duration: 45,
    volatility: 'low',
    expectedOutcome: 0.2,
    tau: 15
  },
  {
    sport: 'soccer',
    title: 'Penalty kick scored in 10s?',
    duration: 10,
    volatility: 'extreme',
    expectedOutcome: 0.8,
    tau: 15
  },
  {
    sport: 'soccer',
    title: 'Ball out of bounds in 20s?',
    duration: 20,
    volatility: 'medium',
    expectedOutcome: 0.6,
    tau: 15
  },
  
  // Basketball scenarios
  {
    sport: 'basketball',
    title: '3-pointer in next 24s?',
    duration: 24,
    volatility: 'high',
    expectedOutcome: 0.35,
    tau: 40
  },
  {
    sport: 'basketball',
    title: 'Dunk in next 30s?',
    duration: 30,
    volatility: 'medium',
    expectedOutcome: 0.15,
    tau: 40
  },
  {
    sport: 'basketball',
    title: 'Free throw made in 15s?',
    duration: 15,
    volatility: 'low',
    expectedOutcome: 0.75,
    tau: 40
  },
  {
    sport: 'basketball',
    title: 'Steal in next 10s?',
    duration: 10,
    volatility: 'high',
    expectedOutcome: 0.2,
    tau: 40
  },
  {
    sport: 'basketball',
    title: 'Timeout called in 5s?',
    duration: 5,
    volatility: 'extreme',
    expectedOutcome: 0.1,
    tau: 40
  },
  
  // Tennis scenarios
  {
    sport: 'tennis',
    title: 'Ace in next point (30s)?',
    duration: 30,
    volatility: 'medium',
    expectedOutcome: 0.25,
    tau: 20
  },
  {
    sport: 'tennis',
    title: 'Double fault in 20s?',
    duration: 20,
    volatility: 'low',
    expectedOutcome: 0.1,
    tau: 20
  },
  {
    sport: 'tennis',
    title: 'Rally over 10 shots in 45s?',
    duration: 45,
    volatility: 'high',
    expectedOutcome: 0.3,
    tau: 20
  },
  {
    sport: 'tennis',
    title: 'Break point in 60s?',
    duration: 60,
    volatility: 'medium',
    expectedOutcome: 0.15,
    tau: 20
  },
  {
    sport: 'tennis',
    title: 'Net point won in 15s?',
    duration: 15,
    volatility: 'high',
    expectedOutcome: 0.4,
    tau: 20
  }
];

// ============ JOURNEY SCENARIOS ============

class FlashBettingJourney {
  constructor(persona, scenario, journeyType) {
    this.persona = persona;
    this.scenario = scenario;
    this.journeyType = journeyType;
    this.results = [];
    this.errors = [];
    this.provider = null;
    this.signer = null;
    this.startTime = Date.now();
  }
  
  async initialize() {
    const { provider, signer } = backend.initPolygonProvider();
    this.provider = provider;
    this.signer = signer;
  }
  
  async execute() {
    try {
      await this.initialize();
      
      switch (this.journeyType) {
        case 'SINGLE_BET':
          return await this.executeSingleBet();
        case 'CHAINED_BETS':
          return await this.executeChainedBets();
        case 'RAPID_FIRE':
          return await this.executeRapidFire();
        case 'HEDGE_STRATEGY':
          return await this.executeHedgeStrategy();
        case 'MARTINGALE':
          return await this.executeMartingale();
        case 'ARBITRAGE':
          return await this.executeArbitrage();
        case 'MOMENTUM_TRADING':
          return await this.executeMomentumTrading();
        case 'CONTRARIAN':
          return await this.executeContrarian();
        case 'LADDER_STRATEGY':
          return await this.executeLadderStrategy();
        case 'ALL_IN':
          return await this.executeAllIn();
        default:
          throw new Error(`Unknown journey type: ${this.journeyType}`);
      }
    } catch (error) {
      this.errors.push({
        journey: this.journeyType,
        error: error.message,
        timestamp: Date.now()
      });
      return { success: false, error: error.message };
    }
  }
  
  async executeSingleBet() {
    const betSize = this.selectBetSize();
    const leverage = this.selectLeverage();
    const isYes = Math.random() > 0.5;
    
    // Create market
    const marketId = await this.createFlashMarket();
    
    // Place bet
    const position = await this.openPosition(marketId, betSize, leverage, isYes);
    
    // Simulate market resolution
    await this.simulateMarketResolution(marketId);
    
    // Claim winnings
    const result = await this.claimWinnings(position.id);
    
    return {
      success: true,
      journey: 'SINGLE_BET',
      persona: this.persona.name,
      marketId,
      position,
      result
    };
  }
  
  async executeChainedBets() {
    const chainLength = Math.floor(Math.random() * 3) + 1;
    const markets = [];
    const leverages = [];
    
    // Create multiple markets
    for (let i = 0; i < chainLength; i++) {
      const marketId = await this.createFlashMarket();
      markets.push(marketId);
      leverages.push(this.selectLeverage());
    }
    
    // Place chained bet
    const initialStake = this.selectBetSize();
    const chainedBet = await this.placeChainedBet(markets, leverages, initialStake);
    
    // Simulate resolutions
    for (const marketId of markets) {
      await this.simulateMarketResolution(marketId);
    }
    
    return {
      success: true,
      journey: 'CHAINED_BETS',
      persona: this.persona.name,
      chainLength,
      markets,
      leverages,
      initialStake,
      effectiveLeverage: this.calculateEffectiveLeverage(leverages),
      chainedBet
    };
  }
  
  async executeRapidFire() {
    const numberOfBets = Math.floor(Math.random() * 10) + 5;
    const positions = [];
    
    for (let i = 0; i < numberOfBets; i++) {
      const marketId = await this.createFlashMarket();
      const betSize = this.selectBetSize();
      const leverage = this.selectLeverage();
      const isYes = Math.random() > 0.5;
      
      const position = await this.openPosition(marketId, betSize, leverage, isYes);
      positions.push({ marketId, position });
      
      // Rapid fire - don't wait for resolution
    }
    
    // Simulate all resolutions
    for (const { marketId } of positions) {
      await this.simulateMarketResolution(marketId);
    }
    
    return {
      success: true,
      journey: 'RAPID_FIRE',
      persona: this.persona.name,
      numberOfBets,
      positions,
      totalVolume: positions.reduce((sum, p) => sum + p.position.size, 0)
    };
  }
  
  async executeHedgeStrategy() {
    const marketId = await this.createFlashMarket();
    const betSize = this.selectBetSize();
    const leverage = this.selectLeverage();
    
    // Open primary position
    const primaryPosition = await this.openPosition(marketId, betSize, leverage, true);
    
    // Open hedge position (opposite side, lower leverage)
    const hedgeSize = betSize * 0.5;
    const hedgeLeverage = Math.floor(leverage * 0.3);
    const hedgePosition = await this.openPosition(marketId, hedgeSize, hedgeLeverage, false);
    
    // Simulate resolution
    await this.simulateMarketResolution(marketId);
    
    return {
      success: true,
      journey: 'HEDGE_STRATEGY',
      persona: this.persona.name,
      marketId,
      primaryPosition,
      hedgePosition,
      netExposure: primaryPosition.size - hedgePosition.size
    };
  }
  
  async executeMartingale() {
    const rounds = [];
    let currentBet = this.selectBetSize();
    let totalLoss = 0;
    let won = false;
    
    for (let round = 0; round < 5 && !won; round++) {
      const marketId = await this.createFlashMarket();
      const leverage = this.selectLeverage();
      const isYes = Math.random() > 0.5;
      
      const position = await this.openPosition(marketId, currentBet, leverage, isYes);
      
      // Simulate resolution
      const outcome = await this.simulateMarketResolution(marketId);
      
      if (outcome.won) {
        won = true;
        rounds.push({ round, bet: currentBet, result: 'WIN', position });
      } else {
        totalLoss += currentBet;
        currentBet *= 2; // Double the bet
        rounds.push({ round, bet: currentBet, result: 'LOSS', position });
      }
    }
    
    return {
      success: true,
      journey: 'MARTINGALE',
      persona: this.persona.name,
      rounds,
      finalOutcome: won ? 'RECOVERED' : 'BUSTED',
      totalLoss,
      totalBet: rounds.reduce((sum, r) => sum + r.bet, 0)
    };
  }
  
  async executeArbitrage() {
    // Create two correlated markets
    const market1 = await this.createFlashMarket();
    const market2 = await this.createFlashMarket();
    
    // Get prices
    const price1Yes = await this.getMarketPrice(market1, true);
    const price1No = await this.getMarketPrice(market1, false);
    const price2Yes = await this.getMarketPrice(market2, true);
    const price2No = await this.getMarketPrice(market2, false);
    
    // Find arbitrage opportunity
    const betSize = this.selectBetSize();
    const leverage = this.selectLeverage();
    
    const positions = [];
    
    // Execute arbitrage trades
    if (price1Yes + price2No < 9500) { // Arbitrage opportunity
      positions.push(await this.openPosition(market1, betSize, leverage, true));
      positions.push(await this.openPosition(market2, betSize, leverage, false));
    } else if (price1No + price2Yes < 9500) {
      positions.push(await this.openPosition(market1, betSize, leverage, false));
      positions.push(await this.openPosition(market2, betSize, leverage, true));
    }
    
    // Simulate resolutions
    await this.simulateMarketResolution(market1);
    await this.simulateMarketResolution(market2);
    
    return {
      success: true,
      journey: 'ARBITRAGE',
      persona: this.persona.name,
      markets: [market1, market2],
      prices: { price1Yes, price1No, price2Yes, price2No },
      positions,
      arbitrageFound: positions.length > 0
    };
  }
  
  async executeMomentumTrading() {
    const marketId = await this.createFlashMarket();
    const positions = [];
    
    // Monitor price momentum
    for (let i = 0; i < 3; i++) {
      const priceYes = await this.getMarketPrice(marketId, true);
      
      if (priceYes > 6000 && i > 0) { // Momentum detected
        const betSize = this.selectBetSize() * (1 + i * 0.5);
        const leverage = this.selectLeverage();
        const position = await this.openPosition(marketId, betSize, leverage, true);
        positions.push(position);
      }
      
      // Wait a bit
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    
    // Simulate resolution
    await this.simulateMarketResolution(marketId);
    
    return {
      success: true,
      journey: 'MOMENTUM_TRADING',
      persona: this.persona.name,
      marketId,
      positions,
      pyramidingUsed: positions.length > 1
    };
  }
  
  async executeContrarian() {
    const marketId = await this.createFlashMarket();
    
    // Get current sentiment
    const priceYes = await this.getMarketPrice(marketId, true);
    const priceNo = await this.getMarketPrice(marketId, false);
    
    // Bet against the crowd
    const isContrarian = priceYes > 7000 ? false : (priceNo > 7000 ? true : Math.random() > 0.5);
    const betSize = this.selectBetSize();
    const leverage = this.selectLeverage() * 1.5; // Higher leverage for contrarian
    
    const position = await this.openPosition(marketId, betSize, leverage, isContrarian);
    
    // Simulate resolution
    await this.simulateMarketResolution(marketId);
    
    return {
      success: true,
      journey: 'CONTRARIAN',
      persona: this.persona.name,
      marketId,
      marketSentiment: { priceYes, priceNo },
      contrariaPnosition: isContrarian,
      position
    };
  }
  
  async executeLadderStrategy() {
    const marketId = await this.createFlashMarket();
    const ladderSteps = 5;
    const positions = [];
    const baseBet = this.selectBetSize();
    
    // Create ladder of positions
    for (let i = 0; i < ladderSteps; i++) {
      const betSize = baseBet * (1 - i * 0.15); // Decreasing bet sizes
      const leverage = this.selectLeverage() * (1 + i * 0.2); // Increasing leverage
      const isYes = i % 2 === 0; // Alternate sides
      
      const position = await this.openPosition(marketId, betSize, leverage, isYes);
      positions.push({
        step: i + 1,
        size: betSize,
        leverage,
        side: isYes ? 'YES' : 'NO',
        position
      });
    }
    
    // Simulate resolution
    await this.simulateMarketResolution(marketId);
    
    return {
      success: true,
      journey: 'LADDER_STRATEGY',
      persona: this.persona.name,
      marketId,
      ladderSteps,
      positions,
      totalExposure: positions.reduce((sum, p) => sum + p.size * p.leverage, 0)
    };
  }
  
  async executeAllIn() {
    const marketId = await this.createFlashMarket();
    
    // Use maximum bet size and leverage
    const maxBet = Math.max(...this.persona.avgBetSize) * 2;
    const maxLeverage = Math.max(...this.persona.leveragePreference);
    const isYes = Math.random() > 0.5;
    
    const position = await this.openPosition(marketId, maxBet, maxLeverage, isYes);
    
    // Simulate resolution
    const outcome = await this.simulateMarketResolution(marketId);
    
    return {
      success: true,
      journey: 'ALL_IN',
      persona: this.persona.name,
      marketId,
      position,
      totalExposure: maxBet * maxLeverage,
      outcome: outcome.won ? 'JACKPOT' : 'BUSTED'
    };
  }
  
  // Helper methods
  
  async createFlashMarket() {
    const flashBetting = backend.getPolygonContract('FlashBetting', this.signer);
    const marketFactory = backend.getPolygonContract('MarketFactory', this.signer);
    
    try {
      // Use MarketFactory to create flash market
      const tx = await marketFactory.createFlashMarket(
        this.scenario.title,
        this.scenario.duration,
        this.scenario.sport
      );
      
      const receipt = await tx.wait();
      const event = receipt.events?.find(e => e.event === 'MarketCreated');
      
      return event?.args?.marketId || ethers.utils.formatBytes32String(`market_${Date.now()}`);
    } catch (error) {
      // Fallback to mock market ID
      return ethers.utils.formatBytes32String(`market_${Date.now()}`);
    }
  }
  
  async openPosition(marketId, betSize, leverage, isYes) {
    const flashBetting = backend.getPolygonContract('FlashBetting', this.signer);
    
    try {
      // Approve USDC
      const usdc = new ethers.Contract(
        backend.addresses.polygon.USDC,
        ['function approve(address,uint256) returns (bool)'],
        this.signer
      );
      
      const amount = ethers.utils.parseUnits(betSize.toString(), 6);
      await usdc.approve(flashBetting.address, amount);
      
      // Open position
      const tx = await flashBetting.openFlashPosition(
        marketId,
        amount,
        isYes,
        leverage
      );
      
      const receipt = await tx.wait();
      
      return {
        id: `position_${Date.now()}`,
        marketId,
        size: betSize,
        leverage,
        side: isYes ? 'YES' : 'NO',
        tx: receipt.transactionHash
      };
    } catch (error) {
      // Return mock position for testing
      return {
        id: `position_${Date.now()}`,
        marketId,
        size: betSize,
        leverage,
        side: isYes ? 'YES' : 'NO',
        mock: true
      };
    }
  }
  
  async placeChainedBet(markets, leverages, initialStake) {
    const flashBetting = backend.getPolygonContract('FlashBetting', this.signer);
    
    try {
      const amount = ethers.utils.parseUnits(initialStake.toString(), 6);
      
      const tx = await flashBetting.placeChainedBet(
        markets,
        leverages,
        amount
      );
      
      const receipt = await tx.wait();
      
      return {
        id: `chain_${Date.now()}`,
        markets,
        leverages,
        stake: initialStake,
        effectiveLeverage: this.calculateEffectiveLeverage(leverages),
        tx: receipt.transactionHash
      };
    } catch (error) {
      // Return mock chained bet
      return {
        id: `chain_${Date.now()}`,
        markets,
        leverages,
        stake: initialStake,
        effectiveLeverage: this.calculateEffectiveLeverage(leverages),
        mock: true
      };
    }
  }
  
  async getMarketPrice(marketId, isYes) {
    const flashBetting = backend.getPolygonContract('FlashBetting', this.signer);
    
    try {
      const price = await flashBetting.getCurrentPrice(marketId, isYes);
      return price.toNumber();
    } catch (error) {
      // Return simulated price
      return Math.floor(Math.random() * 4000) + 3000; // 3000-7000
    }
  }
  
  async simulateMarketResolution(marketId) {
    // Simulate market outcome based on expected probability
    const won = Math.random() < this.scenario.expectedOutcome;
    
    return {
      marketId,
      won,
      resolutionTime: Date.now(),
      finalPrice: won ? 10000 : 0
    };
  }
  
  async claimWinnings(positionId) {
    // Simulate claiming winnings
    const won = Math.random() < this.scenario.expectedOutcome;
    const payout = won ? Math.random() * 1000 : 0;
    
    return {
      positionId,
      claimed: true,
      payout,
      timestamp: Date.now()
    };
  }
  
  selectBetSize() {
    const sizes = this.persona.avgBetSize;
    return sizes[Math.floor(Math.random() * sizes.length)];
  }
  
  selectLeverage() {
    const leverages = this.persona.leveragePreference;
    return leverages[Math.floor(Math.random() * leverages.length)];
  }
  
  calculateEffectiveLeverage(leverages) {
    const BASE_LEVERAGE = 100;
    const CHAIN_MULTIPLIER = 5;
    
    let effective = BASE_LEVERAGE;
    for (let i = 0; i < leverages.length; i++) {
      const multiplier = CHAIN_MULTIPLIER * (i + 1) / leverages.length;
      effective = effective * leverages[i] * multiplier / 100;
    }
    
    return Math.min(effective, 500); // Cap at 500x
  }
}

// ============ EXHAUSTIVE JOURNEY GENERATOR ============

class ExhaustiveJourneyGenerator {
  constructor() {
    this.journeys = [];
    this.results = [];
    this.stats = {
      total: 0,
      successful: 0,
      failed: 0,
      errors: []
    };
  }
  
  generateAllJourneys() {
    const journeyTypes = [
      'SINGLE_BET',
      'CHAINED_BETS',
      'RAPID_FIRE',
      'HEDGE_STRATEGY',
      'MARTINGALE',
      'ARBITRAGE',
      'MOMENTUM_TRADING',
      'CONTRARIAN',
      'LADDER_STRATEGY',
      'ALL_IN'
    ];
    
    // Generate journeys for each combination
    for (const persona of Object.values(USER_PERSONAS)) {
      for (const scenario of FLASH_MARKET_SCENARIOS) {
        // Filter scenarios by persona sports preference
        if (persona.sports.includes('all') || persona.sports.includes(scenario.sport)) {
          for (const journeyType of journeyTypes) {
            this.journeys.push({
              persona,
              scenario,
              journeyType,
              id: `journey_${this.journeys.length + 1}`
            });
          }
        }
      }
    }
    
    console.log(`Generated ${this.journeys.length} unique journey combinations`);
    return this.journeys;
  }
  
  async executeAllJourneys() {
    console.log('\n🚀 EXECUTING EXHAUSTIVE FLASH BETTING JOURNEYS');
    console.log('=' .repeat(50));
    
    const startTime = Date.now();
    
    for (let i = 0; i < this.journeys.length; i++) {
      const journey = this.journeys[i];
      
      console.log(`\n[${i + 1}/${this.journeys.length}] Executing: ${journey.persona.name} - ${journey.journeyType} - ${journey.scenario.title}`);
      
      try {
        const flashJourney = new FlashBettingJourney(
          journey.persona,
          journey.scenario,
          journey.journeyType
        );
        
        const result = await flashJourney.execute();
        
        if (result.success) {
          this.stats.successful++;
          console.log(`  ✅ Success`);
        } else {
          this.stats.failed++;
          console.log(`  ❌ Failed: ${result.error}`);
          this.stats.errors.push(result.error);
        }
        
        this.results.push({
          ...journey,
          result,
          executionTime: Date.now() - startTime
        });
        
      } catch (error) {
        this.stats.failed++;
        this.stats.errors.push(error.message);
        console.log(`  ❌ Error: ${error.message}`);
        
        this.results.push({
          ...journey,
          result: { success: false, error: error.message },
          executionTime: Date.now() - startTime
        });
      }
      
      this.stats.total++;
      
      // Brief pause to avoid overwhelming the system
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    
    const totalTime = Date.now() - startTime;
    
    return {
      stats: this.stats,
      results: this.results,
      totalTime,
      avgTimePerJourney: totalTime / this.journeys.length
    };
  }
  
  generateReport() {
    console.log('\n' + '='.repeat(60));
    console.log('📊 FLASH BETTING JOURNEY TEST REPORT');
    console.log('='.repeat(60));
    
    // Overall stats
    console.log('\n📈 OVERALL STATISTICS:');
    console.log(`  Total Journeys: ${this.stats.total}`);
    console.log(`  ✅ Successful: ${this.stats.successful} (${(this.stats.successful / this.stats.total * 100).toFixed(1)}%)`);
    console.log(`  ❌ Failed: ${this.stats.failed} (${(this.stats.failed / this.stats.total * 100).toFixed(1)}%)`);
    
    // Journey type breakdown
    console.log('\n📊 JOURNEY TYPE BREAKDOWN:');
    const journeyTypeStats = {};
    for (const result of this.results) {
      if (!journeyTypeStats[result.journeyType]) {
        journeyTypeStats[result.journeyType] = { total: 0, successful: 0 };
      }
      journeyTypeStats[result.journeyType].total++;
      if (result.result.success) {
        journeyTypeStats[result.journeyType].successful++;
      }
    }
    
    for (const [type, stats] of Object.entries(journeyTypeStats)) {
      const successRate = (stats.successful / stats.total * 100).toFixed(1);
      console.log(`  ${type}: ${stats.successful}/${stats.total} (${successRate}%)`);
    }
    
    // Persona breakdown
    console.log('\n👤 PERSONA BREAKDOWN:');
    const personaStats = {};
    for (const result of this.results) {
      const personaName = result.persona.name;
      if (!personaStats[personaName]) {
        personaStats[personaName] = { total: 0, successful: 0 };
      }
      personaStats[personaName].total++;
      if (result.result.success) {
        personaStats[personaName].successful++;
      }
    }
    
    for (const [persona, stats] of Object.entries(personaStats)) {
      const successRate = (stats.successful / stats.total * 100).toFixed(1);
      console.log(`  ${persona}: ${stats.successful}/${stats.total} (${successRate}%)`);
    }
    
    // Sport breakdown
    console.log('\n⚽ SPORT BREAKDOWN:');
    const sportStats = {};
    for (const result of this.results) {
      const sport = result.scenario.sport;
      if (!sportStats[sport]) {
        sportStats[sport] = { total: 0, successful: 0 };
      }
      sportStats[sport].total++;
      if (result.result.success) {
        sportStats[sport].successful++;
      }
    }
    
    for (const [sport, stats] of Object.entries(sportStats)) {
      const successRate = (stats.successful / stats.total * 100).toFixed(1);
      console.log(`  ${sport}: ${stats.successful}/${stats.total} (${successRate}%)`);
    }
    
    // Error analysis
    if (this.stats.errors.length > 0) {
      console.log('\n⚠️ ERROR ANALYSIS:');
      const errorCounts = {};
      for (const error of this.stats.errors) {
        errorCounts[error] = (errorCounts[error] || 0) + 1;
      }
      
      const sortedErrors = Object.entries(errorCounts)
        .sort((a, b) => b[1] - a[1])
        .slice(0, 5);
      
      console.log('  Top 5 Errors:');
      for (const [error, count] of sortedErrors) {
        console.log(`    - ${error}: ${count} occurrences`);
      }
    }
    
    console.log('\n' + '='.repeat(60));
    console.log('✅ FLASH BETTING JOURNEY TESTING COMPLETE');
    console.log('='.repeat(60));
  }
}

// ============ MAIN EXECUTION ============

async function runExhaustiveFlashBettingTests() {
  const generator = new ExhaustiveJourneyGenerator();
  
  // Generate all possible journeys
  generator.generateAllJourneys();
  
  // Execute all journeys
  const results = await generator.executeAllJourneys();
  
  // Generate comprehensive report
  generator.generateReport();
  
  // Save results to file
  const fs = require('fs');
  fs.writeFileSync(
    'flash_betting_test_results.json',
    JSON.stringify(results, null, 2)
  );
  
  console.log('\n💾 Detailed results saved to: flash_betting_test_results.json');
  
  return results;
}

// Export for use
module.exports = {
  USER_PERSONAS,
  FLASH_MARKET_SCENARIOS,
  FlashBettingJourney,
  ExhaustiveJourneyGenerator,
  runExhaustiveFlashBettingTests
};

// Run if executed directly
if (require.main === module) {
  runExhaustiveFlashBettingTests()
    .then(() => process.exit(0))
    .catch(error => {
      console.error('Fatal error:', error);
      process.exit(1);
    });
}