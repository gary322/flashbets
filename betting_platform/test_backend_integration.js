const { 
  initPolygonProvider, 
  initSolanaConnection, 
  getPolygonContract,
  addresses,
  getDeploymentStats,
  interfaces 
} = require('./backend_integration');

const { ethers } = require('ethers');
const { PublicKey } = require('@solana/web3.js');

// Color codes for console output
const colors = {
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  reset: '\x1b[0m'
};

function logSuccess(message) {
  console.log(`${colors.green}✅ ${message}${colors.reset}`);
}

function logError(message) {
  console.log(`${colors.red}❌ ${message}${colors.reset}`);
}

function logInfo(message) {
  console.log(`${colors.blue}ℹ️  ${message}${colors.reset}`);
}

function logHeader(message) {
  console.log(`\n${colors.yellow}${'='.repeat(50)}${colors.reset}`);
  console.log(`${colors.yellow}${message}${colors.reset}`);
  console.log(`${colors.yellow}${'='.repeat(50)}${colors.reset}`);
}

async function testPolygonConnection() {
  logHeader('Testing Polygon Connection');
  
  try {
    const { provider, signer } = initPolygonProvider();
    
    // Test provider connection
    const blockNumber = await provider.getBlockNumber();
    logSuccess(`Connected to Polygon at block ${blockNumber}`);
    
    // Test signer
    const address = await signer.getAddress();
    const balance = await signer.getBalance();
    logSuccess(`Signer address: ${address}`);
    logSuccess(`Signer balance: ${ethers.utils.formatEther(balance)} ETH`);
    
    return true;
  } catch (error) {
    logError(`Polygon connection failed: ${error.message}`);
    return false;
  }
}

async function testPolygonContracts() {
  logHeader('Testing Polygon Contracts');
  
  try {
    const { signer } = initPolygonProvider();
    
    // Test each deployed contract
    const contracts = [
      'BettingPlatform',
      'PolymarketIntegration',
      'MarketFactory',
      'FlashBetting',
      'LeverageVault',
      'LiquidityPool'
    ];
    
    for (const contractName of contracts) {
      try {
        const contract = getPolygonContract(contractName, signer);
        const address = contract.address;
        
        // Verify contract is deployed (has code)
        const code = await signer.provider.getCode(address);
        if (code === '0x') {
          logError(`${contractName} not deployed at ${address}`);
          continue;
        }
        
        logSuccess(`${contractName} deployed at ${address}`);
        
        // Test a view function based on contract type
        if (contractName === 'BettingPlatform') {
          const totalVolume = await contract.totalVolume();
          logInfo(`  Total volume: ${totalVolume.toString()}`);
        } else if (contractName === 'MarketFactory') {
          const totalMarkets = await contract.totalMarketsCreated();
          logInfo(`  Total markets created: ${totalMarkets.toString()}`);
        } else if (contractName === 'LeverageVault') {
          const maxLeverage = await contract.MAX_LEVERAGE();
          logInfo(`  Max leverage: ${maxLeverage.toString()}x`);
        } else if (contractName === 'FlashBetting') {
          const maxDuration = await contract.MAX_FLASH_DURATION();
          logInfo(`  Max flash duration: ${maxDuration.toString()} seconds`);
        }
      } catch (error) {
        logError(`Error testing ${contractName}: ${error.message}`);
      }
    }
    
    return true;
  } catch (error) {
    logError(`Contract testing failed: ${error.message}`);
    return false;
  }
}

async function testSolanaConnection() {
  logHeader('Testing Solana Connection');
  
  try {
    const connection = initSolanaConnection();
    
    // Test connection
    const slot = await connection.getSlot();
    logSuccess(`Connected to Solana at slot ${slot}`);
    
    // Get cluster info
    const version = await connection.getVersion();
    logSuccess(`Solana version: ${version['solana-core']}`);
    
    // Check balance
    const balance = await connection.getBalance(new PublicKey('11111111111111111111111111111111'));
    logSuccess(`System program balance: ${balance / 1e9} SOL`);
    
    return true;
  } catch (error) {
    logError(`Solana connection failed: ${error.message}`);
    return false;
  }
}

