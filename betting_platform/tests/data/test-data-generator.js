#!/usr/bin/env node

/**
 * Test data generator for comprehensive testing
 * Generates wallets, markets, liquidity, and price history
 */

const { Keypair, PublicKey, Connection, LAMPORTS_PER_SOL } = require('@solana/web3.js');
const faker = require('faker');
const fs = require('fs');
const path = require('path');
const chalk = require('chalk');

class TestDataGenerator {
  constructor(config) {
    this.config = config;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.data = {
      wallets: [],
      markets: [],
      verses: [],
      priceHistory: {},
      liquidityProviders: []
    };
  }

  async generateAll() {
    console.log(chalk.bold.blue('🎲 Generating test data...\n'));
    
    await this.generateWallets(10000);
    await this.generateVerses(10);
    await this.generateMarkets(100);
    await this.generatePriceHistory();
    await this.generateLiquidityProviders(50);
    
    this.saveData();
    console.log(chalk.bold.green('\n✅ Test data generation complete!'));
  }

  async generateWallets(count) {
    console.log(chalk.cyan(`Generating ${count} test wallets...`));
    
    const walletTypes = [
      { type: 'trader', weight: 0.6 },
      { type: 'liquidity_provider', weight: 0.2 },
      { type: 'market_maker', weight: 0.1 },
      { type: 'whale', weight: 0.05 },
      { type: 'bot', weight: 0.05 }
    ];
    
    for (let i = 0; i < count; i++) {
      const keypair = Keypair.generate();
      const type = this.weightedRandom(walletTypes);
      
      // Determine initial balance based on type
      let balance;
      switch (type) {
        case 'whale':
          balance = 100000 + Math.random() * 900000; // 100k-1M
          break;
        case 'market_maker':
          balance = 50000 + Math.random() * 450000; // 50k-500k
          break;
        case 'liquidity_provider':
          balance = 10000 + Math.random() * 90000; // 10k-100k
          break;
        case 'trader':
          balance = 100 + Math.random() * 9900; // 100-10k
          break;
        case 'bot':
          balance = 1000 + Math.random() * 9000; // 1k-10k
          break;
      }
      
      this.data.wallets.push({
        id: i,
        publicKey: keypair.publicKey.toBase58(),
        secretKey: Array.from(keypair.secretKey),
        type,
        balance,
        tradingStyle: this.generateTradingStyle(type),
        riskProfile: this.generateRiskProfile(),
        createdAt: new Date().toISOString()
      });
      
      if (i % 1000 === 0) {
        console.log(chalk.gray(`  Generated ${i} wallets...`));
      }
    }
    
    console.log(chalk.green(`✓ Generated ${count} wallets`));
  }

  async generateVerses(count) {
    console.log(chalk.cyan(`Generating ${count} verses...`));
    
    const categories = [
      'Politics', 'Sports', 'Crypto', 'Finance', 'Technology',
      'Entertainment', 'Science', 'Weather', 'Gaming', 'Social'
    ];
    
    for (let i = 0; i < count; i++) {
      const verseId = BigInt(i + 1);
      
      this.data.verses.push({
        id: verseId.toString(),
        name: `${categories[i]} Verse`,
        category: categories[i],
        description: faker.lorem.paragraph(),
        parentId: i > 0 && Math.random() > 0.5 ? BigInt(Math.floor(Math.random() * i)).toString() : null,
        depth: i === 0 ? 0 : Math.floor(Math.random() * 3),
        marketCount: 0,
        totalVolume: 0,
        createdAt: new Date().toISOString()
      });
    }
    
    console.log(chalk.green(`✓ Generated ${count} verses`));
  }

