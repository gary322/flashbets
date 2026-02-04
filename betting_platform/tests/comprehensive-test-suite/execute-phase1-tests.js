#!/usr/bin/env node

/**
 * Execute Phase 1 Tests independently
 */

const fs = require('fs');
const path = require('path');
const chalk = require('chalk').default || require('chalk');
const Phase1Tests = require('./phase1-onboarding-tests');

async function runPhase1() {
  console.log(chalk.bold.cyan('\n📋 PHASE 1: Core User Onboarding & Authentication (25 tests)\n'));
  
  // Load test config
  const configPath = path.join(__dirname, 'test-config.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  
  // Create Phase 1 test instance
  const phase1 = new Phase1Tests(config);
  
  try {
    const results = await phase1.runAll();
    
    console.log(chalk.bold('\nPhase 1 Results:'));
    console.log(chalk.green(`✅ Passed: ${results.passed}`));
    console.log(chalk.red(`❌ Failed: ${results.failed}`));
    
    if (results.errors.length > 0) {
      console.log(chalk.red('\nErrors:'));
      results.errors.forEach(err => {
        console.log(chalk.red(`  ${err.test}: ${err.error}`));
      });
    }
    
    // Save results
    const resultsPath = path.join(__dirname, 'phase1-results.json');
    fs.writeFileSync(resultsPath, JSON.stringify(results, null, 2));
    
    return results;
  } catch (error) {
    console.error(chalk.red('Phase 1 failed:'), error);
    throw error;
  }
}

if (require.main === module) {
  runPhase1().catch(console.error);
}

module.exports = runPhase1;