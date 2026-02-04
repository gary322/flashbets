const { ethers } = require('ethers');
const { Connection, PublicKey, Keypair } = require('@solana/web3.js');
const fs = require('fs');
const path = require('path');

// Load deployment info
const polygonDeployment = require('./contracts/deployments/localhost-deployment.json');

// Load ABIs
const loadABI = (contractName) => {
  const abiPath = path.join(__dirname, 'contracts/abi', `${contractName}.json`);
  return JSON.parse(fs.readFileSync(abiPath, 'utf8'));
};

// Load IDLs
const loadIDL = (programName) => {
  const idlPath = path.join(__dirname, 'idl', `${programName}.json`);
  return JSON.parse(fs.readFileSync(idlPath, 'utf8'));
};

// Polygon Configuration
const polygonConfig = {
  rpcUrl: 'http://localhost:8545',
  contracts: polygonDeployment.contracts,
  abis: {
    BettingPlatform: loadABI('BettingPlatform'),
    PolymarketIntegration: loadABI('PolymarketIntegration'),
    MarketFactory: loadABI('MarketFactory'),
    FlashBetting: loadABI('FlashBetting'),
    LeverageVault: loadABI('LeverageVault'),
    LiquidityPool: loadABI('LiquidityPool')
  }
};

// Solana Configuration
const solanaConfig = {
  rpcUrl: 'http://localhost:8899',
  programs: {
    bettingPlatform: loadIDL('betting_platform'),
    flashBetting: loadIDL('flash_betting')
  }
};

// Initialize Polygon Provider
const initPolygonProvider = () => {
  const provider = new ethers.providers.JsonRpcProvider(polygonConfig.rpcUrl);
  const signer = new ethers.Wallet(
    '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
    provider
  );
  
  return { provider, signer };
};

// Initialize Solana Connection
const initSolanaConnection = () => {
  const connection = new Connection(solanaConfig.rpcUrl, 'confirmed');
  return connection;
};

// Get Polygon Contract Instance
const getPolygonContract = (contractName, signer) => {
  const address = polygonConfig.contracts[contractName];
  const abi = polygonConfig.abis[contractName];
  
  if (!address) {
    throw new Error(`Contract ${contractName} not found in deployment`);
  }
  
  return new ethers.Contract(address, abi, signer);
};

// ============ POLYGON BETTING FUNCTIONS ============

/**
 * Open a leveraged position on Polygon
 * @param {Object} params - Position parameters
 * @returns {Promise<string>} Position ID
 */
async function openPolygonPosition(params) {
  const { marketId, collateral, leverage, isLong } = params;
  const { signer } = initPolygonProvider();
  const bettingPlatform = getPolygonContract('BettingPlatform', signer);
  
  // Approve USDC spending first
  const usdc = new ethers.Contract(
    polygonConfig.contracts.USDC,
    ['function approve(address,uint256) returns (bool)'],
    signer
  );
  
  await usdc.approve(bettingPlatform.address, collateral);
  
  // Open position
  const tx = await bettingPlatform.openPosition(
    marketId,
    collateral,
    leverage,
    isLong
  );
  
  const receipt = await tx.wait();
  const event = receipt.events.find(e => e.event === 'PositionOpened');
  
  return event.args.positionId;
}

/**
 * Create a flash market on Polygon
 * @param {Object} params - Market parameters
 * @returns {Promise<string>} Market ID
 */
async function createFlashMarket(params) {
  const { title, duration, sport } = params;
  const { signer } = initPolygonProvider();
  const marketFactory = getPolygonContract('MarketFactory', signer);
  
  const tx = await marketFactory.createFlashMarket(title, duration, sport);
  const receipt = await tx.wait();
  const event = receipt.events.find(e => e.event === 'MarketCreated');
  
  return event.args.marketId;
}

/**
 * Open a flash position with leverage chaining
 * @param {Object} params - Flash position parameters
 * @returns {Promise<string>} Position ID
 */
async function openFlashPosition(params) {
  const { marketId, amount, isYes, leverage } = params;
  const { signer } = initPolygonProvider();
  const flashBetting = getPolygonContract('FlashBetting', signer);
  
  // Approve USDC
  const usdc = new ethers.Contract(
    polygonConfig.contracts.USDC,
    ['function approve(address,uint256) returns (bool)'],
    signer
  );
  
  await usdc.approve(flashBetting.address, amount);
  
  // Open flash position
  const tx = await flashBetting.openFlashPosition(
    marketId,
    amount,
    isYes,
    leverage
  );
  
  const receipt = await tx.wait();
  const event = receipt.events.find(e => e.event === 'FlashPositionOpened');
  
  return event.args.positionId;
}

/**
 * Add liquidity to the pool
 * @param {string} amount - Amount of USDC to add
 * @returns {Promise<string>} LP tokens received
 */
async function addLiquidity(amount) {
  const { signer } = initPolygonProvider();
  const liquidityPool = getPolygonContract('LiquidityPool', signer);
  
  // Approve USDC
  const usdc = new ethers.Contract(
    polygonConfig.contracts.USDC,
    ['function approve(address,uint256) returns (bool)'],
    signer
  );
  
  await usdc.approve(liquidityPool.address, amount);
  
  // Add liquidity
  const tx = await liquidityPool.addLiquidity(amount);
  const receipt = await tx.wait();
  const event = receipt.events.find(e => e.event === 'LiquidityAdded');
  
  return event.args.shares;
}

