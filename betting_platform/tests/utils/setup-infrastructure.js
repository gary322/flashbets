#!/usr/bin/env node

/**
 * Setup test infrastructure for comprehensive testing
 * - Fresh Solana validator
 * - Deploy contracts
 * - Initialize program
 * - Start services
 */

const { spawn, exec } = require('child_process');
const { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL } = require('@solana/web3.js');
const fs = require('fs');
const path = require('path');
const chalk = require('chalk');
const ora = require('ora');

const TEST_CONFIG = {
  rpcUrl: 'http://localhost:8899',
  programId: null, // Will be set after deployment
  globalConfigSeed: 42,
  services: {
    validator: null,
    api: null,
    ui: null
  }
};

class TestInfrastructure {
  constructor() {
    this.connection = null;
    this.adminKeypair = null;
    this.programKeypair = null;
  }

  async setup() {
    console.log(chalk.bold.blue('🚀 Setting up test infrastructure...\n'));
    
    try {
      await this.killExistingServices();
      await this.startSolanaValidator();
      await this.deployContracts();
      await this.initializeProgram();
      await this.startAPIBackend();
      await this.startUIFrontend();
      await this.verifySetup();
      
      console.log(chalk.bold.green('\n✅ Test infrastructure setup complete!'));
      this.saveConfig();
    } catch (error) {
      console.error(chalk.red('❌ Setup failed:'), error);
      await this.cleanup();
      process.exit(1);
    }
  }

  async killExistingServices() {
    const spinner = ora('Killing existing services...').start();
    
    try {
      // Kill existing Solana validator
      await this.execCommand('pkill -f solana-test-validator || true');
      
      // Kill existing API server
      await this.execCommand('lsof -ti:8081 | xargs kill -9 || true');
      
      // Kill existing UI server
      await this.execCommand('lsof -ti:3000 | xargs kill -9 || true');
      
      // Wait for ports to be freed
      await new Promise(resolve => setTimeout(resolve, 2000));
      
      spinner.succeed('Existing services killed');
    } catch (error) {
      spinner.fail('Failed to kill services');
      throw error;
    }
  }