async function testABILoading() {
  logHeader('Testing ABI Loading');
  
  try {
    const abis = interfaces.polygon;
    
    for (const [contractName, abi] of Object.entries(abis)) {
      const functionCount = abi.filter(item => item.type === 'function').length;
      const eventCount = abi.filter(item => item.type === 'event').length;
      
      logSuccess(`${contractName} ABI loaded:`);
      logInfo(`  Functions: ${functionCount}`);
      logInfo(`  Events: ${eventCount}`);
    }
    
    return true;
  } catch (error) {
    logError(`ABI loading failed: ${error.message}`);
    return false;
  }
}

async function testIDLLoading() {
  logHeader('Testing IDL Loading');
  
  try {
    const idls = interfaces.solana;
    
    for (const [programName, idl] of Object.entries(idls)) {
      logSuccess(`${programName} IDL loaded:`);
      logInfo(`  Program ID: ${idl.programId}`);
      logInfo(`  Instructions: ${idl.instructions.length}`);
      logInfo(`  Accounts: ${idl.accounts.length}`);
    }
    
    return true;
  } catch (error) {
    logError(`IDL loading failed: ${error.message}`);
    return false;
  }
}

async function testDeploymentStats() {
  logHeader('Testing Deployment Statistics');
  
  try {
    const stats = await getDeploymentStats();
    
    logSuccess('Polygon Stats:');
    logInfo(`  Block number: ${stats.polygon.blockNumber}`);
    logInfo(`  Chain ID: ${stats.polygon.chainId}`);
    logInfo(`  Total contracts: ${stats.polygon.totalContracts}`);
    logInfo(`  Contracts: ${stats.polygon.contracts.join(', ')}`);
    
    logSuccess('Solana Stats:');
    logInfo(`  Current slot: ${stats.solana.slot}`);
    logInfo(`  Total programs: ${stats.solana.totalPrograms}`);
    logInfo(`  Programs: ${stats.solana.programs.join(', ')}`);
    
    logSuccess('Deployment Info:');
    logInfo(`  Timestamp: ${stats.deployment.timestamp}`);
    logInfo(`  Deployer: ${stats.deployment.deployer}`);
    
    return true;
  } catch (error) {
    logError(`Stats retrieval failed: ${error.message}`);
    return false;
  }
}

async function displayContractAddresses() {
  logHeader('Contract Addresses');
  
  console.log('\n📍 Polygon Contracts:');
  for (const [name, address] of Object.entries(addresses.polygon)) {
    console.log(`  ${name}: ${address}`);
  }
  
  console.log('\n📍 Solana Programs:');
  for (const [name, programId] of Object.entries(addresses.solana)) {
    console.log(`  ${name}: ${programId}`);
  }
}

async function runAllTests() {
  console.log('\n');
  console.log('🚀 BETTING PLATFORM BACKEND INTEGRATION TEST');
  console.log('============================================');
  
  const tests = [
    { name: 'Polygon Connection', fn: testPolygonConnection },
    { name: 'Polygon Contracts', fn: testPolygonContracts },
    { name: 'Solana Connection', fn: testSolanaConnection },
    { name: 'ABI Loading', fn: testABILoading },
    { name: 'IDL Loading', fn: testIDLLoading },
    { name: 'Deployment Stats', fn: testDeploymentStats }
  ];
  
  let passed = 0;
  let failed = 0;
  
  for (const test of tests) {
    const result = await test.fn();
    if (result) {
      passed++;
    } else {
      failed++;
    }
  }
  
  // Display addresses
  await displayContractAddresses();
  
  // Summary
  logHeader('Test Summary');
  console.log(`Total tests: ${tests.length}`);
  console.log(`${colors.green}Passed: ${passed}${colors.reset}`);
  if (failed > 0) {
    console.log(`${colors.red}Failed: ${failed}${colors.reset}`);
  }
  
  if (failed === 0) {
    console.log(`\n${colors.green}🎉 ALL TESTS PASSED! Backend integration is working correctly.${colors.reset}`);
    console.log('\n📝 Next Steps:');
    console.log('1. Use the backend_integration module in your API');
    console.log('2. Call functions like openPolygonPosition(), createFlashMarket(), etc.');
    console.log('3. Access contract instances with getPolygonContract()');
    console.log('4. Monitor both chains with getDeploymentStats()');
  } else {
    console.log(`\n${colors.red}⚠️  Some tests failed. Please check the errors above.${colors.reset}`);
  }
}

// Run tests
runAllTests().catch(console.error);