#!/usr/bin/env node

/**
 * Initialize the deployed betting platform program
 */

const {
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  sendAndConfirmTransaction,
} = require('@solana/web3.js');

const PROGRAM_ID = new PublicKey('73TxR5vPTtczjQUfw4yRE23kMEau3deDZd94brmb29Kj');
const RPC_URL = 'http://localhost:8899';

async function initialize() {
  console.log('🚀 Initializing Betting Platform Program...');
  console.log('Program ID:', PROGRAM_ID.toBase58());
  
  // Connect to local validator
  const connection = new Connection(RPC_URL, 'confirmed');
  
  // Create a new wallet for testing
  const payer = Keypair.generate();
  console.log('Test wallet:', payer.publicKey.toBase58());
  
  // Airdrop SOL to the wallet
  console.log('Requesting airdrop...');
  const airdropSig = await connection.requestAirdrop(payer.publicKey, 2 * 1e9); // 2 SOL
  await connection.confirmTransaction(airdropSig);
  console.log('Airdrop confirmed');
  
  // Create seed for PDA derivation
  const seed = BigInt(42);
  const seedBuffer = Buffer.alloc(16);
  seedBuffer.writeBigUInt64LE(seed & BigInt('0xFFFFFFFFFFFFFFFF'), 0);
  seedBuffer.writeBigUInt64LE((seed >> BigInt(64)) & BigInt('0xFFFFFFFFFFFFFFFF'), 8);
  
  // Derive PDAs with seed
  const [globalConfigPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from('global_config'), seedBuffer],
    PROGRAM_ID
  );
  console.log('Global Config PDA:', globalConfigPDA.toBase58());
  
  // Check if already initialized
  const accountInfo = await connection.getAccountInfo(globalConfigPDA);
  if (accountInfo) {
    console.log('✅ Program already initialized');
    return;
  }
  
  // Create initialize instruction
  // BettingPlatformInstruction::Initialize { seed: u128 }
  // Instruction discriminator 0 + u128 seed (16 bytes)
  const instructionData = Buffer.alloc(17);
  instructionData.writeUInt8(0, 0); // Initialize variant
  
  // Use the same seed buffer
  seedBuffer.copy(instructionData, 1);
  
  const initializeIx = {
    programId: PROGRAM_ID,
    keys: [
      { pubkey: globalConfigPDA, isSigner: false, isWritable: true },
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false }, // Rent sysvar
    ],
    data: instructionData,
  };
  
  // Create and send transaction
  const tx = new Transaction().add(initializeIx);
  
  try {
    const sig = await sendAndConfirmTransaction(connection, tx, [payer]);
    console.log('✅ Program initialized successfully!');
    console.log('Transaction signature:', sig);
  } catch (error) {
    console.error('❌ Failed to initialize:', error);
  }
}

initialize().catch(console.error);