  async startSolanaValidator() {
    const spinner = ora('Starting Solana test validator...').start();
    
    try {
      // Start validator with specific settings for testing
      TEST_CONFIG.services.validator = spawn('solana-test-validator', [
        '--reset',
        '--quiet',
        '--ticks-per-slot', '8',
        '--slots-per-epoch', '32'
      ], {
        detached: true,
        stdio: 'ignore'
      });
      
      TEST_CONFIG.services.validator.unref();
      
      // Wait for validator to be ready
      this.connection = new Connection(TEST_CONFIG.rpcUrl, 'confirmed');
      
      let retries = 30;
      while (retries > 0) {
        try {
          await this.connection.getVersion();
          break;
        } catch (e) {
          retries--;
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      }
      
      if (retries === 0) {
        throw new Error('Validator failed to start');
      }
      
      spinner.succeed('Solana validator started');
    } catch (error) {
      spinner.fail('Failed to start validator');
      throw error;
    }
  }

  async deployContracts() {
    const spinner = ora('Deploying smart contracts...').start();
    
    try {
      // Generate program keypair
      this.programKeypair = Keypair.generate();
      const programKeypairPath = path.join(__dirname, 'program-keypair.json');
      fs.writeFileSync(
        programKeypairPath,
        JSON.stringify(Array.from(this.programKeypair.secretKey))
      );
      
      // Build contract
      spinner.text = 'Building contract...';
      const contractPath = path.join(__dirname, '../../programs/betting_platform_native');
      await this.execCommand(`cd ${contractPath} && cargo build-sbf`);
      
      // Deploy contract
      spinner.text = 'Deploying contract...';
      const soPath = path.join(contractPath, 'target/deploy/betting_platform_native.so');
      const deployResult = await this.execCommand(
        `solana program deploy --program-id ${programKeypairPath} ${soPath}`
      );
      
      TEST_CONFIG.programId = this.programKeypair.publicKey.toBase58();
      
      spinner.succeed(`Contract deployed: ${TEST_CONFIG.programId}`);
    } catch (error) {
      spinner.fail('Failed to deploy contracts');
      throw error;
    }
  }

  async initializeProgram() {
    const spinner = ora('Initializing program...').start();
    
    try {
      // Create admin keypair
      this.adminKeypair = Keypair.generate();
      
      // Airdrop SOL to admin
      const airdropSig = await this.connection.requestAirdrop(
        this.adminKeypair.publicKey,
        10 * LAMPORTS_PER_SOL
      );
      await this.connection.confirmTransaction(airdropSig);
      
      // Initialize program using the initialization script
      const initScript = `
        const { Connection, PublicKey, Keypair, Transaction, SystemProgram, SYSVAR_RENT_PUBKEY, sendAndConfirmTransaction } = require('@solana/web3.js');
        
        const PROGRAM_ID = new PublicKey('${TEST_CONFIG.programId}');
        const connection = new Connection('${TEST_CONFIG.rpcUrl}', 'confirmed');
        const adminKeypair = Keypair.fromSecretKey(new Uint8Array(${JSON.stringify(Array.from(this.adminKeypair.secretKey))}));
        
        async function initialize() {
          const seed = BigInt(${TEST_CONFIG.globalConfigSeed});
          const seedBuffer = Buffer.alloc(16);
          seedBuffer.writeBigUInt64LE(seed & BigInt('0xFFFFFFFFFFFFFFFF'), 0);
          seedBuffer.writeBigUInt64LE((seed >> BigInt(64)) & BigInt('0xFFFFFFFFFFFFFFFF'), 8);
          
          const [globalConfigPDA] = PublicKey.findProgramAddressSync(
            [Buffer.from('global_config'), seedBuffer],
            PROGRAM_ID
          );
          
          const instructionData = Buffer.alloc(17);
          instructionData.writeUInt8(0, 0);
          seedBuffer.copy(instructionData, 1);
          
          const initializeIx = {
            programId: PROGRAM_ID,
            keys: [
              { pubkey: globalConfigPDA, isSigner: false, isWritable: true },
              { pubkey: adminKeypair.publicKey, isSigner: true, isWritable: true },
              { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
              { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
            ],
            data: instructionData,
          };
          
          const tx = new Transaction().add(initializeIx);
          const sig = await sendAndConfirmTransaction(connection, tx, [adminKeypair]);
          console.log('Initialized:', sig);
        }
        
        initialize().catch(console.error);
      `;
      
      fs.writeFileSync(path.join(__dirname, 'temp-init.js'), initScript);
      await this.execCommand(`node ${path.join(__dirname, 'temp-init.js')}`);
      fs.unlinkSync(path.join(__dirname, 'temp-init.js'));
      
      spinner.succeed('Program initialized');
    } catch (error) {
      spinner.fail('Failed to initialize program');
      throw error;
    }
  }

  async startAPIBackend() {
    const spinner = ora('Starting API backend...').start();
    
    try {
      // Update .env with test program ID
      const envPath = path.join(__dirname, '../../api_runner/.env');
      let envContent = fs.readFileSync(envPath, 'utf8');
      envContent = envContent.replace(/PROGRAM_ID=.*/, `PROGRAM_ID=${TEST_CONFIG.programId}`);
      fs.writeFileSync(envPath, envContent);
      
      // Start API server
      TEST_CONFIG.services.api = spawn('cargo', ['run'], {
        cwd: path.join(__dirname, '../../api_runner'),
        detached: true,
        stdio: 'ignore',
        env: { ...process.env, RUST_LOG: 'info' }
      });
      
      TEST_CONFIG.services.api.unref();
      
      // Wait for API to be ready
      let retries = 60;
      while (retries > 0) {
        try {
          const response = await fetch('http://localhost:8081/health');
          if (response.ok) break;
        } catch (e) {
          // Continue waiting
        }
        retries--;
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      
      if (retries === 0) {
        throw new Error('API failed to start');
      }
      
      spinner.succeed('API backend started on port 8081');
    } catch (error) {
      spinner.fail('Failed to start API backend');
      throw error;
    }
  }

  async startUIFrontend() {
    const spinner = ora('Starting UI frontend...').start();
    
    try {
      // Start UI server
      TEST_CONFIG.services.ui = spawn('npm', ['run', 'dev'], {
        cwd: path.join(__dirname, '../../app'),
        detached: true,
        stdio: 'ignore'
      });
      
      TEST_CONFIG.services.ui.unref();
      
      // Wait for UI to be ready
      let retries = 60;
      while (retries > 0) {
        try {
          const response = await fetch('http://localhost:3000');
          if (response.ok) break;
        } catch (e) {
          // Continue waiting
        }
        retries--;
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      
      if (retries === 0) {
        throw new Error('UI failed to start');
      }
      
      spinner.succeed('UI frontend started on port 3000');
    } catch (error) {
      spinner.fail('Failed to start UI frontend');
      throw error;
    }
  }

  async verifySetup() {
    const spinner = ora('Verifying setup...').start();
    
    try {
      // Check Solana
      const version = await this.connection.getVersion();
      console.log(chalk.gray(`  Solana version: ${version['solana-core']}`));
      
      // Check API
      const apiHealth = await fetch('http://localhost:8081/health');
      const apiData = await apiHealth.json();
      console.log(chalk.gray(`  API status: ${apiData.status}`));
      
      // Check UI
      const uiResponse = await fetch('http://localhost:3000');
      console.log(chalk.gray(`  UI status: ${uiResponse.ok ? 'running' : 'error'}`));
      
      spinner.succeed('All services verified');
    } catch (error) {
      spinner.fail('Verification failed');
      throw error;
    }
  }

  saveConfig() {
    const configPath = path.join(__dirname, '../test-config.json');
    fs.writeFileSync(configPath, JSON.stringify({
      programId: TEST_CONFIG.programId,
      rpcUrl: TEST_CONFIG.rpcUrl,
      apiUrl: 'http://localhost:8081',
      uiUrl: 'http://localhost:3000',
      wsUrl: 'ws://localhost:8081/ws',
      adminKeypair: Array.from(this.adminKeypair.secretKey),
      globalConfigSeed: TEST_CONFIG.globalConfigSeed,
      setupTime: new Date().toISOString()
    }, null, 2));
    
    console.log(chalk.gray(`\nConfiguration saved to: ${configPath}`));
  }

  async cleanup() {
    console.log(chalk.yellow('\nCleaning up...'));
    
    if (TEST_CONFIG.services.validator) {
      TEST_CONFIG.services.validator.kill();
    }
    if (TEST_CONFIG.services.api) {
      TEST_CONFIG.services.api.kill();
    }
    if (TEST_CONFIG.services.ui) {
      TEST_CONFIG.services.ui.kill();
    }
  }

  execCommand(cmd) {
    return new Promise((resolve, reject) => {
      exec(cmd, (error, stdout, stderr) => {
        if (error) {
          reject(error);
        } else {
          resolve(stdout);
        }
      });
    });
  }
}

// Run setup
const infra = new TestInfrastructure();
infra.setup().catch(console.error);

// Handle cleanup on exit
process.on('SIGINT', async () => {
  await infra.cleanup();
  process.exit(0);
});

module.exports = { TestInfrastructure };