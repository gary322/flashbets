#!/usr/bin/env node

/**
 * Basic connectivity test to verify all services are running
 */

const fetch = require('node-fetch');
const { Connection } = require('@solana/web3.js');
const chalk = require('chalk').default || require('chalk');

async function testConnectivity() {
  console.log(chalk.bold.blue('🔌 Testing Basic Connectivity\n'));
  
  const results = {
    solana: false,
    api: false,
    frontend: false,
    apiEndpoints: {}
  };
  
  // Test Solana
  try {
    const connection = new Connection('http://localhost:8899', 'confirmed');
    const version = await connection.getVersion();
    results.solana = true;
    console.log(chalk.green('✅ Solana validator running:'), version['solana-core']);
  } catch (error) {
    console.log(chalk.red('❌ Solana validator not accessible'));
  }
  
  // Test API
  try {
    const health = await fetch('http://localhost:8081/health');
    results.api = health.ok;
    const data = await health.json();
    console.log(chalk.green('✅ API running:'), data.status);
    
    // Test specific endpoints
    const endpoints = [
      '/api/wallet/challenge/test',
      '/api/wallet/demo/create',
      '/api/markets',
      '/api/verses'
    ];
    
    for (const endpoint of endpoints) {
      try {
        const response = await fetch(`http://localhost:8081${endpoint}`);
        results.apiEndpoints[endpoint] = response.ok;
        console.log(response.ok ? chalk.green('✅') : chalk.red('❌'), `API endpoint ${endpoint}: ${response.status}`);
      } catch (error) {
        results.apiEndpoints[endpoint] = false;
        console.log(chalk.red('❌'), `API endpoint ${endpoint}: Failed`);
      }
    }
  } catch (error) {
    console.log(chalk.red('❌ API not accessible'));
  }
  
  // Test Frontend
  try {
    const response = await fetch('http://localhost:3000');
    results.frontend = response.ok;
    console.log(chalk.green('✅ Frontend running:'), response.status);
    
    // Check if it's the right app
    const html = await response.text();
    if (html.includes('Next.js') || html.includes('_next')) {
      console.log(chalk.green('✅ Next.js app detected'));
    }
  } catch (error) {
    console.log(chalk.red('❌ Frontend not accessible'));
  }
  
  console.log(chalk.bold.blue('\n📊 Summary:'));
  console.log('Solana:', results.solana ? chalk.green('✅') : chalk.red('❌'));
  console.log('API:', results.api ? chalk.green('✅') : chalk.red('❌'));
  console.log('Frontend:', results.frontend ? chalk.green('✅') : chalk.red('❌'));
  
  return results;
}

if (require.main === module) {
  testConnectivity().catch(console.error);
}

module.exports = testConnectivity;