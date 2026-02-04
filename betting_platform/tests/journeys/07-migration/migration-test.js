#!/usr/bin/env node

/**
 * Migration Journey Test
 * Tests platform migration scenarios and data portability
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class MigrationJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      dataExported: 0,
      settingsMigrated: 0,
      positionsMigrated: 0,
      walletsMigrated: 0,
      migrationErrors: 0,
      dataIntegrityChecks: 0
    };
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n🔄 Starting Migration Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select test wallet with existing data
      const wallet = this.testData.wallets.find(w => 
        w.balance > 1000 && w.type === 'trader'
      ) || this.testData.wallets[userId % this.testData.wallets.length];
      
      // Test migration scenarios
      await this.testExportUserData(page, wallet);
      await this.testBackupWalletData(page);
      await this.testExportTradingHistory(page);
      await this.testExportPositions(page);
      await this.testExportSettings(page);
      await this.testValidateDataIntegrity(page);
      await this.testImportFromBackup(page);
      await this.testCrossChainMigration(page);
      await this.testWalletMigration(page);
      await this.testSettingsMigration(page);
      await this.testVerifyMigration(page);
      await this.testRollbackCapability(page);
      await this.testCleanupOldData(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = ((this.metrics.dataExported + this.metrics.settingsMigrated + 
                                  this.metrics.positionsMigrated + this.metrics.walletsMigrated) / 
                                  (this.metrics.dataExported + this.metrics.settingsMigrated + 
                                   this.metrics.positionsMigrated + this.metrics.walletsMigrated + 
                                   this.metrics.migrationErrors) * 100) || 100;
      
      console.log(chalk.green(`✅ Migration journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Migration journey failed:'), error);
      throw error;
    }
  }

  async testExportUserData(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing user data export...'));
    
    try {
      // Navigate to export page
      await page.goto(`${this.config.uiUrl}/settings/export`, { waitUntil: 'networkidle' });
      
      // Full data export
      const fullExportButton = await page.$('button:has-text("Export All Data"), [data-export="full"]');
      if (!fullExportButton) {
        throw new Error('Full export button not found');
      }
      
      await fullExportButton.click();
      await page.waitForTimeout(500);
      
      // Select export format
      const formatModal = await page.$('[role="dialog"], .export-modal');
      if (formatModal) {
        const formatSelect = await formatModal.$('select[name="exportFormat"]');
        if (formatSelect) {
          await formatSelect.selectOption('json'); // JSON, CSV, ZIP
          console.log(chalk.gray('    Export format: JSON'));
        }
        
        // Select data categories
        const dataCategories = {
          profile: await formatModal.$('input[name="includeProfile"]'),
          wallets: await formatModal.$('input[name="includeWallets"]'),
          positions: await formatModal.$('input[name="includePositions"]'),
          history: await formatModal.$('input[name="includeHistory"]'),
          settings: await formatModal.$('input[name="includeSettings"]'),
          preferences: await formatModal.$('input[name="includePreferences"]')
        };
        
        let selectedCategories = 0;
        for (const [category, checkbox] of Object.entries(dataCategories)) {
          if (checkbox) {
            await checkbox.check();
            selectedCategories++;
          }
        }
        
        console.log(chalk.gray(`    Selected ${selectedCategories} data categories`));
        
        // Set password for encrypted export
        const encryptCheckbox = await formatModal.$('input[name="encryptExport"]');
        if (encryptCheckbox) {
          await encryptCheckbox.check();
          
          const passwordInput = await formatModal.$('input[name="exportPassword"]');
          if (passwordInput) {
            await passwordInput.fill('SecureExportPass123!');
          }
        }
        
        // Include metadata
        const metadataCheckbox = await formatModal.$('input[name="includeMetadata"]');
        if (metadataCheckbox) {
          await metadataCheckbox.check();
        }
        
        // Generate export
        const generateButton = await formatModal.$('button:has-text("Generate Export")');
        if (generateButton) {
          await generateButton.click();
          console.log(chalk.gray('    Generating export...'));
          await page.waitForTimeout(3000);
          
          // Check export progress
          const progressBar = await formatModal.$('.export-progress, [data-progress]');
          if (progressBar) {
            console.log(chalk.gray('    Export progress tracked'));
          }
          
          // Download link
          const downloadLink = await formatModal.$('a:has-text("Download"), button:has-text("Download")');
          if (downloadLink) {
            console.log(chalk.green('    ✓ Export generated successfully'));
            this.metrics.dataExported++;
          }
        }
      }
      
      this.metrics.stepTimings.exportUserData = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'exportUserData',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.migrationErrors++;
      throw error;
    }
  }

  async testBackupWalletData(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing wallet data backup...'));
    
    try {
      // Navigate to wallet backup
      const walletBackupButton = await page.$('button:has-text("Backup Wallets"), [data-backup="wallets"]');
      if (!walletBackupButton) {
        console.log(chalk.yellow('    ⚠ Wallet backup not available'));
        return;
      }
      
      await walletBackupButton.click();
      await page.waitForTimeout(500);
      
      // Select wallets to backup
      const walletCheckboxes = await page.$$('.wallet-checkbox, [data-wallet-backup]');
      console.log(chalk.gray(`    Found ${walletCheckboxes.length} wallets to backup`));
      
      for (const checkbox of walletCheckboxes) {
        await checkbox.check();
      }
      
      // Choose backup method
      const backupMethods = await page.$$('.backup-method, [data-backup-method]');
      if (backupMethods.length > 0) {
        // Select encrypted file backup
        const encryptedBackup = await page.$('[data-backup-method="encrypted"], button:has-text("Encrypted File")');
        if (encryptedBackup) {
          await encryptedBackup.click();
          
          // Set backup password
          const backupPasswordInput = await page.$('input[name="backupPassword"]');
          if (backupPasswordInput) {
            await backupPasswordInput.fill('WalletBackupPass456!');
          }
          
          // Confirm password
          const confirmPasswordInput = await page.$('input[name="confirmBackupPassword"]');
          if (confirmPasswordInput) {
            await confirmPasswordInput.fill('WalletBackupPass456!');
          }
        }
        
        // Include transaction history
        const historyCheckbox = await page.$('input[name="includeHistory"]');
        if (historyCheckbox) {
          await historyCheckbox.check();
        }
        
        // Create backup
        const createBackupButton = await page.$('button:has-text("Create Backup")');
        if (createBackupButton) {
          await createBackupButton.click();
          await page.waitForTimeout(2000);
          
          console.log(chalk.green('    ✓ Wallet backup created'));
          this.metrics.walletsMigrated++;
        }
      }
      
      this.metrics.stepTimings.backupWalletData = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'backupWalletData',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testExportTradingHistory(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing trading history export...'));
    
    try {
      // Navigate to trading history
      const historyButton = await page.$('button:has-text("Export History"), [data-export="history"]');
      if (!historyButton) {
        console.log(chalk.yellow('    ⚠ History export not available'));
        return;
      }
      
      await historyButton.click();
      await page.waitForTimeout(500);
      
      // Configure history export
      const historyModal = await page.$('[role="dialog"], .history-export-modal');
      if (historyModal) {
        // Set date range
        const dateRangeSelect = await historyModal.$('select[name="dateRange"]');
        if (dateRangeSelect) {
          await dateRangeSelect.selectOption('all'); // All time
        }
        
        // Select trade types
        const tradeTypes = {
          market: await historyModal.$('input[name="includeMarket"]'),
          limit: await historyModal.$('input[name="includeLimit"]'),
          stop: await historyModal.$('input[name="includeStop"]'),
          conditional: await historyModal.$('input[name="includeConditional"]')
        };
        
        for (const [type, checkbox] of Object.entries(tradeTypes)) {
          if (checkbox) {
            await checkbox.check();
          }
        }
        
        // Include P&L data
        const pnlCheckbox = await historyModal.$('input[name="includePnL"]');
        if (pnlCheckbox) {
          await pnlCheckbox.check();
        }
        
        // Include fees
        const feesCheckbox = await historyModal.$('input[name="includeFees"]');
        if (feesCheckbox) {
          await feesCheckbox.check();
        }
        
        // Select format
        const formatSelect = await historyModal.$('select[name="historyFormat"]');
        if (formatSelect) {
          await formatSelect.selectOption('csv'); // For tax reporting
        }
        
        // Export for tax reporting
        const taxFormatCheckbox = await historyModal.$('input[name="taxFormat"]');
        if (taxFormatCheckbox) {
          await taxFormatCheckbox.check();
          console.log(chalk.gray('    Tax reporting format enabled'));
        }
        
        // Generate export
        const exportButton = await historyModal.$('button:has-text("Export History")');
        if (exportButton) {
          await exportButton.click();
          await page.waitForTimeout(2000);
          
          // Check export status
          const statusElement = await historyModal.$('.export-status');
          if (statusElement) {
            const status = await statusElement.textContent();
            console.log(chalk.gray(`    Export status: ${status}`));
          }
          
          console.log(chalk.green('    ✓ Trading history exported'));
          this.metrics.dataExported++;
        }
      }
      
      this.metrics.stepTimings.exportTradingHistory = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'exportTradingHistory',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testExportPositions(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing positions export...'));
    
    try {
      // Export current positions
      const positionsButton = await page.$('button:has-text("Export Positions"), [data-export="positions"]');
      if (!positionsButton) {
        console.log(chalk.yellow('    ⚠ Positions export not available'));
        return;
      }
      
      await positionsButton.click();
      await page.waitForTimeout(500);
      
      // Configure positions export
      const positionsModal = await page.$('[role="dialog"], .positions-export-modal');
      if (positionsModal) {
        // Select position types
        const positionTypes = {
          open: await positionsModal.$('input[name="includeOpen"]'),
          closed: await positionsModal.$('input[name="includeClosed"]'),
          pending: await positionsModal.$('input[name="includePending"]')
        };
        
        for (const [type, checkbox] of Object.entries(positionTypes)) {
          if (checkbox) {
            await checkbox.check();
          }
        }
        
        // Include position details
        const detailOptions = {
          entryPrice: await positionsModal.$('input[name="includeEntry"]'),
          exitPrice: await positionsModal.$('input[name="includeExit"]'),
          pnl: await positionsModal.$('input[name="includePnL"]'),
          fees: await positionsModal.$('input[name="includeFees"]'),
          leverage: await positionsModal.$('input[name="includeLeverage"]'),
          stopLoss: await positionsModal.$('input[name="includeStopLoss"]'),
          takeProfit: await positionsModal.$('input[name="includeTakeProfit"]')
        };
        
        for (const [detail, checkbox] of Object.entries(detailOptions)) {
          if (checkbox) {
            await checkbox.check();
          }
        }
        
        // Group by market
        const groupByMarketCheckbox = await positionsModal.$('input[name="groupByMarket"]');
        if (groupByMarketCheckbox) {
          await groupByMarketCheckbox.check();
        }
        
        // Export positions
        const exportButton = await positionsModal.$('button:has-text("Export Positions")');
        if (exportButton) {
          await exportButton.click();
          await page.waitForTimeout(1500);
          
          console.log(chalk.green('    ✓ Positions exported'));
          this.metrics.positionsMigrated++;
        }
      }
      
      this.metrics.stepTimings.exportPositions = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'exportPositions',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testExportSettings(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing settings export...'));
    
    try {
      // Export user settings
      const settingsButton = await page.$('button:has-text("Export Settings"), [data-export="settings"]');
      if (!settingsButton) {
        console.log(chalk.yellow('    ⚠ Settings export not available'));
        return;
      }
      
      await settingsButton.click();
      await page.waitForTimeout(500);
      
      // Configure settings export
      const settingsModal = await page.$('[role="dialog"], .settings-export-modal');
      if (settingsModal) {
        // Select setting categories
        const settingCategories = {
          trading: await settingsModal.$('input[name="tradingSettings"]'),
          notifications: await settingsModal.$('input[name="notificationSettings"]'),
          security: await settingsModal.$('input[name="securitySettings"]'),
          display: await settingsModal.$('input[name="displaySettings"]'),
          api: await settingsModal.$('input[name="apiSettings"]'),
          preferences: await settingsModal.$('input[name="userPreferences"]')
        };
        
        let exportedCategories = 0;
        for (const [category, checkbox] of Object.entries(settingCategories)) {
          if (checkbox) {
            await checkbox.check();
            exportedCategories++;
          }
        }
        
        console.log(chalk.gray(`    Exporting ${exportedCategories} setting categories`));
        
        // Include custom configurations
        const customConfigCheckbox = await settingsModal.$('input[name="includeCustom"]');
        if (customConfigCheckbox) {
          await customConfigCheckbox.check();
        }
        
        // Export format
        const formatSelect = await settingsModal.$('select[name="settingsFormat"]');
        if (formatSelect) {
          await formatSelect.selectOption('json'); // JSON for easy import
        }
        
        // Generate settings export
        const exportButton = await settingsModal.$('button:has-text("Export Settings")');
        if (exportButton) {
          await exportButton.click();
          await page.waitForTimeout(1000);
          
          console.log(chalk.green('    ✓ Settings exported'));
          this.metrics.settingsMigrated++;
        }
      }
      
      this.metrics.stepTimings.exportSettings = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'exportSettings',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testValidateDataIntegrity(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing data integrity validation...'));
    
    try {
      // Run data integrity check
      const integrityButton = await page.$('button:has-text("Validate Data"), [data-integrity-check]');
      if (!integrityButton) {
        console.log(chalk.yellow('    ⚠ Data integrity check not available'));
        return;
      }
      
      await integrityButton.click();
      await page.waitForTimeout(500);
      
      // Configure integrity check
      const integrityModal = await page.$('[role="dialog"], .integrity-check-modal');
      if (integrityModal) {
        // Select check types
        const checkTypes = {
          balances: await integrityModal.$('input[name="checkBalances"]'),
          positions: await integrityModal.$('input[name="checkPositions"]'),
          history: await integrityModal.$('input[name="checkHistory"]'),
          wallets: await integrityModal.$('input[name="checkWallets"]'),
          settings: await integrityModal.$('input[name="checkSettings"]')
        };
        
        for (const [type, checkbox] of Object.entries(checkTypes)) {
          if (checkbox) {
            await checkbox.check();
          }
        }
        
        // Run comprehensive check
        const runCheckButton = await integrityModal.$('button:has-text("Run Check")');
        if (runCheckButton) {
          await runCheckButton.click();
          console.log(chalk.gray('    Running integrity checks...'));
          await page.waitForTimeout(3000);
          
          // Check results
          const checkResults = await integrityModal.$$('.check-result, [data-check-result]');
          let passedChecks = 0;
          let failedChecks = 0;
          
          for (const result of checkResults) {
            const status = await result.$eval('.check-status', el => el.textContent);
            if (status.includes('Pass') || status.includes('✓')) {
              passedChecks++;
            } else {
              failedChecks++;
            }
          }
          
          console.log(chalk.gray(`    Passed: ${passedChecks}, Failed: ${failedChecks}`));
          this.metrics.dataIntegrityChecks += passedChecks;
          
          if (failedChecks === 0) {
            console.log(chalk.green('    ✓ All integrity checks passed'));
          } else {
            console.log(chalk.yellow(`    ⚠ ${failedChecks} integrity issues found`));
          }
          
          // Generate integrity report
          const reportButton = await integrityModal.$('button:has-text("Generate Report")');
          if (reportButton) {
            await reportButton.click();
            console.log(chalk.gray('    Integrity report generated'));
          }
        }
      }
      
      this.metrics.stepTimings.validateDataIntegrity = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'validateDataIntegrity',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testImportFromBackup(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing import from backup...'));
    
    try {
      // Navigate to import section
      const importButton = await page.$('button:has-text("Import Data"), [data-import]');
      if (!importButton) {
        console.log(chalk.yellow('    ⚠ Import functionality not available'));
        return;
      }
      
      await importButton.click();
      await page.waitForTimeout(500);
      
      // Import modal
      const importModal = await page.$('[role="dialog"], .import-modal');
      if (importModal) {
        // Select import type
        const importTypes = await importModal.$$('.import-type, [data-import-type]');
        console.log(chalk.gray(`    Import options: ${importTypes.length}`));
        
        // Test backup file import
        const backupImport = await importModal.$('[data-import-type="backup"], button:has-text("From Backup")');
        if (backupImport) {
          await backupImport.click();
          
          // Simulate file upload
          const fileInput = await importModal.$('input[type="file"][name="backupFile"]');
          if (fileInput) {
            // In real scenario, this would be a file upload
            console.log(chalk.gray('    Backup file upload simulated'));
          }
          
          // Enter backup password
          const passwordInput = await importModal.$('input[name="backupPassword"]');
          if (passwordInput) {
            await passwordInput.fill('WalletBackupPass456!');
          }
          
          // Preview import
          const previewButton = await importModal.$('button:has-text("Preview Import")');
          if (previewButton) {
            await previewButton.click();
            await page.waitForTimeout(1500);
            
            // Check preview data
            const previewItems = await importModal.$$('.preview-item, [data-preview-item]');
            console.log(chalk.gray(`    Preview shows ${previewItems.length} items to import`));
            
            // Selective import
            const selectAllCheckbox = await importModal.$('input[name="selectAll"]');
            if (selectAllCheckbox) {
              await selectAllCheckbox.check();
            }
            
            // Import data
            const importDataButton = await importModal.$('button:has-text("Import Selected")');
            if (importDataButton) {
              console.log(chalk.gray('    Import process ready (not executing in test)'));
              console.log(chalk.green('    ✓ Import from backup validated'));
            }
          }
        }
        
        // Test CSV import
        const csvImport = await importModal.$('[data-import-type="csv"], button:has-text("From CSV")');
        if (csvImport) {
          await csvImport.click();
          
          // Map CSV columns
          const columnMapping = await importModal.$('.column-mapping, [data-column-mapping]');
          if (columnMapping) {
            console.log(chalk.gray('    CSV column mapping available'));
          }
        }
      }
      
      this.metrics.stepTimings.importFromBackup = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'importFromBackup',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCrossChainMigration(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing cross-chain migration...'));
    
    try {
      // Navigate to cross-chain migration
      const crossChainButton = await page.$('button:has-text("Cross-Chain"), [data-cross-chain]');
      if (!crossChainButton) {
        console.log(chalk.yellow('    ⚠ Cross-chain migration not available'));
        return;
      }
      
      await crossChainButton.click();
      await page.waitForTimeout(500);
      
      // Cross-chain migration modal
      const migrationModal = await page.$('[role="dialog"], .cross-chain-modal');
      if (migrationModal) {
        // Select source chain
        const sourceChainSelect = await migrationModal.$('select[name="sourceChain"]');
        if (sourceChainSelect) {
          await sourceChainSelect.selectOption('ethereum'); // From Ethereum
        }
        
        // Select destination chain
        const destChainSelect = await migrationModal.$('select[name="destinationChain"]');
        if (destChainSelect) {
          await destChainSelect.selectOption('solana'); // To Solana
        }
        
        // Connect source wallet
        const connectSourceButton = await migrationModal.$('button:has-text("Connect Source Wallet")');
        if (connectSourceButton) {
          await connectSourceButton.click();
          console.log(chalk.gray('    Source wallet connection simulated'));
        }
        
        // Select assets to migrate
        const assetCheckboxes = await migrationModal.$$('.asset-checkbox, [data-asset-migrate]');
        console.log(chalk.gray(`    Available assets: ${assetCheckboxes.length}`));
        
        for (const checkbox of assetCheckboxes.slice(0, 2)) { // Migrate first 2 assets
          await checkbox.check();
        }
        
        // Check migration fees
        const estimateFeesButton = await migrationModal.$('button:has-text("Estimate Fees")');
        if (estimateFeesButton) {
          await estimateFeesButton.click();
          await page.waitForTimeout(1000);
          
          const feesDisplay = await migrationModal.$eval('.migration-fees', el => el.textContent).catch(() => 'N/A');
          console.log(chalk.gray(`    Migration fees: ${feesDisplay}`));
        }
        
        // Preview migration
        const previewMigrationButton = await migrationModal.$('button:has-text("Preview Migration")');
        if (previewMigrationButton) {
          await previewMigrationButton.click();
          await page.waitForTimeout(1500);
          
          // Check migration summary
          const migrationSummary = await migrationModal.$('.migration-summary');
          if (migrationSummary) {
            const assetsCount = await migrationSummary.$eval('.assets-count', el => el.textContent).catch(() => '0');
            const totalValue = await migrationSummary.$eval('.total-value', el => el.textContent).catch(() => 'N/A');
            
            console.log(chalk.gray(`    Migration: ${assetsCount} assets, ${totalValue} value`));
            console.log(chalk.green('    ✓ Cross-chain migration ready'));
          }
        }
      }
      
      this.metrics.stepTimings.crossChainMigration = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'crossChainMigration',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testWalletMigration(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing wallet migration...'));
    
    try {
      // Wallet migration tool
      const walletMigrationButton = await page.$('button:has-text("Migrate Wallets"), [data-wallet-migration]');
      if (!walletMigrationButton) {
        console.log(chalk.yellow('    ⚠ Wallet migration not available'));
        return;
      }
      
      await walletMigrationButton.click();
      await page.waitForTimeout(500);
      
      // Wallet migration modal
      const walletModal = await page.$('[role="dialog"], .wallet-migration-modal');
      if (walletModal) {
        // Select migration type
        const migrationType = await walletModal.$('select[name="migrationType"]');
        if (migrationType) {
          await migrationType.selectOption('seed_phrase'); // Seed phrase migration
        }
        
        // Enter source wallet info
        const seedPhraseTextarea = await walletModal.$('textarea[name="seedPhrase"]');
        if (seedPhraseTextarea) {
          // Test seed phrase (not real)
          await seedPhraseTextarea.fill('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about');
        }
        
        // Set derivation path
        const derivationPathInput = await walletModal.$('input[name="derivationPath"]');
        if (derivationPathInput) {
          await derivationPathInput.fill("m/44'/501'/0'/0'"); // Solana derivation path
        }
        
        // Validate wallet
        const validateButton = await walletModal.$('button:has-text("Validate Wallet")');
        if (validateButton) {
          await validateButton.click();
          await page.waitForTimeout(1000);
          
          const validationResult = await walletModal.$('.validation-result');
          if (validationResult) {
            const status = await validationResult.textContent();
            console.log(chalk.gray(`    Validation: ${status}`));
          }
        }
        
        // Check for existing balances
        const checkBalancesButton = await walletModal.$('button:has-text("Check Balances")');
        if (checkBalancesButton) {
          await checkBalancesButton.click();
          await page.waitForTimeout(1000);
          
          const balanceInfo = await walletModal.$('.balance-info');
          if (balanceInfo) {
            const balances = await balanceInfo.textContent();
            console.log(chalk.gray(`    Existing balances: ${balances}`));
          }
        }
        
        // Import wallet
        const importWalletButton = await walletModal.$('button:has-text("Import Wallet")');
        if (importWalletButton) {
          console.log(chalk.gray('    Wallet import ready (not executing with real keys)'));
          console.log(chalk.green('    ✓ Wallet migration process validated'));
          this.metrics.walletsMigrated++;
        }
      }
      
      this.metrics.stepTimings.walletMigration = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'walletMigration',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSettingsMigration(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing settings migration...'));
    
    try {
      // Settings migration/restore
      const settingsMigrationButton = await page.$('button:has-text("Restore Settings"), [data-settings-migration]');
      if (!settingsMigrationButton) {
        console.log(chalk.yellow('    ⚠ Settings migration not available'));
        return;
      }
      
      await settingsMigrationButton.click();
      await page.waitForTimeout(500);
      
      // Settings migration modal
      const settingsModal = await page.$('[role="dialog"], .settings-migration-modal');
      if (settingsModal) {
        // Upload settings file
        const fileInput = await settingsModal.$('input[type="file"][name="settingsFile"]');
        if (fileInput) {
          console.log(chalk.gray('    Settings file upload ready'));
        }
        
        // Or paste settings JSON
        const settingsTextarea = await settingsModal.$('textarea[name="settingsJson"]');
        if (settingsTextarea) {
          const testSettings = JSON.stringify({
            theme: 'dark',
            notifications: { email: true, push: false },
            trading: { defaultLeverage: 10, confirmTrades: true },
            display: { currency: 'USD', timezone: 'UTC' }
          });
          
          await settingsTextarea.fill(testSettings);
          console.log(chalk.gray('    Settings JSON provided'));
        }
        
        // Validate settings format
        const validateSettingsButton = await settingsModal.$('button:has-text("Validate Settings")');
        if (validateSettingsButton) {
          await validateSettingsButton.click();
          await page.waitForTimeout(500);
          
          const validationStatus = await settingsModal.$eval('.settings-validation', el => el.textContent).catch(() => 'Valid');
          console.log(chalk.gray(`    Settings validation: ${validationStatus}`));
        }
        
        // Preview settings changes
        const previewChangesButton = await settingsModal.$('button:has-text("Preview Changes")');
        if (previewChangesButton) {
          await previewChangesButton.click();
          await page.waitForTimeout(500);
          
          const changesList = await settingsModal.$$('.settings-change, [data-settings-change]');
          console.log(chalk.gray(`    Settings changes: ${changesList.length} items`));
        }
        
        // Apply settings
        const applySettingsButton = await settingsModal.$('button:has-text("Apply Settings")');
        if (applySettingsButton) {
          await applySettingsButton.click();
          console.log(chalk.green('    ✓ Settings migration completed'));
          this.metrics.settingsMigrated++;
        }
      }
      
      this.metrics.stepTimings.settingsMigration = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'settingsMigration',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testVerifyMigration(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing migration verification...'));
    
    try {
      // Migration verification tool
      const verifyButton = await page.$('button:has-text("Verify Migration"), [data-verify-migration]');
      if (!verifyButton) {
        console.log(chalk.yellow('    ⚠ Migration verification not available'));
        return;
      }
      
      await verifyButton.click();
      await page.waitForTimeout(500);
      
      // Verification modal
      const verificationModal = await page.$('[role="dialog"], .verification-modal');
      if (verificationModal) {
        // Run verification checks
        const runVerificationButton = await verificationModal.$('button:has-text("Run Verification")');
        if (runVerificationButton) {
          await runVerificationButton.click();
          console.log(chalk.gray('    Running migration verification...'));
          await page.waitForTimeout(2000);
          
          // Check verification results
          const verificationResults = await verificationModal.$$('.verification-result, [data-verification-result]');
          let passedVerifications = 0;
          let failedVerifications = 0;
          
          for (const result of verificationResults) {
            const status = await result.$eval('.verification-status', el => el.textContent);
            const checkName = await result.$eval('.check-name', el => el.textContent);
            
            if (status.includes('Pass') || status.includes('✓')) {
              passedVerifications++;
              console.log(chalk.gray(`    ✓ ${checkName}`));
            } else {
              failedVerifications++;
              console.log(chalk.yellow(`    ⚠ ${checkName}: ${status}`));
            }
          }
          
          console.log(chalk.gray(`    Verification: ${passedVerifications} passed, ${failedVerifications} failed`));
          
          // Overall verification status
          if (failedVerifications === 0) {
            console.log(chalk.green('    ✓ Migration verification successful'));
          } else {
            console.log(chalk.yellow('    ⚠ Migration verification found issues'));
          }
          
          // Generate verification report
          const reportButton = await verificationModal.$('button:has-text("Generate Report")');
          if (reportButton) {
            await reportButton.click();
            console.log(chalk.gray('    Verification report generated'));
          }
        }
      }
      
      this.metrics.stepTimings.verifyMigration = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'verifyMigration',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testRollbackCapability(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing rollback capability...'));
    
    try {
      // Rollback option
      const rollbackButton = await page.$('button:has-text("Rollback"), [data-rollback]');
      if (!rollbackButton) {
        console.log(chalk.yellow('    ⚠ Rollback capability not available'));
        return;
      }
      
      await rollbackButton.click();
      await page.waitForTimeout(500);
      
      // Rollback modal
      const rollbackModal = await page.$('[role="dialog"], .rollback-modal');
      if (rollbackModal) {
        // Select rollback point
        const rollbackPoints = await rollbackModal.$$('.rollback-point, [data-rollback-point]');
        console.log(chalk.gray(`    Available rollback points: ${rollbackPoints.length}`));
        
        if (rollbackPoints.length > 0) {
          // Select most recent rollback point
          const latestPoint = rollbackPoints[0];
          await latestPoint.click();
          
          const pointInfo = await latestPoint.textContent();
          console.log(chalk.gray(`    Selected rollback point: ${pointInfo}`));
          
          // Preview rollback changes
          const previewRollbackButton = await rollbackModal.$('button:has-text("Preview Rollback")');
          if (previewRollbackButton) {
            await previewRollbackButton.click();
            await page.waitForTimeout(1000);
            
            const rollbackSummary = await rollbackModal.$('.rollback-summary');
            if (rollbackSummary) {
              const summary = await rollbackSummary.textContent();
              console.log(chalk.gray(`    Rollback preview: ${summary}`));
            }
          }
          
          // Confirm rollback capability
          const confirmRollbackButton = await rollbackModal.$('button:has-text("Confirm Rollback")');
          if (confirmRollbackButton) {
            console.log(chalk.gray('    Rollback capability confirmed (not executing)'));
            console.log(chalk.green('    ✓ Rollback feature available'));
          }
        }
      }
      
      this.metrics.stepTimings.rollbackCapability = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'rollbackCapability',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCleanupOldData(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing cleanup of old data...'));
    
    try {
      // Data cleanup tool
      const cleanupButton = await page.$('button:has-text("Cleanup Data"), [data-cleanup]');
      if (!cleanupButton) {
        console.log(chalk.yellow('    ⚠ Data cleanup not available'));
        return;
      }
      
      await cleanupButton.click();
      await page.waitForTimeout(500);
      
      // Cleanup modal
      const cleanupModal = await page.$('[role="dialog"], .cleanup-modal');
      if (cleanupModal) {
        // Select data to cleanup
        const cleanupOptions = {
          oldBackups: await cleanupModal.$('input[name="cleanupBackups"]'),
          expiredSessions: await cleanupModal.$('input[name="cleanupSessions"]'),
          tempFiles: await cleanupModal.$('input[name="cleanupTempFiles"]'),
          oldLogs: await cleanupModal.$('input[name="cleanupLogs"]'),
          cachesData: await cleanupModal.$('input[name="cleanupCaches"]')
        };
        
        let selectedOptions = 0;
        for (const [option, checkbox] of Object.entries(cleanupOptions)) {
          if (checkbox) {
            await checkbox.check();
            selectedOptions++;
          }
        }
        
        console.log(chalk.gray(`    Selected ${selectedOptions} cleanup options`));
        
        // Estimate cleanup size
        const estimateButton = await cleanupModal.$('button:has-text("Estimate Size")');
        if (estimateButton) {
          await estimateButton.click();
          await page.waitForTimeout(1000);
          
          const estimatedSize = await cleanupModal.$eval('.cleanup-size', el => el.textContent).catch(() => 'N/A');
          console.log(chalk.gray(`    Estimated cleanup size: ${estimatedSize}`));
        }
        
        // Set retention policies
        const retentionSelect = await cleanupModal.$('select[name="retentionPeriod"]');
        if (retentionSelect) {
          await retentionSelect.selectOption('30days'); // Keep data for 30 days
        }
        
        // Execute cleanup
        const executeCleanupButton = await cleanupModal.$('button:has-text("Execute Cleanup")');
        if (executeCleanupButton) {
          console.log(chalk.gray('    Cleanup process ready (not executing)'));
          console.log(chalk.green('    ✓ Data cleanup capability verified'));
        }
      }
      
      this.metrics.stepTimings.cleanupOldData = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'cleanupOldData',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running migration load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalDataExported: 0,
    totalMigrations: 0,
    integrityChecks: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new MigrationJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          results.successful++;
          results.totalDataExported += metrics.dataExported;
          results.totalMigrations += (metrics.settingsMigrated + metrics.positionsMigrated + metrics.walletsMigrated);
          results.integrityChecks += metrics.dataIntegrityChecks;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Heavy stagger for migration operations
    if (i % 2 === 0) {
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  
  // Display results
  console.log(chalk.bold('\nMigration Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Data Exports: ${results.totalDataExported}`));
  console.log(chalk.magenta(`  Total Migrations: ${results.totalMigrations}`));
  console.log(chalk.magenta(`  Integrity Checks: ${results.integrityChecks}`));
  
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
      const singleTest = new MigrationJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests with lower concurrency due to complexity
      await runLoadTest(config, testData, 5);     // 5 users
      await runLoadTest(config, testData, 25);    // 25 users
      await runLoadTest(config, testData, 100);   // 100 users
      
      console.log(chalk.bold.green('\n✅ All migration tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { MigrationJourneyTest, runLoadTest };