  async generateMarkets(count) {
    console.log(chalk.cyan(`Generating ${count} markets...`));
    
    const marketTemplates = [
      {
        category: 'Politics',
        templates: [
          'Will {candidate} win the {election}?',
          'Will {party} control the {chamber} after {date}?',
          'Will {bill} pass in Congress by {date}?'
        ]
      },
      {
        category: 'Sports',
        templates: [
          'Will {team} win the {championship}?',
          'Will {player} score over {points} points in {game}?',
          'Will {team1} beat {team2} on {date}?'
        ]
      },
      {
        category: 'Crypto',
        templates: [
          'Will {crypto} reach ${price} by {date}?',
          'Will {crypto} market cap exceed ${amount} in {year}?',
          'Will {protocol} TVL reach ${amount} by {date}?'
        ]
      },
      {
        category: 'Finance',
        templates: [
          'Will {stock} close above ${price} on {date}?',
          'Will the Fed raise rates in {month}?',
          'Will {index} hit {level} by {date}?'
        ]
      },
      {
        category: 'Technology',
        templates: [
          'Will {company} release {product} by {date}?',
          'Will {technology} reach {milestone} users by {date}?',
          'Will {company} acquire {target} in {year}?'
        ]
      }
    ];
    
    for (let i = 0; i < count; i++) {
      const marketId = BigInt(Date.now() + i);
      const verseIndex = Math.floor(Math.random() * this.data.verses.length);
      const verse = this.data.verses[verseIndex];
      const categoryTemplates = marketTemplates.find(t => t.category === verse.category) || marketTemplates[0];
      const template = categoryTemplates.templates[Math.floor(Math.random() * categoryTemplates.templates.length)];
      
      // Generate title from template
      const title = this.fillTemplate(template);
      
      // Settlement time: 1 day to 1 year from now
      const settlementDays = Math.floor(Math.random() * 365) + 1;
      const settlementTime = new Date();
      settlementTime.setDate(settlementTime.getDate() + settlementDays);
      
      const market = {
        id: marketId.toString(),
        verseId: verse.id,
        title,
        description: faker.lorem.paragraph(),
        outcomes: ['Yes', 'No'],
        settlementTime: settlementTime.toISOString(),
        createdAt: new Date().toISOString(),
        initialLiquidity: 1000 + Math.random() * 99000, // 1k-100k
        currentPrices: [0.5, 0.5], // Start at 50/50
        volume24h: 0,
        totalVolume: 0,
        liquidity: 0,
        openInterest: 0,
        ammType: this.randomChoice(['LMSR', 'PMAMM', 'L2AMM']),
        status: 'active'
      };
      
      this.data.markets.push(market);
      verse.marketCount++;
      
      if (i % 10 === 0) {
        console.log(chalk.gray(`  Generated ${i} markets...`));
      }
    }
    
    console.log(chalk.green(`✓ Generated ${count} markets`));
  }

  async generatePriceHistory() {
    console.log(chalk.cyan('Generating price history...'));
    
    for (const market of this.data.markets) {
      const history = [];
      const startDate = new Date(market.createdAt);
      const now = new Date();
      const intervals = Math.floor((now - startDate) / (60 * 1000)); // 1-minute intervals
      
      let price = 0.5;
      
      for (let i = 0; i < Math.min(intervals, 1440); i++) { // Max 24 hours of history
        // Random walk with mean reversion
        const change = (Math.random() - 0.5) * 0.02; // ±2% per interval
        const meanReversion = (0.5 - price) * 0.01; // 1% mean reversion
        price = Math.max(0.01, Math.min(0.99, price + change + meanReversion));
        
        const timestamp = new Date(startDate.getTime() + i * 60 * 1000);
        
        history.push({
          timestamp: timestamp.toISOString(),
          price,
          volume: Math.random() * 1000,
          trades: Math.floor(Math.random() * 10) + 1
        });
      }
      
      this.data.priceHistory[market.id] = history;
      
      // Update current price
      if (history.length > 0) {
        const latestPrice = history[history.length - 1].price;
        market.currentPrices = [latestPrice, 1 - latestPrice];
      }
    }
    
    console.log(chalk.green('✓ Generated price history'));
  }

  async generateLiquidityProviders(count) {
    console.log(chalk.cyan(`Generating ${count} liquidity providers...`));
    
    // Select LP wallets
    const lpWallets = this.data.wallets
      .filter(w => w.type === 'liquidity_provider' || w.type === 'market_maker')
      .slice(0, count);
    
    for (const wallet of lpWallets) {
      // Each LP provides to 1-10 markets
      const marketCount = Math.floor(Math.random() * 10) + 1;
      const selectedMarkets = this.randomSample(this.data.markets, marketCount);
      
      for (const market of selectedMarkets) {
        const liquidity = wallet.balance * (0.1 + Math.random() * 0.4); // 10-50% of balance
        
        this.data.liquidityProviders.push({
          walletId: wallet.id,
          marketId: market.id,
          liquidity,
          shares: liquidity, // Simplified: 1:1 shares
          addedAt: new Date().toISOString()
        });
        
        market.liquidity += liquidity;
      }
    }
    
    console.log(chalk.green(`✓ Generated ${count} liquidity providers`));
  }

  // Helper methods
  generateTradingStyle(walletType) {
    const styles = {
      trader: ['scalper', 'swing', 'position', 'arbitrage'],
      liquidity_provider: ['passive', 'active'],
      market_maker: ['tight_spread', 'wide_spread'],
      whale: ['accumulator', 'manipulator', 'long_term'],
      bot: ['high_frequency', 'arbitrage', 'market_making']
    };
    
    return this.randomChoice(styles[walletType] || ['unknown']);
  }

  generateRiskProfile() {
    return {
      maxLeverage: Math.floor(Math.random() * 100) + 1,
      maxPositionSize: Math.random() * 0.5 + 0.1, // 10-60% of balance
      stopLoss: Math.random() * 0.2 + 0.05, // 5-25%
      takeProfit: Math.random() * 0.5 + 0.1, // 10-60%
      riskScore: Math.floor(Math.random() * 100) + 1
    };
  }

