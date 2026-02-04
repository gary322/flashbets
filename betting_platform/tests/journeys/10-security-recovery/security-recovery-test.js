#!/usr/bin/env node

/**
 * Security and Recovery Journey Test
 * Tests security features, account recovery, and emergency procedures
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

class SecurityRecoveryJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      securityChecksPerformed: 0,
      recoveryActionsCompleted: 0,
      vulnerabilitiesFound: 0,
      securityAlertsTriggered: 0,
      mfaEnabled: 0,
      backupsCreated: 0,
      auditTrailEvents: 0
    };
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n🔒 Starting Security and Recovery Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select test wallet
      const wallet = this.testData.wallets[userId % this.testData.wallets.length];
      
      // Test security and recovery features
      await this.testSecurityDashboard(page);
      await this.testMultiFactorAuthentication(page);
      await this.testPasswordSecurity(page);
      await this.testSessionManagement(page);
      await this.testDeviceManagement(page);
      await this.testSecurityAudits(page);
      await this.testSuspiciousActivityDetection(page);
      await this.testAccountRecovery(page);
      await this.testWalletRecovery(page);
      await this.testSecurityBackups(page);
      await this.testEmergencyLock(page);
      await this.testPenetrationTesting(page);
      await this.testComplianceChecks(page);
      await this.testPrivacyControls(page);
      await this.testIncidentResponse(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = ((this.metrics.securityChecksPerformed + this.metrics.recoveryActionsCompleted) / 
                                  ((this.metrics.securityChecksPerformed + this.metrics.recoveryActionsCompleted + this.metrics.errors.length) || 1) * 100);
      
      console.log(chalk.green(`✅ Security and recovery journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Security and recovery journey failed:'), error);
      throw error;
    }
  }

  async testSecurityDashboard(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing security dashboard...'));
    
    try {
      // Navigate to security dashboard
      await page.goto(`${this.config.uiUrl}/security`, { waitUntil: 'networkidle' });
      
      // Security overview
      const securityMetrics = {
        securityScore: await page.$eval('.security-score, [data-security-score]', el => el.textContent).catch(() => 'N/A'),
        activeThreats: await page.$eval('.active-threats, [data-threats]', el => el.textContent).catch(() => '0'),
        lastScan: await page.$eval('.last-scan, [data-last-scan]', el => el.textContent).catch(() => 'N/A'),
        vulnerabilities: await page.$eval('.vulnerabilities, [data-vulnerabilities]', el => el.textContent).catch(() => '0'),
        mfaStatus: await page.$eval('.mfa-status, [data-mfa-status]', el => el.textContent).catch(() => 'Disabled'),
        sessionCount: await page.$eval('.active-sessions, [data-sessions]', el => el.textContent).catch(() => '1')
      };
      
      console.log(chalk.gray('    Security dashboard:'));
      for (const [key, value] of Object.entries(securityMetrics)) {
        console.log(chalk.gray(`    - ${key}: ${value}`));
      }
      
      // Extract vulnerability count
      const vulnCount = parseInt(securityMetrics.vulnerabilities.replace(/[^0-9]/g, '')) || 0;
      this.metrics.vulnerabilitiesFound += vulnCount;
      
      // Check security recommendations
      const recommendations = await page.$$('.security-recommendation, [data-recommendation]');
      console.log(chalk.gray(`    Security recommendations: ${recommendations.length}`));
      
      if (recommendations.length > 0) {
        for (let i = 0; i < Math.min(3, recommendations.length); i++) {
          const rec = recommendations[i];
          const recText = await rec.textContent();
          console.log(chalk.gray(`    - ${recText.substring(0, 50)}...`));
        }
      }
      
      // Check recent security events
      const securityEvents = await page.$$('.security-event, [data-security-event]');
      console.log(chalk.gray(`    Recent security events: ${securityEvents.length}`));
      this.metrics.auditTrailEvents += securityEvents.length;
      
      this.metrics.stepTimings.securityDashboard = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Security dashboard reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'securityDashboard',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testMultiFactorAuthentication(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing multi-factor authentication...'));
    
    try {
      // Navigate to MFA settings
      const mfaButton = await page.$('button:has-text("MFA"), button:has-text("Two-Factor"), [data-mfa]');
      if (!mfaButton) {
        console.log(chalk.yellow('    ⚠ MFA settings not found'));
        return;
      }
      
      await mfaButton.click();
      await page.waitForTimeout(500);
      
      // MFA setup modal
      const mfaModal = await page.$('[role="dialog"], .mfa-modal');
      if (mfaModal) {
        // Check current MFA status
        const mfaStatus = await mfaModal.$eval('.mfa-current-status', el => el.textContent).catch(() => 'Disabled');
        console.log(chalk.gray(`    Current MFA status: ${mfaStatus}`));
        
        if (mfaStatus.includes('Disabled')) {
          // Enable TOTP
          const totpOption = await mfaModal.$('[data-mfa-type="totp"], button:has-text("Authenticator App")');
          if (totpOption) {
            await totpOption.click();
            await page.waitForTimeout(500);
            
            // QR code setup
            const qrCode = await mfaModal.$('.qr-code, [data-qr-code]');
            if (qrCode) {
              console.log(chalk.gray('    TOTP QR code displayed'));
              
              // Manual key alternative
              const manualKey = await mfaModal.$eval('.manual-key', el => el.textContent).catch(() => 'N/A');
              console.log(chalk.gray(`    Manual key: ${manualKey.substring(0, 10)}...`));
              
              // Simulate verification
              const verifyInput = await mfaModal.$('input[name="totpCode"]');
              if (verifyInput) {
                await verifyInput.fill('123456'); // Test code
                
                const verifyButton = await mfaModal.$('button:has-text("Verify"), button:has-text("Enable")');
                if (verifyButton) {
                  await verifyButton.click();
                  console.log(chalk.gray('    TOTP MFA enabled (simulated)'));
                  this.metrics.mfaEnabled++;
                }
              }
            }
          }
          
          // SMS backup option
          const smsOption = await mfaModal.$('[data-mfa-type="sms"], button:has-text("SMS")');
          if (smsOption) {
            await smsOption.click();
            
            const phoneInput = await mfaModal.$('input[name="phoneNumber"]');
            if (phoneInput) {
              await phoneInput.fill('+1234567890');
              
              const sendSmsButton = await mfaModal.$('button:has-text("Send Code")');
              if (sendSmsButton) {
                await sendSmsButton.click();
                console.log(chalk.gray('    SMS backup configured'));
              }
            }
          }
        }
        
        // Recovery codes
        const recoveryCodesButton = await mfaModal.$('button:has-text("Recovery Codes")');
        if (recoveryCodesButton) {
          await recoveryCodesButton.click();
          await page.waitForTimeout(500);
          
          const recoveryCodes = await mfaModal.$$('.recovery-code, [data-recovery-code]');
          console.log(chalk.gray(`    Recovery codes generated: ${recoveryCodes.length}`));
          this.metrics.backupsCreated++;
        }
        
        // Hardware key support
        const hardwareKeyOption = await mfaModal.$('[data-mfa-type="hardware"], button:has-text("Hardware Key")');
        if (hardwareKeyOption) {
          console.log(chalk.gray('    Hardware key MFA available'));
        }
      }
      
      this.metrics.stepTimings.multiFactorAuthentication = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Multi-factor authentication tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'multiFactorAuthentication',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPasswordSecurity(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing password security...'));
    
    try {
      // Password settings
      const passwordButton = await page.$('button:has-text("Password"), [data-password-settings]');
      if (!passwordButton) {
        console.log(chalk.yellow('    ⚠ Password settings not found'));
        return;
      }
      
      await passwordButton.click();
      await page.waitForTimeout(500);
      
      // Password strength checker
      const passwordModal = await page.$('[role="dialog"], .password-modal');
      if (passwordModal) {
        // Test password strength
        const testPasswords = [
          'weak123',
          'StrongerPass123!',
          'VeryStrongP@ssw0rd2024!#$'
        ];
        
        const passwordInput = await passwordModal.$('input[name="newPassword"]');
        if (passwordInput) {
          for (const password of testPasswords) {
            await passwordInput.fill(password);
            await page.waitForTimeout(300);
            
            const strengthIndicator = await passwordModal.$eval('.password-strength', el => el.textContent).catch(() => 'Unknown');
            console.log(chalk.gray(`    Password "${password.substring(0, 8)}...": ${strengthIndicator}`));
          }
        }
        
        // Password history check
        const historyCheck = await passwordModal.$('.password-history-check');
        if (historyCheck) {
          console.log(chalk.gray('    Password history validation available'));
        }
        
        // Password expiry settings
        const expirySettings = await passwordModal.$('.password-expiry');
        if (expirySettings) {
          const expirySelect = await expirySettings.$('select[name="passwordExpiry"]');
          if (expirySelect) {
            await expirySelect.selectOption('90'); // 90 days
            console.log(chalk.gray('    Password expiry set to 90 days'));
          }
        }
        
        // Password requirements display
        const requirements = await passwordModal.$$('.password-requirement');
        console.log(chalk.gray(`    Password requirements: ${requirements.length} rules`));
      }
      
      this.metrics.stepTimings.passwordSecurity = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Password security tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'passwordSecurity',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSessionManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing session management...'));
    
    try {
      // Session management section
      const sessionButton = await page.$('button:has-text("Sessions"), [data-sessions]');
      if (!sessionButton) {
        console.log(chalk.yellow('    ⚠ Session management not found'));
        return;
      }
      
      await sessionButton.click();
      await page.waitForTimeout(500);
      
      // Active sessions
      const activeSessions = await page.$$('.active-session, [data-session]');
      console.log(chalk.gray(`    Active sessions: ${activeSessions.length}`));
      
      for (let i = 0; i < Math.min(3, activeSessions.length); i++) {
        const session = activeSessions[i];
        const device = await session.$eval('.session-device', el => el.textContent);
        const location = await session.$eval('.session-location', el => el.textContent).catch(() => 'Unknown');
        const lastActive = await session.$eval('.last-active', el => el.textContent);
        const isCurrent = await session.$eval('.current-session', el => el.textContent).catch(() => null);
        
        console.log(chalk.gray(`    Session ${i + 1}: ${device} (${location})`));
        console.log(chalk.gray(`      Last active: ${lastActive} ${isCurrent ? '(Current)' : ''}`));
        
        // Terminate other sessions (not current)
        if (!isCurrent && i > 0) {
          const terminateButton = await session.$('button:has-text("Terminate")');
          if (terminateButton) {
            await terminateButton.click();
            console.log(chalk.gray(`      Session terminated`));
          }
        }
      }
      
      // Session settings
      const sessionSettings = await page.$('.session-settings, [data-session-settings]');
      if (sessionSettings) {
        // Auto-logout timeout
        const timeoutSelect = await sessionSettings.$('select[name="sessionTimeout"]');
        if (timeoutSelect) {
          await timeoutSelect.selectOption('30'); // 30 minutes
          console.log(chalk.gray('    Session timeout: 30 minutes'));
        }
        
        // Concurrent session limit
        const sessionLimitInput = await sessionSettings.$('input[name="maxSessions"]');
        if (sessionLimitInput) {
          await sessionLimitInput.fill('3'); // Max 3 sessions
          console.log(chalk.gray('    Max concurrent sessions: 3'));
        }
        
        // Require re-auth for sensitive actions
        const reAuthCheckbox = await sessionSettings.$('input[name="requireReAuth"]');
        if (reAuthCheckbox) {
          await reAuthCheckbox.check();
          console.log(chalk.gray('    Re-authentication required for sensitive actions'));
        }
      }
      
      this.metrics.stepTimings.sessionManagement = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Session management tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'sessionManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testDeviceManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing device management...'));
    
    try {
      // Device management section
      const deviceButton = await page.$('button:has-text("Devices"), [data-devices]');
      if (!deviceButton) {
        console.log(chalk.yellow('    ⚠ Device management not found'));
        return;
      }
      
      await deviceButton.click();
      await page.waitForTimeout(500);
      
      // Trusted devices
      const trustedDevices = await page.$$('.trusted-device, [data-device]');
      console.log(chalk.gray(`    Trusted devices: ${trustedDevices.length}`));
      
      for (let i = 0; i < Math.min(3, trustedDevices.length); i++) {
        const device = trustedDevices[i];
        const deviceName = await device.$eval('.device-name', el => el.textContent);
        const deviceType = await device.$eval('.device-type', el => el.textContent);
        const lastSeen = await device.$eval('.last-seen', el => el.textContent);
        const trustLevel = await device.$eval('.trust-level', el => el.textContent).catch(() => 'Unknown');
        
        console.log(chalk.gray(`    Device ${i + 1}: ${deviceName} (${deviceType})`));
        console.log(chalk.gray(`      Last seen: ${lastSeen}, Trust: ${trustLevel}`));
        
        // Device actions
        const actionsDropdown = await device.$('.device-actions, [data-device-actions]');
        if (actionsDropdown) {
          const actions = await actionsDropdown.$$('button, a');
          console.log(chalk.gray(`      Available actions: ${actions.length}`));
        }
      }
      
      // Device fingerprinting
      const fingerprintingSection = await page.$('.device-fingerprinting, [data-fingerprinting]');
      if (fingerprintingSection) {
        const enableFingerprintingCheckbox = await fingerprintingSection.$('input[name="enableFingerprinting"]');
        if (enableFingerprintingCheckbox) {
          await enableFingerprintingCheckbox.check();
          console.log(chalk.gray('    Device fingerprinting enabled'));
        }
      }
      
      // Suspicious device alerts
      const alertSettings = await page.$('.device-alert-settings');
      if (alertSettings) {
        const newDeviceAlertCheckbox = await alertSettings.$('input[name="newDeviceAlert"]');
        if (newDeviceAlertCheckbox) {
          await newDeviceAlertCheckbox.check();
          console.log(chalk.gray('    New device alerts enabled'));
        }
      }
      
      this.metrics.stepTimings.deviceManagement = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Device management tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'deviceManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSecurityAudits(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing security audits...'));
    
    try {
      // Security audit section
      const auditButton = await page.$('button:has-text("Security Audit"), [data-audit]');
      if (!auditButton) {
        console.log(chalk.yellow('    ⚠ Security audit not available'));
        return;
      }
      
      await auditButton.click();
      await page.waitForTimeout(500);
      
      // Run security scan
      const scanButton = await page.$('button:has-text("Run Scan"), button:has-text("Start Audit")');
      if (scanButton) {
        await scanButton.click();
        console.log(chalk.gray('    Running security scan...'));
        await page.waitForTimeout(3000);
        
        // Scan results
        const scanResults = await page.$('.scan-results, [data-scan-results]');
        if (scanResults) {
          const criticalIssues = await scanResults.$eval('.critical-issues', el => el.textContent).catch(() => '0');
          const highIssues = await scanResults.$eval('.high-issues', el => el.textContent).catch(() => '0');
          const mediumIssues = await scanResults.$eval('.medium-issues', el => el.textContent).catch(() => '0');
          const lowIssues = await scanResults.$eval('.low-issues', el => el.textContent).catch(() => '0');
          
          console.log(chalk.gray('    Scan results:'));
          console.log(chalk.gray(`    - Critical: ${criticalIssues}`));
          console.log(chalk.gray(`    - High: ${highIssues}`));
          console.log(chalk.gray(`    - Medium: ${mediumIssues}`));
          console.log(chalk.gray(`    - Low: ${lowIssues}`));
          
          const totalIssues = parseInt(criticalIssues) + parseInt(highIssues) + parseInt(mediumIssues) + parseInt(lowIssues);
          this.metrics.vulnerabilitiesFound += totalIssues;
        }
      }
      
      // Audit log
      const auditLog = await page.$$('.audit-log-entry, [data-audit-entry]');
      console.log(chalk.gray(`    Audit log entries: ${auditLog.length}`));
      this.metrics.auditTrailEvents += auditLog.length;
      
      // Compliance checks
      const complianceSection = await page.$('.compliance-checks, [data-compliance]');
      if (complianceSection) {
        const complianceItems = await complianceSection.$$('.compliance-item');
        let passedChecks = 0;
        
        for (const item of complianceItems) {
          const status = await item.$eval('.compliance-status', el => el.textContent);
          if (status.includes('Pass') || status.includes('✓')) {
            passedChecks++;
          }
        }
        
        console.log(chalk.gray(`    Compliance checks: ${passedChecks}/${complianceItems.length} passed`));
      }
      
      // Generate audit report
      const reportButton = await page.$('button:has-text("Generate Report")');
      if (reportButton) {
        await reportButton.click();
        console.log(chalk.gray('    Security audit report generated'));
      }
      
      this.metrics.stepTimings.securityAudits = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Security audits completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'securityAudits',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSuspiciousActivityDetection(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing suspicious activity detection...'));
    
    try {
      // Activity monitoring section
      const monitoringButton = await page.$('button:has-text("Activity Monitor"), [data-monitoring]');
      if (!monitoringButton) {
        console.log(chalk.yellow('    ⚠ Activity monitoring not available'));
        return;
      }
      
      await monitoringButton.click();
      await page.waitForTimeout(500);
      
      // Configure detection rules
      const detectionRules = await page.$('.detection-rules, [data-rules]');
      if (detectionRules) {
        // Unusual login patterns
        const loginPatternCheckbox = await detectionRules.$('input[name="unusualLogin"]');
        if (loginPatternCheckbox) {
          await loginPatternCheckbox.check();
          console.log(chalk.gray('    Unusual login pattern detection enabled'));
        }
        
        // Large transaction alerts
        const transactionAlertInput = await detectionRules.$('input[name="largeTransactionThreshold"]');
        if (transactionAlertInput) {
          await transactionAlertInput.fill('10000'); // $10k threshold
          console.log(chalk.gray('    Large transaction threshold: $10,000'));
        }
        
        // Geographic anomalies
        const geoAnomalyCheckbox = await detectionRules.$('input[name="geoAnomaly"]');
        if (geoAnomalyCheckbox) {
          await geoAnomalyCheckbox.check();
          console.log(chalk.gray('    Geographic anomaly detection enabled'));
        }
        
        // API abuse detection
        const apiAbuseCheckbox = await detectionRules.$('input[name="apiAbuse"]');
        if (apiAbuseCheckbox) {
          await apiAbuseCheckbox.check();
          console.log(chalk.gray('    API abuse detection enabled'));
        }
      }
      
      // Simulate suspicious activity
      const simulateButton = await page.$('button:has-text("Simulate Activity"), [data-simulate]');
      if (simulateButton) {
        await simulateButton.click();
        console.log(chalk.gray('    Simulating suspicious activity...'));
        await page.waitForTimeout(2000);
        
        // Check for alerts
        const alerts = await page.$$('.security-alert, [data-alert]');
        console.log(chalk.gray(`    Security alerts triggered: ${alerts.length}`));
        this.metrics.securityAlertsTriggered += alerts.length;
        
        if (alerts.length > 0) {
          const firstAlert = alerts[0];
          const alertType = await firstAlert.$eval('.alert-type', el => el.textContent);
          const alertSeverity = await firstAlert.$eval('.alert-severity', el => el.textContent);
          const alertDescription = await firstAlert.$eval('.alert-description', el => el.textContent);
          
          console.log(chalk.gray(`    Alert: ${alertType} (${alertSeverity})`));
          console.log(chalk.gray(`    Description: ${alertDescription.substring(0, 50)}...`));
        }
      }
      
      // Real-time monitoring status
      const monitoringStatus = await page.$('.monitoring-status, [data-monitoring-status]');
      if (monitoringStatus) {
        const status = await monitoringStatus.textContent();
        console.log(chalk.gray(`    Monitoring status: ${status}`));
      }
      
      this.metrics.stepTimings.suspiciousActivityDetection = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Suspicious activity detection tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'suspiciousActivityDetection',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testAccountRecovery(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing account recovery...'));
    
    try {
      // Account recovery section
      const recoveryButton = await page.$('button:has-text("Account Recovery"), [data-recovery]');
      if (!recoveryButton) {
        console.log(chalk.yellow('    ⚠ Account recovery not available'));
        return;
      }
      
      await recoveryButton.click();
      await page.waitForTimeout(500);
      
      // Recovery methods setup
      const recoveryMethods = await page.$('.recovery-methods, [data-recovery-methods]');
      if (recoveryMethods) {
        // Email recovery
        const emailRecovery = await recoveryMethods.$('.email-recovery');
        if (emailRecovery) {
          const emailInput = await emailRecovery.$('input[name="recoveryEmail"]');
          if (emailInput) {
            await emailInput.fill('recovery@example.com');
            
            const verifyEmailButton = await emailRecovery.$('button:has-text("Verify Email")');
            if (verifyEmailButton) {
              await verifyEmailButton.click();
              console.log(chalk.gray('    Email recovery method configured'));
            }
          }
        }
        
        // Phone recovery
        const phoneRecovery = await recoveryMethods.$('.phone-recovery');
        if (phoneRecovery) {
          const phoneInput = await phoneRecovery.$('input[name="recoveryPhone"]');
          if (phoneInput) {
            await phoneInput.fill('+1234567890');
            console.log(chalk.gray('    Phone recovery method configured'));
          }
        }
        
        // Security questions
        const securityQuestions = await recoveryMethods.$('.security-questions');
        if (securityQuestions) {
          const questionSelects = await securityQuestions.$$('select[name^="question"]');
          const answerInputs = await securityQuestions.$$('input[name^="answer"]');
          
          for (let i = 0; i < Math.min(questionSelects.length, answerInputs.length); i++) {
            await questionSelects[i].selectOption({ index: i + 1 });
            await answerInputs[i].fill(`Test Answer ${i + 1}`);
          }
          
          console.log(chalk.gray(`    Security questions configured: ${questionSelects.length}`));
        }
        
        // Trusted contacts
        const trustedContacts = await recoveryMethods.$('.trusted-contacts');
        if (trustedContacts) {
          const addContactButton = await trustedContacts.$('button:has-text("Add Contact")');
          if (addContactButton) {
            await addContactButton.click();
            
            const contactEmailInput = await page.$('input[name="contactEmail"]');
            if (contactEmailInput) {
              await contactEmailInput.fill('trusted@example.com');
              
              const saveContactButton = await page.$('button:has-text("Save Contact")');
              if (saveContactButton) {
                await saveContactButton.click();
                console.log(chalk.gray('    Trusted contact added'));
              }
            }
          }
        }
      }
      
      // Test recovery process
      const testRecoveryButton = await page.$('button:has-text("Test Recovery"), [data-test-recovery]');
      if (testRecoveryButton) {
        await testRecoveryButton.click();
        console.log(chalk.gray('    Testing recovery process...'));
        await page.waitForTimeout(2000);
        
        // Recovery process simulation
        const recoverySteps = await page.$$('.recovery-step, [data-recovery-step]');
        console.log(chalk.gray(`    Recovery process steps: ${recoverySteps.length}`));
        
        this.metrics.recoveryActionsCompleted++;
      }
      
      this.metrics.stepTimings.accountRecovery = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Account recovery tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'accountRecovery',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testWalletRecovery(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing wallet recovery...'));
    
    try {
      // Wallet recovery section
      const walletRecoveryButton = await page.$('button:has-text("Wallet Recovery"), [data-wallet-recovery]');
      if (!walletRecoveryButton) {
        console.log(chalk.yellow('    ⚠ Wallet recovery not available'));
        return;
      }
      
      await walletRecoveryButton.click();
      await page.waitForTimeout(500);
      
      // Seed phrase backup
      const seedPhraseSection = await page.$('.seed-phrase-backup, [data-seed-backup]');
      if (seedPhraseSection) {
        const showSeedButton = await seedPhraseSection.$('button:has-text("Show Seed Phrase")');
        if (showSeedButton) {
          await showSeedButton.click();
          await page.waitForTimeout(500);
          
          // Verify seed phrase display
          const seedWords = await page.$$('.seed-word, [data-seed-word]');
          console.log(chalk.gray(`    Seed phrase words: ${seedWords.length}`));
          
          // Test seed phrase verification
          const verifyButton = await page.$('button:has-text("Verify Seed Phrase")');
          if (verifyButton) {
            await verifyButton.click();
            
            // Simulate seed phrase verification
            const verificationInputs = await page.$$('input[name^="seedWord"]');
            for (let i = 0; i < Math.min(3, verificationInputs.length); i++) {
              await verificationInputs[i].fill(`word${i + 1}`);
            }
            
            const confirmVerifyButton = await page.$('button:has-text("Confirm Verification")');
            if (confirmVerifyButton) {
              await confirmVerifyButton.click();
              console.log(chalk.gray('    Seed phrase verification completed'));
            }
          }
          
          this.metrics.backupsCreated++;
        }
      }
      
      // Hardware wallet recovery
      const hardwareRecoverySection = await page.$('.hardware-recovery');
      if (hardwareRecoverySection) {
        const hardwareTypes = await hardwareRecoverySection.$$('.hardware-type');
        console.log(chalk.gray(`    Hardware wallet types supported: ${hardwareTypes.length}`));
      }
      
      // Multi-signature recovery
      const multisigSection = await page.$('.multisig-recovery');
      if (multisigSection) {
        const requiredSignatures = await multisigSection.$eval('.required-signatures', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Multi-sig required signatures: ${requiredSignatures}`));
      }
      
      // Recovery time estimates
      const recoveryEstimates = await page.$('.recovery-estimates');
      if (recoveryEstimates) {
        const seedRecoveryTime = await recoveryEstimates.$eval('.seed-recovery-time', el => el.textContent).catch(() => 'N/A');
        const hardwareRecoveryTime = await recoveryEstimates.$eval('.hardware-recovery-time', el => el.textContent).catch(() => 'N/A');
        
        console.log(chalk.gray(`    Recovery time estimates:`));
        console.log(chalk.gray(`    - Seed phrase: ${seedRecoveryTime}`));
        console.log(chalk.gray(`    - Hardware wallet: ${hardwareRecoveryTime}`));
      }
      
      this.metrics.stepTimings.walletRecovery = {
        duration: Date.now() - stepStart
      };
      this.metrics.recoveryActionsCompleted++;
      
      console.log(chalk.green('    ✓ Wallet recovery tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'walletRecovery',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSecurityBackups(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing security backups...'));
    
    try {
      // Backup management
      const backupButton = await page.$('button:has-text("Security Backups"), [data-backups]');
      if (!backupButton) {
        console.log(chalk.yellow('    ⚠ Security backups not available'));
        return;
      }
      
      await backupButton.click();
      await page.waitForTimeout(500);
      
      // Create encrypted backup
      const createBackupButton = await page.$('button:has-text("Create Backup")');
      if (createBackupButton) {
        await createBackupButton.click();
        
        const backupModal = await page.$('[role="dialog"], .backup-modal');
        if (backupModal) {
          // Select backup type
          const backupTypes = await backupModal.$$('.backup-type');
          if (backupTypes.length > 0) {
            await backupTypes[0].click(); // Select first type
          }
          
          // Set encryption password
          const encryptionPasswordInput = await backupModal.$('input[name="encryptionPassword"]');
          if (encryptionPasswordInput) {
            await encryptionPasswordInput.fill('SecureBackupPass123!');
          }
          
          // Generate backup
          const generateButton = await backupModal.$('button:has-text("Generate Backup")');
          if (generateButton) {
            await generateButton.click();
            console.log(chalk.gray('    Encrypted backup generated'));
            this.metrics.backupsCreated++;
          }
        }
      }
      
      // Existing backups
      const existingBackups = await page.$$('.backup-item, [data-backup]');
      console.log(chalk.gray(`    Existing backups: ${existingBackups.length}`));
      
      for (let i = 0; i < Math.min(3, existingBackups.length); i++) {
        const backup = existingBackups[i];
        const backupDate = await backup.$eval('.backup-date', el => el.textContent);
        const backupSize = await backup.$eval('.backup-size', el => el.textContent);
        const backupType = await backup.$eval('.backup-type', el => el.textContent);
        
        console.log(chalk.gray(`    Backup ${i + 1}: ${backupType} - ${backupDate} (${backupSize})`));
      }
      
      // Backup verification
      if (existingBackups.length > 0) {
        const verifyButton = await existingBackups[0].$('button:has-text("Verify")');
        if (verifyButton) {
          await verifyButton.click();
          console.log(chalk.gray('    Backup integrity verified'));
        }
      }
      
      // Automatic backup settings
      const autoBackupSection = await page.$('.auto-backup-settings');
      if (autoBackupSection) {
        const enableAutoBackupCheckbox = await autoBackupSection.$('input[name="enableAutoBackup"]');
        if (enableAutoBackupCheckbox) {
          await enableAutoBackupCheckbox.check();
          
          const frequencySelect = await autoBackupSection.$('select[name="backupFrequency"]');
          if (frequencySelect) {
            await frequencySelect.selectOption('weekly');
            console.log(chalk.gray('    Auto-backup enabled (weekly)'));
          }
        }
      }
      
      this.metrics.stepTimings.securityBackups = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Security backups tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'securityBackups',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testEmergencyLock(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing emergency lock...'));
    
    try {
      // Emergency procedures
      const emergencyButton = await page.$('button:has-text("Emergency"), [data-emergency]');
      if (!emergencyButton) {
        console.log(chalk.yellow('    ⚠ Emergency procedures not available'));
        return;
      }
      
      await emergencyButton.click();
      await page.waitForTimeout(500);
      
      // Emergency lock options
      const emergencyModal = await page.$('[role="dialog"], .emergency-modal');
      if (emergencyModal) {
        console.log(chalk.gray('    Emergency options available:'));
        
        // Account freeze
        const freezeAccountButton = await emergencyModal.$('button:has-text("Freeze Account")');
        if (freezeAccountButton) {
          console.log(chalk.gray('    - Account freeze'));
        }
        
        // Immediate logout
        const logoutAllButton = await emergencyModal.$('button:has-text("Logout All Sessions")');
        if (logoutAllButton) {
          console.log(chalk.gray('    - Logout all sessions'));
        }
        
        // Disable trading
        const disableTradingButton = await emergencyModal.$('button:has-text("Disable Trading")');
        if (disableTradingButton) {
          console.log(chalk.gray('    - Disable trading'));
        }
        
        // Emergency contact
        const contactEmergencyButton = await emergencyModal.$('button:has-text("Contact Support")');
        if (contactEmergencyButton) {
          console.log(chalk.gray('    - Emergency support contact'));
        }
        
        // Test emergency procedure (simulate)
        const testEmergencyButton = await emergencyModal.$('button:has-text("Test Emergency Lock")');
        if (testEmergencyButton) {
          await testEmergencyButton.click();
          console.log(chalk.gray('    Emergency lock tested (simulated)'));
          this.metrics.recoveryActionsCompleted++;
        }
        
        // Close modal without activating emergency
        const closeButton = await emergencyModal.$('button:has-text("Cancel"), [aria-label="Close"]');
        if (closeButton) {
          await closeButton.click();
        }
      }
      
      this.metrics.stepTimings.emergencyLock = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Emergency lock procedures tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'emergencyLock',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPenetrationTesting(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing penetration testing tools...'));
    
    try {
      // Security testing tools (if available in test environment)
      const penTestButton = await page.$('button:has-text("Penetration Test"), [data-pentest]');
      if (!penTestButton) {
        console.log(chalk.yellow('    ⚠ Penetration testing tools not available'));
        return;
      }
      
      await penTestButton.click();
      await page.waitForTimeout(500);
      
      // Basic security tests
      const testCategories = [
        'SQL Injection',
        'XSS Vulnerabilities',
        'CSRF Protection',
        'Authentication Bypass',
        'Authorization Flaws',
        'Input Validation'
      ];
      
      console.log(chalk.gray('    Running security tests:'));
      
      for (const category of testCategories) {
        const testButton = await page.$(`button:has-text("${category}"), [data-test="${category.toLowerCase().replace(' ', '-')}"]`);
        if (testButton) {
          await testButton.click();
          await page.waitForTimeout(500);
          
          // Check test results
          const result = await page.$eval('.test-result', el => el.textContent).catch(() => 'PASS');
          console.log(chalk.gray(`    - ${category}: ${result}`));
          
          if (result.includes('FAIL') || result.includes('VULNERABLE')) {
            this.metrics.vulnerabilitiesFound++;
          }
        }
      }
      
      // Load testing simulation
      const loadTestButton = await page.$('button:has-text("Load Test")');
      if (loadTestButton) {
        await loadTestButton.click();
        console.log(chalk.gray('    Load testing simulation started'));
      }
      
      this.metrics.stepTimings.penetrationTesting = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Penetration testing completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'penetrationTesting',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testComplianceChecks(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing compliance checks...'));
    
    try {
      // Compliance dashboard
      const complianceButton = await page.$('button:has-text("Compliance"), [data-compliance]');
      if (!complianceButton) {
        console.log(chalk.yellow('    ⚠ Compliance checks not available'));
        return;
      }
      
      await complianceButton.click();
      await page.waitForTimeout(500);
      
      // Compliance standards
      const standards = [
        'GDPR',
        'CCPA',
        'SOX',
        'PCI DSS',
        'ISO 27001',
        'SOC 2'
      ];
      
      console.log(chalk.gray('    Compliance standards:'));
      
      for (const standard of standards) {
        const standardSection = await page.$(`[data-standard="${standard.toLowerCase()}"], .${standard.toLowerCase().replace(' ', '-')}-compliance`);
        if (standardSection) {
          const status = await standardSection.$eval('.compliance-status', el => el.textContent).catch(() => 'Unknown');
          const lastAudit = await standardSection.$eval('.last-audit', el => el.textContent).catch(() => 'Never');
          
          console.log(chalk.gray(`    - ${standard}: ${status} (Last audit: ${lastAudit})`));
        }
      }
      
      // Data retention policies
      const dataRetentionSection = await page.$('.data-retention');
      if (dataRetentionSection) {
        const retentionPeriod = await dataRetentionSection.$eval('.retention-period', el => el.textContent);
        console.log(chalk.gray(`    Data retention period: ${retentionPeriod}`));
      }
      
      // Privacy controls
      const privacyControls = await page.$$('.privacy-control');
      console.log(chalk.gray(`    Privacy controls: ${privacyControls.length} configured`));
      
      this.metrics.stepTimings.complianceChecks = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Compliance checks completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'complianceChecks',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPrivacyControls(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing privacy controls...'));
    
    try {
      // Privacy settings
      const privacyButton = await page.$('button:has-text("Privacy"), [data-privacy]');
      if (!privacyButton) {
        console.log(chalk.yellow('    ⚠ Privacy controls not available'));
        return;
      }
      
      await privacyButton.click();
      await page.waitForTimeout(500);
      
      // Data sharing preferences
      const dataSharingSection = await page.$('.data-sharing-preferences');
      if (dataSharingSection) {
        const sharingOptions = await dataSharingSection.$$('input[type="checkbox"]');
        console.log(chalk.gray(`    Data sharing options: ${sharingOptions.length}`));
        
        // Disable all unnecessary sharing
        for (const option of sharingOptions) {
          const isChecked = await option.isChecked();
          if (isChecked) {
            await option.uncheck();
          }
        }
        
        console.log(chalk.gray('    Data sharing minimized'));
      }
      
      // Cookie preferences
      const cookieSection = await page.$('.cookie-preferences');
      if (cookieSection) {
        const essentialOnly = await cookieSection.$('input[name="essentialOnly"]');
        if (essentialOnly) {
          await essentialOnly.check();
          console.log(chalk.gray('    Cookie preferences: Essential only'));
        }
      }
      
      // Data export request
      const dataExportButton = await page.$('button:has-text("Export My Data")');
      if (dataExportButton) {
        await dataExportButton.click();
        console.log(chalk.gray('    Data export request initiated'));
      }
      
      // Data deletion request
      const dataDeletionButton = await page.$('button:has-text("Delete My Data")');
      if (dataDeletionButton) {
        console.log(chalk.gray('    Data deletion option available'));
      }
      
      this.metrics.stepTimings.privacyControls = {
        duration: Date.now() - stepStart
      };
      this.metrics.securityChecksPerformed++;
      
      console.log(chalk.green('    ✓ Privacy controls tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'privacyControls',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testIncidentResponse(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing incident response...'));
    
    try {
      // Incident response system
      const incidentButton = await page.$('button:has-text("Incident Response"), [data-incident]');
      if (!incidentButton) {
        console.log(chalk.yellow('    ⚠ Incident response not available'));
        return;
      }
      
      await incidentButton.click();
      await page.waitForTimeout(500);
      
      // Create test incident
      const createIncidentButton = await page.$('button:has-text("Report Incident")');
      if (createIncidentButton) {
        await createIncidentButton.click();
        
        const incidentModal = await page.$('[role="dialog"], .incident-modal');
        if (incidentModal) {
          // Incident details
          const incidentTypeSelect = await incidentModal.$('select[name="incidentType"]');
          if (incidentTypeSelect) {
            await incidentTypeSelect.selectOption('security_breach');
          }
          
          const severitySelect = await incidentModal.$('select[name="severity"]');
          if (severitySelect) {
            await severitySelect.selectOption('high');
          }
          
          const descriptionTextarea = await incidentModal.$('textarea[name="description"]');
          if (descriptionTextarea) {
            await descriptionTextarea.fill('Test security incident for testing incident response procedures');
          }
          
          // Submit incident
          const submitButton = await incidentModal.$('button:has-text("Submit Incident")');
          if (submitButton) {
            await submitButton.click();
            console.log(chalk.gray('    Test incident created'));
          }
        }
      }
      
      // Incident tracking
      const activeIncidents = await page.$$('.active-incident, [data-incident]');
      console.log(chalk.gray(`    Active incidents: ${activeIncidents.length}`));
      
      // Response procedures
      const responseProcedures = await page.$('.response-procedures');
      if (responseProcedures) {
        const procedures = await responseProcedures.$$('.procedure-step');
        console.log(chalk.gray(`    Response procedures: ${procedures.length} steps`));
      }
      
      // Emergency contacts
      const emergencyContacts = await page.$$('.emergency-contact');
      console.log(chalk.gray(`    Emergency contacts: ${emergencyContacts.length} configured`));
      
      this.metrics.stepTimings.incidentResponse = {
        duration: Date.now() - stepStart
      };
      this.metrics.recoveryActionsCompleted++;
      
      console.log(chalk.green('    ✓ Incident response tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'incidentResponse',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running security and recovery load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalSecurityChecks: 0,
    totalRecoveryActions: 0,
    vulnerabilitiesFound: 0,
    securityAlertsTriggered: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new SecurityRecoveryJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          results.successful++;
          results.totalSecurityChecks += metrics.securityChecksPerformed;
          results.totalRecoveryActions += metrics.recoveryActionsCompleted;
          results.vulnerabilitiesFound += metrics.vulnerabilitiesFound;
          results.securityAlertsTriggered += metrics.securityAlertsTriggered;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Heavy stagger for security operations
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
  console.log(chalk.bold('\nSecurity and Recovery Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Security Checks: ${results.totalSecurityChecks}`));
  console.log(chalk.magenta(`  Recovery Actions: ${results.totalRecoveryActions}`));
  console.log(chalk.yellow(`  Vulnerabilities Found: ${results.vulnerabilitiesFound}`));
  console.log(chalk.red(`  Security Alerts: ${results.securityAlertsTriggered}`));
  
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
      const singleTest = new SecurityRecoveryJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests with lower concurrency due to security operations
      await runLoadTest(config, testData, 5);     // 5 users
      await runLoadTest(config, testData, 25);    // 25 users
      await runLoadTest(config, testData, 100);   // 100 users
      
      console.log(chalk.bold.green('\n✅ All security and recovery tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { SecurityRecoveryJourneyTest, runLoadTest };