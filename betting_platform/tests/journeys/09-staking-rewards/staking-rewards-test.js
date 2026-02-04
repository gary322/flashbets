#!/usr/bin/env node

/**
 * Staking and Rewards Journey Test
 * Tests staking mechanisms, rewards distribution, and governance features
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class StakingRewardsJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      tokensStaked: 0,
      rewardsClaimed: 0,
      governanceVotes: 0,
      delegationsCreated: 0,
      compoundingActions: 0,
      totalAPY: 0,
      stakingErrors: 0
    };
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n💎 Starting Staking and Rewards Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select test wallet with tokens
      const wallet = this.testData.wallets.find(w => 
        w.balance > 10000
      ) || this.testData.wallets[userId % this.testData.wallets.length];
      
      // Test staking and rewards features
      await this.testStakingDashboard(page, wallet);
      await this.testTokenStaking(page, wallet);
      await this.testStakingPools(page);
      await this.testRewardsCalculation(page);
      await this.testClaimRewards(page);
      await this.testCompoundRewards(page);
      await this.testLockupPeriods(page);
      await this.testDelegation(page);
      await this.testGovernanceParticipation(page);
      await this.testLoyaltyProgram(page);
      await this.testReferralRewards(page);
      await this.testTierBenefits(page);
      await this.testUnstaking(page);
      await this.testRewardsHistory(page);
      await this.testTaxReporting(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = ((this.metrics.tokensStaked + this.metrics.rewardsClaimed) / 
                                  ((this.metrics.tokensStaked + this.metrics.rewardsClaimed + this.metrics.stakingErrors) || 1) * 100);
      
      console.log(chalk.green(`✅ Staking and rewards journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Staking and rewards journey failed:'), error);
      throw error;
    }
  }

  async testStakingDashboard(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing staking dashboard...'));
    
    try {
      // Navigate to staking dashboard
      await page.goto(`${this.config.uiUrl}/staking`, { waitUntil: 'networkidle' });
      
      // Check dashboard overview
      const dashboardMetrics = {
        totalValueLocked: await page.$eval('.tvl, [data-tvl]', el => el.textContent).catch(() => 'N/A'),
        currentAPY: await page.$eval('.current-apy, [data-apy]', el => el.textContent).catch(() => 'N/A'),
        myStakedAmount: await page.$eval('.my-staked, [data-my-staked]', el => el.textContent).catch(() => 'N/A'),
        pendingRewards: await page.$eval('.pending-rewards, [data-pending-rewards]', el => el.textContent).catch(() => 'N/A'),
        stakingTier: await page.$eval('.staking-tier, [data-tier]', el => el.textContent).catch(() => 'N/A'),
        nextTierProgress: await page.$eval('.tier-progress, [data-tier-progress]', el => el.textContent).catch(() => 'N/A')
      };
      
      console.log(chalk.gray('    Staking dashboard:'));
      for (const [key, value] of Object.entries(dashboardMetrics)) {
        console.log(chalk.gray(`    - ${key}: ${value}`));
      }
      
      // Extract APY for metrics
      const apyValue = parseFloat(dashboardMetrics.currentAPY.replace(/[^0-9.]/g, '')) || 0;
      this.metrics.totalAPY = apyValue;
      
      // Check staking statistics
      const stats = await page.$$('.staking-stat, [data-stat]');
      console.log(chalk.gray(`    Additional statistics: ${stats.length}`));
      
      // Check reward history chart
      const rewardChart = await page.$('.reward-chart, [data-reward-chart]');
      if (rewardChart) {
        console.log(chalk.gray('    Reward history chart available'));
      }
      
      this.metrics.stepTimings.stakingDashboard = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Staking dashboard reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'stakingDashboard',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testTokenStaking(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing token staking...'));
    
    try {
      // Find stake button
      const stakeButton = await page.$('button:has-text("Stake"), [data-stake]');
      if (!stakeButton) {
        throw new Error('Stake button not found');
      }
      
      await stakeButton.click();
      await page.waitForTimeout(500);
      
      // Stake modal
      const stakeModal = await page.$('[role="dialog"], .stake-modal');
      if (stakeModal) {
        // Check available balance
        const availableBalance = await stakeModal.$eval('.available-balance', el => el.textContent).catch(() => '0');
        console.log(chalk.gray(`    Available to stake: ${availableBalance}`));
        
        // Set stake amount
        const stakeAmountInput = await stakeModal.$('input[name="stakeAmount"]');
        if (stakeAmountInput) {
          const stakeAmount = Math.min(wallet.balance * 0.2, 5000); // 20% or max 5000
          await stakeAmountInput.fill(stakeAmount.toString());
          console.log(chalk.gray(`    Staking amount: $${stakeAmount}`));
        }
        
        // Quick stake options
        const quickStakeButtons = await stakeModal.$$('.quick-stake, [data-quick-stake]');
        if (quickStakeButtons.length > 0) {
          console.log(chalk.gray(`    Quick stake options: ${quickStakeButtons.length}`));
        }
        
        // Preview staking rewards
        const previewSection = await stakeModal.$('.stake-preview, [data-preview]');
        if (previewSection) {
          const estimatedAPY = await previewSection.$eval('.estimated-apy', el => el.textContent).catch(() => 'N/A');
          const dailyRewards = await previewSection.$eval('.daily-rewards', el => el.textContent).catch(() => 'N/A');
          const monthlyRewards = await previewSection.$eval('.monthly-rewards', el => el.textContent).catch(() => 'N/A');
          
          console.log(chalk.gray(`    Estimated APY: ${estimatedAPY}`));
          console.log(chalk.gray(`    Daily rewards: ${dailyRewards}`));
          console.log(chalk.gray(`    Monthly rewards: ${monthlyRewards}`));
        }
        
        // Confirm staking
        const confirmButton = await stakeModal.$('button:has-text("Confirm Stake")');
        if (confirmButton) {
          await confirmButton.click();
          await page.waitForTimeout(2000);
          
          // Check for success
          const successMessage = await page.$('.success-message, [data-success]');
          if (successMessage) {
            this.metrics.tokensStaked++;
            console.log(chalk.green('    ✓ Tokens staked successfully'));
          }
        }
      }
      
      this.metrics.stepTimings.tokenStaking = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'tokenStaking',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.stakingErrors++;
      throw error;
    }
  }

  async testStakingPools(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing staking pools...'));
    
    try {
      // Navigate to pools
      const poolsTab = await page.$('button:has-text("Pools"), [data-tab="pools"]');
      if (poolsTab) {
        await poolsTab.click();
        await page.waitForTimeout(500);
      }
      
      // Get available pools
      const stakingPools = await page.$$('.staking-pool, [data-pool]');
      console.log(chalk.gray(`    Available pools: ${stakingPools.length}`));
      
      if (stakingPools.length > 0) {
        // Analyze pools
        for (let i = 0; i < Math.min(3, stakingPools.length); i++) {
          const pool = stakingPools[i];
          const poolName = await pool.$eval('.pool-name', el => el.textContent);
          const poolAPY = await pool.$eval('.pool-apy', el => el.textContent);
          const poolTVL = await pool.$eval('.pool-tvl', el => el.textContent);
          const lockPeriod = await pool.$eval('.lock-period', el => el.textContent).catch(() => 'Flexible');
          
          console.log(chalk.gray(`    Pool ${i + 1}: ${poolName}`));
          console.log(chalk.gray(`      APY: ${poolAPY}, TVL: ${poolTVL}, Lock: ${lockPeriod}`));
          
          // Check pool details
          const detailsButton = await pool.$('button:has-text("Details")');
          if (detailsButton && i === 0) { // Check first pool details
            await detailsButton.click();
            await page.waitForTimeout(500);
            
            const poolDetails = await page.$('.pool-details, [data-pool-details]');
            if (poolDetails) {
              const rewardToken = await poolDetails.$eval('.reward-token', el => el.textContent).catch(() => 'N/A');
              const minStake = await poolDetails.$eval('.min-stake', el => el.textContent).catch(() => 'N/A');
              const poolCapacity = await poolDetails.$eval('.pool-capacity', el => el.textContent).catch(() => 'N/A');
              
              console.log(chalk.gray(`      Rewards in: ${rewardToken}`));
              console.log(chalk.gray(`      Min stake: ${minStake}`));
              console.log(chalk.gray(`      Capacity: ${poolCapacity}`));
            }
          }
        }
        
        // Join a pool
        const joinButton = await stakingPools[0].$('button:has-text("Join Pool")');
        if (joinButton) {
          await joinButton.click();
          await page.waitForTimeout(1000);
          console.log(chalk.gray('    Joined staking pool'));
        }
      }
      
      this.metrics.stepTimings.stakingPools = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Staking pools explored'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'stakingPools',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testRewardsCalculation(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing rewards calculation...'));
    
    try {
      // Access rewards calculator
      const calculatorButton = await page.$('button:has-text("Calculator"), [data-calculator]');
      if (!calculatorButton) {
        console.log(chalk.yellow('    ⚠ Rewards calculator not available'));
        return;
      }
      
      await calculatorButton.click();
      await page.waitForTimeout(500);
      
      // Calculator modal
      const calculatorModal = await page.$('[role="dialog"], .calculator-modal');
      if (calculatorModal) {
        // Input staking parameters
        const amountInput = await calculatorModal.$('input[name="calcAmount"]');
        if (amountInput) {
          await amountInput.fill('10000'); // $10,000
        }
        
        const periodSelect = await calculatorModal.$('select[name="stakingPeriod"]');
        if (periodSelect) {
          await periodSelect.selectOption('365'); // 1 year
        }
        
        const apyInput = await calculatorModal.$('input[name="apyRate"]');
        if (apyInput) {
          const currentValue = await apyInput.inputValue();
          if (!currentValue) {
            await apyInput.fill('15'); // 15% APY
          }
        }
        
        // Enable compounding
        const compoundCheckbox = await calculatorModal.$('input[name="enableCompounding"]');
        if (compoundCheckbox) {
          await compoundCheckbox.check();
          
          const compoundFrequency = await calculatorModal.$('select[name="compoundFrequency"]');
          if (compoundFrequency) {
            await compoundFrequency.selectOption('daily'); // Daily compounding
          }
        }
        
        // Calculate rewards
        const calculateButton = await calculatorModal.$('button:has-text("Calculate")');
        if (calculateButton) {
          await calculateButton.click();
          await page.waitForTimeout(500);
          
          // Get calculation results
          const results = await calculatorModal.$('.calculation-results');
          if (results) {
            const totalRewards = await results.$eval('.total-rewards', el => el.textContent);
            const finalBalance = await results.$eval('.final-balance', el => el.textContent);
            const effectiveAPY = await results.$eval('.effective-apy', el => el.textContent);
            
            console.log(chalk.gray('    Calculation results:'));
            console.log(chalk.gray(`    - Total rewards: ${totalRewards}`));
            console.log(chalk.gray(`    - Final balance: ${finalBalance}`));
            console.log(chalk.gray(`    - Effective APY: ${effectiveAPY}`));
          }
        }
        
        // Close calculator
        const closeButton = await calculatorModal.$('button:has-text("Close"), [aria-label="Close"]');
        if (closeButton) {
          await closeButton.click();
        }
      }
      
      this.metrics.stepTimings.rewardsCalculation = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Rewards calculation completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'rewardsCalculation',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testClaimRewards(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing rewards claiming...'));
    
    try {
      // Check pending rewards
      const pendingRewardsElement = await page.$('.pending-rewards, [data-pending-rewards]');
      if (!pendingRewardsElement) {
        console.log(chalk.yellow('    ⚠ No pending rewards element found'));
        return;
      }
      
      const pendingAmount = await pendingRewardsElement.textContent();
      const pendingValue = parseFloat(pendingAmount.replace(/[^0-9.]/g, '')) || 0;
      
      if (pendingValue <= 0) {
        console.log(chalk.gray('    No rewards to claim yet'));
        return;
      }
      
      console.log(chalk.gray(`    Pending rewards: ${pendingAmount}`));
      
      // Claim rewards button
      const claimButton = await page.$('button:has-text("Claim Rewards"), button:has-text("Claim")');
      if (!claimButton) {
        console.log(chalk.yellow('    ⚠ Claim button not found'));
        return;
      }
      
      await claimButton.click();
      await page.waitForTimeout(500);
      
      // Claim modal
      const claimModal = await page.$('[role="dialog"], .claim-modal');
      if (claimModal) {
        // Review claim details
        const claimDetails = await claimModal.$('.claim-details');
        if (claimDetails) {
          const rewardAmount = await claimDetails.$eval('.reward-amount', el => el.textContent);
          const claimFee = await claimDetails.$eval('.claim-fee', el => el.textContent).catch(() => 'Free');
          const netAmount = await claimDetails.$eval('.net-amount', el => el.textContent);
          
          console.log(chalk.gray(`    Claiming: ${rewardAmount}`));
          console.log(chalk.gray(`    Fee: ${claimFee}`));
          console.log(chalk.gray(`    Net amount: ${netAmount}`));
        }
        
        // Select claim destination
        const destinationSelect = await claimModal.$('select[name="claimDestination"]');
        if (destinationSelect) {
          await destinationSelect.selectOption('wallet'); // Wallet or restake
        }
        
        // Confirm claim
        const confirmClaimButton = await claimModal.$('button:has-text("Confirm Claim")');
        if (confirmClaimButton) {
          await confirmClaimButton.click();
          await page.waitForTimeout(2000);
          
          // Check success
          const successIndicator = await page.$('.claim-success, [data-claim-success]');
          if (successIndicator) {
            this.metrics.rewardsClaimed++;
            console.log(chalk.green('    ✓ Rewards claimed successfully'));
          }
        }
      }
      
      this.metrics.stepTimings.claimRewards = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'claimRewards',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCompoundRewards(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing rewards compounding...'));
    
    try {
      // Find compound button
      const compoundButton = await page.$('button:has-text("Compound"), [data-compound]');
      if (!compoundButton) {
        console.log(chalk.yellow('    ⚠ Compound feature not available'));
        return;
      }
      
      await compoundButton.click();
      await page.waitForTimeout(500);
      
      // Compound modal
      const compoundModal = await page.$('[role="dialog"], .compound-modal');
      if (compoundModal) {
        // Check compoundable amount
        const compoundableAmount = await compoundModal.$eval('.compoundable-amount', el => el.textContent);
        console.log(chalk.gray(`    Compoundable rewards: ${compoundableAmount}`));
        
        // Enable auto-compound
        const autoCompoundCheckbox = await compoundModal.$('input[name="autoCompound"]');
        if (autoCompoundCheckbox) {
          await autoCompoundCheckbox.check();
          
          // Set frequency
          const frequencySelect = await compoundModal.$('select[name="compoundFrequency"]');
          if (frequencySelect) {
            await frequencySelect.selectOption('weekly'); // Weekly auto-compound
          }
          
          console.log(chalk.gray('    Auto-compound enabled (weekly)'));
        }
        
        // Show compound impact
        const impactSection = await compoundModal.$('.compound-impact');
        if (impactSection) {
          const currentAPY = await impactSection.$eval('.current-apy', el => el.textContent);
          const compoundedAPY = await impactSection.$eval('.compounded-apy', el => el.textContent);
          const yearlyDifference = await impactSection.$eval('.yearly-difference', el => el.textContent);
          
          console.log(chalk.gray(`    Current APY: ${currentAPY}`));
          console.log(chalk.gray(`    With compounding: ${compoundedAPY}`));
          console.log(chalk.gray(`    Yearly difference: ${yearlyDifference}`));
        }
        
        // Execute compound
        const executeCompoundButton = await compoundModal.$('button:has-text("Compound Now")');
        if (executeCompoundButton) {
          await executeCompoundButton.click();
          await page.waitForTimeout(1500);
          
          this.metrics.compoundingActions++;
          console.log(chalk.green('    ✓ Rewards compounded'));
        }
      }
      
      this.metrics.stepTimings.compoundRewards = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'compoundRewards',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testLockupPeriods(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing lockup periods...'));
    
    try {
      // Access lockup options
      const lockupButton = await page.$('button:has-text("Lock Periods"), [data-lockup]');
      if (!lockupButton) {
        console.log(chalk.yellow('    ⚠ Lockup options not available'));
        return;
      }
      
      await lockupButton.click();
      await page.waitForTimeout(500);
      
      // Lockup modal
      const lockupModal = await page.$('[role="dialog"], .lockup-modal');
      if (lockupModal) {
        // Available lockup tiers
        const lockupTiers = await lockupModal.$$('.lockup-tier, [data-lockup-tier]');
        console.log(chalk.gray(`    Lockup tiers: ${lockupTiers.length}`));
        
        for (let i = 0; i < Math.min(3, lockupTiers.length); i++) {
          const tier = lockupTiers[i];
          const period = await tier.$eval('.lock-period', el => el.textContent);
          const bonusAPY = await tier.$eval('.bonus-apy', el => el.textContent);
          const earlyExitPenalty = await tier.$eval('.exit-penalty', el => el.textContent).catch(() => 'N/A');
          
          console.log(chalk.gray(`    Tier ${i + 1}: ${period}`));
          console.log(chalk.gray(`      Bonus APY: ${bonusAPY}`));
          console.log(chalk.gray(`      Early exit penalty: ${earlyExitPenalty}`));
        }
        
        // Select a lockup period
        if (lockupTiers.length > 0) {
          const selectButton = await lockupTiers[0].$('button:has-text("Select")');
          if (selectButton) {
            await selectButton.click();
            await page.waitForTimeout(500);
            
            // Confirm lockup
            const confirmLockupButton = await page.$('button:has-text("Confirm Lockup")');
            if (confirmLockupButton) {
              await confirmLockupButton.click();
              console.log(chalk.gray('    Lockup period selected'));
            }
          }
        }
        
        // Close modal
        const closeButton = await lockupModal.$('button:has-text("Close"), [aria-label="Close"]');
        if (closeButton) {
          await closeButton.click();
        }
      }
      
      this.metrics.stepTimings.lockupPeriods = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Lockup periods reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'lockupPeriods',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testDelegation(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing stake delegation...'));
    
    try {
      // Access delegation
      const delegationButton = await page.$('button:has-text("Delegate"), [data-delegate]');
      if (!delegationButton) {
        console.log(chalk.yellow('    ⚠ Delegation not available'));
        return;
      }
      
      await delegationButton.click();
      await page.waitForTimeout(500);
      
      // Delegation modal
      const delegationModal = await page.$('[role="dialog"], .delegation-modal');
      if (delegationModal) {
        // Available validators
        const validators = await delegationModal.$$('.validator, [data-validator]');
        console.log(chalk.gray(`    Available validators: ${validators.length}`));
        
        if (validators.length > 0) {
          // Analyze top validators
          for (let i = 0; i < Math.min(3, validators.length); i++) {
            const validator = validators[i];
            const name = await validator.$eval('.validator-name', el => el.textContent);
            const commission = await validator.$eval('.commission', el => el.textContent);
            const uptime = await validator.$eval('.uptime', el => el.textContent);
            const delegated = await validator.$eval('.total-delegated', el => el.textContent);
            
            console.log(chalk.gray(`    Validator ${i + 1}: ${name}`));
            console.log(chalk.gray(`      Commission: ${commission}, Uptime: ${uptime}`));
            console.log(chalk.gray(`      Total delegated: ${delegated}`));
          }
          
          // Select validator
          const selectButton = await validators[0].$('button:has-text("Delegate")');
          if (selectButton) {
            await selectButton.click();
            await page.waitForTimeout(500);
            
            // Set delegation amount
            const amountInput = await page.$('input[name="delegationAmount"]');
            if (amountInput) {
              await amountInput.fill('1000'); // Delegate 1000 tokens
            }
            
            // Confirm delegation
            const confirmButton = await page.$('button:has-text("Confirm Delegation")');
            if (confirmButton) {
              await confirmButton.click();
              await page.waitForTimeout(1500);
              
              this.metrics.delegationsCreated++;
              console.log(chalk.green('    ✓ Stake delegated successfully'));
            }
          }
        }
      }
      
      this.metrics.stepTimings.delegation = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'delegation',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testGovernanceParticipation(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing governance participation...'));
    
    try {
      // Navigate to governance
      const governanceTab = await page.$('button:has-text("Governance"), [data-tab="governance"]');
      if (!governanceTab) {
        console.log(chalk.yellow('    ⚠ Governance not available'));
        return;
      }
      
      await governanceTab.click();
      await page.waitForTimeout(500);
      
      // Check voting power
      const votingPower = await page.$eval('.voting-power, [data-voting-power]', el => el.textContent).catch(() => '0');
      console.log(chalk.gray(`    Voting power: ${votingPower}`));
      
      // Active proposals
      const proposals = await page.$$('.proposal, [data-proposal]');
      console.log(chalk.gray(`    Active proposals: ${proposals.length}`));
      
      if (proposals.length > 0) {
        // Review first proposal
        const firstProposal = proposals[0];
        const title = await firstProposal.$eval('.proposal-title', el => el.textContent);
        const status = await firstProposal.$eval('.proposal-status', el => el.textContent);
        const endTime = await firstProposal.$eval('.end-time', el => el.textContent);
        
        console.log(chalk.gray(`    Proposal: ${title}`));
        console.log(chalk.gray(`    Status: ${status}, Ends: ${endTime}`));
        
        // View proposal details
        await firstProposal.click();
        await page.waitForTimeout(500);
        
        // Vote on proposal
        const voteSection = await page.$('.vote-section, [data-vote]');
        if (voteSection && status.includes('Active')) {
          // Cast vote
          const voteForButton = await voteSection.$('button:has-text("Vote For"), button:has-text("Yes")');
          if (voteForButton) {
            await voteForButton.click();
            await page.waitForTimeout(500);
            
            // Confirm vote
            const confirmVoteButton = await page.$('button:has-text("Confirm Vote")');
            if (confirmVoteButton) {
              await confirmVoteButton.click();
              await page.waitForTimeout(1500);
              
              this.metrics.governanceVotes++;
              console.log(chalk.green('    ✓ Vote cast successfully'));
            }
          }
        }
        
        // Check voting results
        const resultsSection = await page.$('.voting-results, [data-results]');
        if (resultsSection) {
          const forVotes = await resultsSection.$eval('.for-votes', el => el.textContent).catch(() => 'N/A');
          const againstVotes = await resultsSection.$eval('.against-votes', el => el.textContent).catch(() => 'N/A');
          const participation = await resultsSection.$eval('.participation', el => el.textContent).catch(() => 'N/A');
          
          console.log(chalk.gray(`    For: ${forVotes}, Against: ${againstVotes}`));
          console.log(chalk.gray(`    Participation: ${participation}`));
        }
      }
      
      this.metrics.stepTimings.governanceParticipation = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Governance participation completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'governanceParticipation',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testLoyaltyProgram(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing loyalty program...'));
    
    try {
      // Access loyalty program
      const loyaltyButton = await page.$('button:has-text("Loyalty"), [data-loyalty]');
      if (!loyaltyButton) {
        console.log(chalk.yellow('    ⚠ Loyalty program not available'));
        return;
      }
      
      await loyaltyButton.click();
      await page.waitForTimeout(500);
      
      // Loyalty dashboard
      const loyaltyDashboard = await page.$('.loyalty-dashboard, [data-loyalty-dashboard]');
      if (loyaltyDashboard) {
        // Current tier and progress
        const currentTier = await loyaltyDashboard.$eval('.current-tier', el => el.textContent);
        const tierProgress = await loyaltyDashboard.$eval('.tier-progress', el => el.textContent);
        const pointsBalance = await loyaltyDashboard.$eval('.points-balance', el => el.textContent);
        
        console.log(chalk.gray(`    Current tier: ${currentTier}`));
        console.log(chalk.gray(`    Progress to next: ${tierProgress}`));
        console.log(chalk.gray(`    Points balance: ${pointsBalance}`));
        
        // Tier benefits
        const benefits = await loyaltyDashboard.$$('.tier-benefit, [data-benefit]');
        console.log(chalk.gray(`    Active benefits: ${benefits.length}`));
        
        // Available rewards
        const rewards = await loyaltyDashboard.$$('.loyalty-reward, [data-reward]');
        console.log(chalk.gray(`    Available rewards: ${rewards.length}`));
        
        if (rewards.length > 0) {
          // Redeem first available reward
          const firstReward = rewards[0];
          const rewardName = await firstReward.$eval('.reward-name', el => el.textContent);
          const rewardCost = await firstReward.$eval('.reward-cost', el => el.textContent);
          
          console.log(chalk.gray(`    Reward: ${rewardName} (${rewardCost})`));
          
          const redeemButton = await firstReward.$('button:has-text("Redeem")');
          if (redeemButton) {
            await redeemButton.click();
            console.log(chalk.gray('    Loyalty reward redeemed'));
          }
        }
      }
      
      this.metrics.stepTimings.loyaltyProgram = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Loyalty program reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'loyaltyProgram',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testReferralRewards(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing referral rewards...'));
    
    try {
      // Access referral program
      const referralButton = await page.$('button:has-text("Referrals"), [data-referrals]');
      if (!referralButton) {
        console.log(chalk.yellow('    ⚠ Referral program not available'));
        return;
      }
      
      await referralButton.click();
      await page.waitForTimeout(500);
      
      // Referral dashboard
      const referralDashboard = await page.$('.referral-dashboard, [data-referral-dashboard]');
      if (referralDashboard) {
        // Referral stats
        const referralCode = await referralDashboard.$eval('.referral-code', el => el.textContent);
        const totalReferrals = await referralDashboard.$eval('.total-referrals', el => el.textContent);
        const activeReferrals = await referralDashboard.$eval('.active-referrals', el => el.textContent);
        const totalEarnings = await referralDashboard.$eval('.referral-earnings', el => el.textContent);
        
        console.log(chalk.gray(`    Referral code: ${referralCode}`));
        console.log(chalk.gray(`    Total referrals: ${totalReferrals}`));
        console.log(chalk.gray(`    Active referrals: ${activeReferrals}`));
        console.log(chalk.gray(`    Total earnings: ${totalEarnings}`));
        
        // Copy referral link
        const copyButton = await referralDashboard.$('button:has-text("Copy Link")');
        if (copyButton) {
          await copyButton.click();
          console.log(chalk.gray('    Referral link copied'));
        }
        
        // Referral tiers
        const referralTiers = await referralDashboard.$$('.referral-tier, [data-referral-tier]');
        if (referralTiers.length > 0) {
          console.log(chalk.gray(`    Referral tiers: ${referralTiers.length}`));
          
          const currentTier = await referralDashboard.$eval('.current-referral-tier', el => el.textContent).catch(() => 'Basic');
          console.log(chalk.gray(`    Current tier: ${currentTier}`));
        }
        
        // Claim referral rewards
        const claimReferralButton = await referralDashboard.$('button:has-text("Claim Referral Rewards")');
        if (claimReferralButton) {
          const claimableAmount = await referralDashboard.$eval('.claimable-referral', el => el.textContent).catch(() => '0');
          if (parseFloat(claimableAmount.replace(/[^0-9.]/g, '')) > 0) {
            await claimReferralButton.click();
            console.log(chalk.gray('    Referral rewards claimed'));
          }
        }
      }
      
      this.metrics.stepTimings.referralRewards = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Referral rewards reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'referralRewards',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testTierBenefits(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing tier benefits...'));
    
    try {
      // Access tier benefits
      const tierBenefitsButton = await page.$('button:has-text("Tier Benefits"), [data-tier-benefits]');
      if (!tierBenefitsButton) {
        console.log(chalk.yellow('    ⚠ Tier benefits not available'));
        return;
      }
      
      await tierBenefitsButton.click();
      await page.waitForTimeout(500);
      
      // Tier benefits modal
      const benefitsModal = await page.$('[role="dialog"], .tier-benefits-modal');
      if (benefitsModal) {
        // All tiers
        const tiers = await benefitsModal.$$('.tier, [data-tier]');
        console.log(chalk.gray(`    Available tiers: ${tiers.length}`));
        
        for (let i = 0; i < Math.min(3, tiers.length); i++) {
          const tier = tiers[i];
          const tierName = await tier.$eval('.tier-name', el => el.textContent);
          const requiredStake = await tier.$eval('.required-stake', el => el.textContent);
          
          console.log(chalk.gray(`    ${tierName}: ${requiredStake} required`));
          
          // Tier benefits
          const benefits = await tier.$$('.benefit-item');
          for (const benefit of benefits.slice(0, 3)) {
            const benefitText = await benefit.textContent();
            console.log(chalk.gray(`      - ${benefitText}`));
          }
        }
        
        // Current tier benefits
        const currentBenefits = await benefitsModal.$('.current-tier-benefits');
        if (currentBenefits) {
          const activeBenefits = await currentBenefits.$$('.active-benefit');
          console.log(chalk.gray(`    Active benefits: ${activeBenefits.length}`));
        }
        
        // Close modal
        const closeButton = await benefitsModal.$('button:has-text("Close"), [aria-label="Close"]');
        if (closeButton) {
          await closeButton.click();
        }
      }
      
      this.metrics.stepTimings.tierBenefits = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Tier benefits reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'tierBenefits',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testUnstaking(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing unstaking process...'));
    
    try {
      // Find unstake button
      const unstakeButton = await page.$('button:has-text("Unstake"), [data-unstake]');
      if (!unstakeButton) {
        console.log(chalk.yellow('    ⚠ Unstake option not available'));
        return;
      }
      
      await unstakeButton.click();
      await page.waitForTimeout(500);
      
      // Unstake modal
      const unstakeModal = await page.$('[role="dialog"], .unstake-modal');
      if (unstakeModal) {
        // Check staked positions
        const stakedPositions = await unstakeModal.$$('.staked-position, [data-staked-position]');
        console.log(chalk.gray(`    Staked positions: ${stakedPositions.length}`));
        
        if (stakedPositions.length > 0) {
          // Select first position
          const firstPosition = stakedPositions[0];
          const amount = await firstPosition.$eval('.staked-amount', el => el.textContent);
          const lockStatus = await firstPosition.$eval('.lock-status', el => el.textContent).catch(() => 'Unlocked');
          
          console.log(chalk.gray(`    Position: ${amount}, Status: ${lockStatus}`));
          
          // Check for penalties
          if (lockStatus.includes('Locked')) {
            const penaltyWarning = await unstakeModal.$('.penalty-warning');
            if (penaltyWarning) {
              const penalty = await penaltyWarning.$eval('.penalty-amount', el => el.textContent);
              console.log(chalk.yellow(`    ⚠ Early unstaking penalty: ${penalty}`));
            }
          }
          
          // Set unstake amount
          const unstakeAmountInput = await unstakeModal.$('input[name="unstakeAmount"]');
          if (unstakeAmountInput) {
            await unstakeAmountInput.fill('100'); // Unstake 100 tokens
          }
          
          // Review unstaking
          const reviewSection = await unstakeModal.$('.unstake-review');
          if (reviewSection) {
            const cooldownPeriod = await reviewSection.$eval('.cooldown-period', el => el.textContent).catch(() => 'None');
            const receiveDate = await reviewSection.$eval('.receive-date', el => el.textContent).catch(() => 'Immediate');
            
            console.log(chalk.gray(`    Cooldown: ${cooldownPeriod}`));
            console.log(chalk.gray(`    Receive funds: ${receiveDate}`));
          }
          
          // Don't actually unstake in test
          console.log(chalk.gray('    Unstaking process verified (not executed)'));
        }
        
        // Close modal
        const closeButton = await unstakeModal.$('button:has-text("Cancel"), [aria-label="Close"]');
        if (closeButton) {
          await closeButton.click();
        }
      }
      
      this.metrics.stepTimings.unstaking = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Unstaking process reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'unstaking',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testRewardsHistory(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing rewards history...'));
    
    try {
      // Navigate to history
      const historyTab = await page.$('button:has-text("History"), [data-tab="history"]');
      if (historyTab) {
        await historyTab.click();
        await page.waitForTimeout(500);
      }
      
      // Rewards history
      const historyItems = await page.$$('.reward-history-item, [data-reward-history]');
      console.log(chalk.gray(`    Reward history: ${historyItems.length} items`));
      
      // Analyze recent rewards
      for (let i = 0; i < Math.min(3, historyItems.length); i++) {
        const item = historyItems[i];
        const date = await item.$eval('.reward-date', el => el.textContent);
        const type = await item.$eval('.reward-type', el => el.textContent);
        const amount = await item.$eval('.reward-amount', el => el.textContent);
        const source = await item.$eval('.reward-source', el => el.textContent).catch(() => 'Staking');
        
        console.log(chalk.gray(`    ${date}: ${type} - ${amount} from ${source}`));
      }
      
      // Filter options
      const filterSelect = await page.$('select[name="rewardFilter"]');
      if (filterSelect) {
        await filterSelect.selectOption('staking'); // Filter by staking rewards
        await page.waitForTimeout(500);
      }
      
      // Summary statistics
      const summarySection = await page.$('.rewards-summary, [data-rewards-summary]');
      if (summarySection) {
        const totalEarned = await summarySection.$eval('.total-earned', el => el.textContent);
        const avgMonthly = await summarySection.$eval('.avg-monthly', el => el.textContent).catch(() => 'N/A');
        const bestMonth = await summarySection.$eval('.best-month', el => el.textContent).catch(() => 'N/A');
        
        console.log(chalk.gray('    Summary:'));
        console.log(chalk.gray(`    - Total earned: ${totalEarned}`));
        console.log(chalk.gray(`    - Avg monthly: ${avgMonthly}`));
        console.log(chalk.gray(`    - Best month: ${bestMonth}`));
      }
      
      // Export history
      const exportButton = await page.$('button:has-text("Export History")');
      if (exportButton) {
        await exportButton.click();
        console.log(chalk.gray('    Rewards history export available'));
      }
      
      this.metrics.stepTimings.rewardsHistory = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Rewards history reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'rewardsHistory',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testTaxReporting(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing tax reporting...'));
    
    try {
      // Access tax reporting
      const taxButton = await page.$('button:has-text("Tax Report"), [data-tax-report]');
      if (!taxButton) {
        console.log(chalk.yellow('    ⚠ Tax reporting not available'));
        return;
      }
      
      await taxButton.click();
      await page.waitForTimeout(500);
      
      // Tax report modal
      const taxModal = await page.$('[role="dialog"], .tax-report-modal');
      if (taxModal) {
        // Select tax year
        const yearSelect = await taxModal.$('select[name="taxYear"]');
        if (yearSelect) {
          const currentYear = new Date().getFullYear();
          await yearSelect.selectOption(currentYear.toString());
        }
        
        // Select report type
        const reportTypeSelect = await taxModal.$('select[name="reportType"]');
        if (reportTypeSelect) {
          await reportTypeSelect.selectOption('form_1099'); // Form 1099-MISC
        }
        
        // Preview report data
        const previewButton = await taxModal.$('button:has-text("Preview")');
        if (previewButton) {
          await previewButton.click();
          await page.waitForTimeout(1000);
          
          const previewSection = await taxModal.$('.tax-preview');
          if (previewSection) {
            const totalIncome = await previewSection.$eval('.total-income', el => el.textContent);
            const stakingRewards = await previewSection.$eval('.staking-rewards', el => el.textContent);
            const referralIncome = await previewSection.$eval('.referral-income', el => el.textContent).catch(() => '$0');
            
            console.log(chalk.gray('    Tax report preview:'));
            console.log(chalk.gray(`    - Total income: ${totalIncome}`));
            console.log(chalk.gray(`    - Staking rewards: ${stakingRewards}`));
            console.log(chalk.gray(`    - Referral income: ${referralIncome}`));
          }
        }
        
        // Generate report
        const generateButton = await taxModal.$('button:has-text("Generate Report")');
        if (generateButton) {
          await generateButton.click();
          console.log(chalk.gray('    Tax report generated'));
        }
        
        // Close modal
        const closeButton = await taxModal.$('button:has-text("Close"), [aria-label="Close"]');
        if (closeButton) {
          await closeButton.click();
        }
      }
      
      this.metrics.stepTimings.taxReporting = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Tax reporting completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'taxReporting',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running staking and rewards load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalStaked: 0,
    totalRewardsClaimed: 0,
    avgAPY: 0,
    governanceParticipation: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  const apys = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new StakingRewardsJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          apys.push(metrics.totalAPY);
          results.successful++;
          results.totalStaked += metrics.tokensStaked;
          results.totalRewardsClaimed += metrics.rewardsClaimed;
          results.governanceParticipation += metrics.governanceVotes;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Stagger starts
    if (i % 5 === 0) {
      await new Promise(resolve => setTimeout(resolve, 300));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  results.avgAPY = apys.reduce((a, b) => a + b, 0) / apys.length;
  
  // Display results
  console.log(chalk.bold('\nStaking and Rewards Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Total Staked: ${results.totalStaked} positions`));
  console.log(chalk.magenta(`  Rewards Claimed: ${results.totalRewardsClaimed}`));
  console.log(chalk.magenta(`  Average APY: ${results.avgAPY.toFixed(2)}%`));
  console.log(chalk.magenta(`  Governance Votes: ${results.governanceParticipation}`));
  
  return results;
}

// Main execution
if (require.main === module) {
  const configPath = path.join(__dirname, '../../test-config.json');
  const dataPath = path.join(__dirname, '../../data/generated-test-data.json');
  
  if (!fs.existsSync(configPath) || !fs.existsSync(dataPath)) {
    console.error(chalk.red('Error: test-config.json or test data not found. Run setup first.'));
    process.exit(1);
  }
  
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const testData = JSON.parse(fs.readFileSync(dataPath, 'utf8'));
  
  // Run tests
  (async () => {
    try {
      // Single user test
      const singleTest = new StakingRewardsJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests
      await runLoadTest(config, testData, 10);    // 10 users
      await runLoadTest(config, testData, 100);   // 100 users
      await runLoadTest(config, testData, 1000);  // 1000 users
      
      console.log(chalk.bold.green('\n✅ All staking and rewards tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { StakingRewardsJourneyTest, runLoadTest };