  fillTemplate(template) {
    const replacements = {
      candidate: () => faker.name.findName(),
      election: () => this.randomChoice(['Presidential Election', 'Senate Race', 'Governor Race']),
      party: () => this.randomChoice(['Democratic Party', 'Republican Party']),
      chamber: () => this.randomChoice(['House', 'Senate']),
      date: () => faker.date.future(1).toLocaleDateString(),
      bill: () => `${this.randomChoice(['HR', 'S'])}${Math.floor(Math.random() * 9999)}`,
      team: () => faker.company.companyName() + ' ' + this.randomChoice(['FC', 'United', 'City']),
      team1: () => faker.company.companyName() + ' ' + this.randomChoice(['FC', 'United', 'City']),
      team2: () => faker.company.companyName() + ' ' + this.randomChoice(['FC', 'United', 'City']),
      player: () => faker.name.findName(),
      points: () => Math.floor(Math.random() * 50) + 10,
      game: () => 'Game ' + Math.floor(Math.random() * 82) + 1,
      championship: () => this.randomChoice(['World Cup', 'Super Bowl', 'NBA Finals', 'World Series']),
      crypto: () => this.randomChoice(['BTC', 'ETH', 'SOL', 'MATIC', 'AVAX']),
      price: () => Math.floor(Math.random() * 100000) + 1000,
      amount: () => (Math.floor(Math.random() * 100) + 1) + 'B',
      year: () => new Date().getFullYear() + Math.floor(Math.random() * 3),
      month: () => faker.date.future(1).toLocaleString('default', { month: 'long' }),
      stock: () => this.randomChoice(['AAPL', 'GOOGL', 'AMZN', 'MSFT', 'TSLA']),
      index: () => this.randomChoice(['S&P 500', 'NASDAQ', 'DOW']),
      level: () => Math.floor(Math.random() * 50000) + 10000,
      company: () => faker.company.companyName(),
      product: () => faker.commerce.productName(),
      technology: () => this.randomChoice(['AI', 'VR', 'Blockchain', '5G']),
      milestone: () => (Math.floor(Math.random() * 10) + 1) + 'B',
      target: () => faker.company.companyName(),
      protocol: () => this.randomChoice(['Uniswap', 'Aave', 'Compound', 'MakerDAO'])
    };
    
    return template.replace(/\{(\w+)\}/g, (match, key) => {
      return replacements[key] ? replacements[key]() : match;
    });
  }

  weightedRandom(items) {
    const weights = items.map(item => item.weight);
    const totalWeight = weights.reduce((a, b) => a + b, 0);
    
    let random = Math.random() * totalWeight;
    
    for (let i = 0; i < items.length; i++) {
      random -= weights[i];
      if (random <= 0) {
        return items[i].type;
      }
    }
    
    return items[items.length - 1].type;
  }

  randomChoice(array) {
    return array[Math.floor(Math.random() * array.length)];
  }

  randomSample(array, count) {
    const shuffled = [...array].sort(() => 0.5 - Math.random());
    return shuffled.slice(0, count);
  }

  saveData() {
    const dataPath = path.join(__dirname, 'generated-test-data.json');
    fs.writeFileSync(dataPath, JSON.stringify(this.data, null, 2));
    
    // Save summary
    const summary = {
      wallets: this.data.wallets.length,
      walletsByType: this.data.wallets.reduce((acc, w) => {
        acc[w.type] = (acc[w.type] || 0) + 1;
        return acc;
      }, {}),
      verses: this.data.verses.length,
      markets: this.data.markets.length,
      marketsByVerse: this.data.verses.map(v => ({
        verse: v.name,
        markets: v.marketCount
      })),
      liquidityProviders: this.data.liquidityProviders.length,
      totalLiquidity: this.data.markets.reduce((sum, m) => sum + m.liquidity, 0),
      generatedAt: new Date().toISOString()
    };
    
    const summaryPath = path.join(__dirname, 'test-data-summary.json');
    fs.writeFileSync(summaryPath, JSON.stringify(summary, null, 2));
    
    console.log(chalk.gray(`\nData saved to: ${dataPath}`));
    console.log(chalk.gray(`Summary saved to: ${summaryPath}`));
  }
}

// Run if called directly
if (require.main === module) {
  const configPath = path.join(__dirname, '../test-config.json');
  
  if (!fs.existsSync(configPath)) {
    console.error(chalk.red('Error: test-config.json not found. Run setup-infrastructure.js first.'));
    process.exit(1);
  }
  
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const generator = new TestDataGenerator(config);
  
  generator.generateAll().catch(error => {
    console.error(chalk.red('Error generating test data:'), error);
    process.exit(1);
  });
}

module.exports = { TestDataGenerator };