/**
 * Get market price from Polymarket integration
 * @param {string} marketId - Market identifier
 * @returns {Promise<number>} Price in basis points
 */
async function getPolymarketPrice(marketId) {
  const { signer } = initPolygonProvider();
  const polymarket = getPolygonContract('PolymarketIntegration', signer);
  
  const price = await polymarket.getMarketPrice(marketId);
  return price.toNumber();
}

// ============ SOLANA BETTING FUNCTIONS ============

/**
 * Create a verse on Solana
 * @param {Object} params - Verse parameters
 * @returns {Promise<string>} Verse public key
 */
async function createSolanaVerse(params) {
  const { title, category, odds } = params;
  const connection = initSolanaConnection();
  
  // In production, this would use Anchor to interact with the program
  // For now, returning mock data
  const versePubkey = Keypair.generate().publicKey.toString();
  
  console.log('Creating Solana verse:', {
    title,
    category,
    odds,
    pubkey: versePubkey
  });
  
  return versePubkey;
}

/**
 * Place a bet on Solana
 * @param {Object} params - Bet parameters
 * @returns {Promise<string>} Transaction signature
 */
async function placeSolanaBet(params) {
  const { verseId, amount, side } = params;
  const connection = initSolanaConnection();
  
  // Mock transaction for demonstration
  const signature = 'mock_' + Date.now();
  
  console.log('Placing Solana bet:', {
    verseId,
    amount,
    side,
    signature
  });
  
  return signature;
}

// ============ CROSS-CHAIN FUNCTIONS ============

/**
 * Get combined portfolio across both chains
 * @param {string} userAddress - User's address
 * @returns {Promise<Object>} Combined portfolio data
 */
async function getCombinedPortfolio(userAddress) {
  // Get Polygon positions
  const { signer } = initPolygonProvider();
  const bettingPlatform = getPolygonContract('BettingPlatform', signer);
  const polygonPositions = await bettingPlatform.getUserPositions(userAddress);
  
  // Get Solana positions (mock for now)
  const solanaPositions = [];
  
  return {
    polygon: {
      positions: polygonPositions,
      chain: 'Polygon',
      contracts: polygonConfig.contracts
    },
    solana: {
      positions: solanaPositions,
      chain: 'Solana',
      programs: Object.keys(solanaConfig.programs).reduce((acc, key) => {
        acc[key] = solanaConfig.programs[key].programId;
        return acc;
      }, {})
    },
    totalPositions: polygonPositions.length + solanaPositions.length
  };
}

/**
 * Get live market data from both chains
 * @returns {Promise<Object>} Market data
 */
async function getLiveMarkets() {
  const { signer } = initPolygonProvider();
  const marketFactory = getPolygonContract('MarketFactory', signer);
  
  // Get Polygon markets
  const polygonMarkets = await marketFactory.getActiveMarkets();
  
  // Mock Solana markets
  const solanaMarkets = [];
  
  return {
    polygon: polygonMarkets,
    solana: solanaMarkets,
    total: polygonMarkets.length + solanaMarkets.length
  };
}

// ============ UTILITY FUNCTIONS ============

/**
 * Get contract stats
 * @returns {Promise<Object>} Deployment statistics
 */
async function getDeploymentStats() {
  const { provider } = initPolygonProvider();
  const blockNumber = await provider.getBlockNumber();
  const network = await provider.getNetwork();
  
  const connection = initSolanaConnection();
  const slot = await connection.getSlot();
  
  return {
    polygon: {
      blockNumber,
      chainId: network.chainId,
      contracts: Object.keys(polygonConfig.contracts),
      totalContracts: Object.keys(polygonConfig.contracts).length
    },
    solana: {
      slot,
      programs: Object.keys(solanaConfig.programs),
      totalPrograms: Object.keys(solanaConfig.programs).length
    },
    deployment: {
      timestamp: polygonDeployment.timestamp,
      deployer: polygonDeployment.deployer
    }
  };
}

// Export configuration and functions
module.exports = {
  // Configurations
  polygonConfig,
  solanaConfig,
  
  // Initialization
  initPolygonProvider,
  initSolanaConnection,
  getPolygonContract,
  
  // Contract addresses
  addresses: {
    polygon: polygonConfig.contracts,
    solana: {
      bettingPlatform: solanaConfig.programs.bettingPlatform.programId,
      flashBetting: solanaConfig.programs.flashBetting.programId
    }
  },
  
  // ABIs and IDLs
  interfaces: {
    polygon: polygonConfig.abis,
    solana: solanaConfig.programs
  },
  
  // Polygon functions
  openPolygonPosition,
  createFlashMarket,
  openFlashPosition,
  addLiquidity,
  getPolymarketPrice,
  
  // Solana functions
  createSolanaVerse,
  placeSolanaBet,
  
  // Cross-chain functions
  getCombinedPortfolio,
  getLiveMarkets,
  getDeploymentStats
};