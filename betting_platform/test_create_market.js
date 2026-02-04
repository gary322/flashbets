#!/usr/bin/env node

/**
 * Test creating a market on the deployed betting platform
 */

const {
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  SystemProgram,
  sendAndConfirmTransaction,
} = require('@solana/web3.js');

const PROGRAM_ID = new PublicKey('73TxR5vPTtczjQUfw4yRE23kMEau3deDZd94brmb29Kj');
const RPC_URL = 'http://localhost:8899';

async function createTestMarket() {
  console.log('🎯 Creating test market on betting platform...');
  
  // Connect to local validator
  const connection = new Connection(RPC_URL, 'confirmed');
  
  // Create test wallet
  const payer = Keypair.generate();
  console.log('Test wallet:', payer.publicKey.toBase58());
  
  // Airdrop SOL
  console.log('Requesting airdrop...');
  const airdropSig = await connection.requestAirdrop(payer.publicKey, 2 * 1e9);
  await connection.confirmTransaction(airdropSig);
  
  // Create a test verse first
  const verseId = BigInt(1);
  const verseSeed = Buffer.alloc(16);
  verseSeed.writeBigUInt64LE(verseId & BigInt('0xFFFFFFFFFFFFFFFF'), 0);
  verseSeed.writeBigUInt64LE((verseId >> BigInt(64)) & BigInt('0xFFFFFFFFFFFFFFFF'), 8);
  
  const [versePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from('verse'), verseSeed],
    PROGRAM_ID
  );
  
  console.log('Verse PDA:', versePDA.toBase58());
  
  // Create market ID
  const marketId = BigInt(Date.now());
  const marketSeed = Buffer.alloc(16);
  marketSeed.writeBigUInt64LE(marketId & BigInt('0xFFFFFFFFFFFFFFFF'), 0);
  marketSeed.writeBigUInt64LE((marketId >> BigInt(64)) & BigInt('0xFFFFFFFFFFFFFFFF'), 8);
  
  const [marketPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from('proposal'), marketSeed],
    PROGRAM_ID
  );
  
  console.log('Market PDA:', marketPDA.toBase58());
  console.log('Market ID:', marketId.toString());
  
  // Create verse first (CreateVerse instruction = variant 49)
  const createVerseData = Buffer.alloc(50);
  createVerseData.writeUInt8(49, 0); // CreateVerse variant
  
  // CreateVerseParams
  createVerseData.writeBigUInt64LE(verseId, 1); // verse_id
  createVerseData.writeBigUInt64LE(BigInt(0), 9); // parent_id option (0 = None)
  createVerseData.writeUInt8(0, 17); // no parent
  
  const createVerseIx = {
    programId: PROGRAM_ID,
    keys: [
      { pubkey: versePDA, isSigner: false, isWritable: true },
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: createVerseData,
  };
  
  try {
    console.log('Creating verse...');
    const verseTx = new Transaction().add(createVerseIx);
    const verseSig = await sendAndConfirmTransaction(connection, verseTx, [payer]);
    console.log('✅ Verse created:', verseSig);
    
    // Now create market (CreateMarket instruction = variant 44)
    const createMarketData = Buffer.alloc(200);
    createMarketData.writeUInt8(44, 0); // CreateMarket variant
    
    // CreateMarketParams structure
    let offset = 1;
    
    // market_id: u128
    createMarketData.writeBigUInt64LE(marketId & BigInt('0xFFFFFFFFFFFFFFFF'), offset);
    createMarketData.writeBigUInt64LE((marketId >> BigInt(64)) & BigInt('0xFFFFFFFFFFFFFFFF'), offset + 8);
    offset += 16;
    
    // verse_id: u128
    createMarketData.writeBigUInt64LE(verseId & BigInt('0xFFFFFFFFFFFFFFFF'), offset);
    createMarketData.writeBigUInt64LE((verseId >> BigInt(64)) & BigInt('0xFFFFFFFFFFFFFFFF'), offset + 8);
    offset += 16;
    
    // title: String (length + bytes)
    const title = "Will BTC reach $100k by EOY?";
    createMarketData.writeUInt32LE(title.length, offset);
    offset += 4;
    createMarketData.write(title, offset);
    offset += title.length;
    
    // description: String
    const description = "Test market for BTC price prediction";
    createMarketData.writeUInt32LE(description.length, offset);
    offset += 4;
    createMarketData.write(description, offset);
    offset += description.length;
    
    // outcomes: Vec<String>
    createMarketData.writeUInt32LE(2, offset); // 2 outcomes
    offset += 4;
    
    const outcome1 = "Yes";
    createMarketData.writeUInt32LE(outcome1.length, offset);
    offset += 4;
    createMarketData.write(outcome1, offset);
    offset += outcome1.length;
    
    const outcome2 = "No";
    createMarketData.writeUInt32LE(outcome2.length, offset);
    offset += 4;
    createMarketData.write(outcome2, offset);
    offset += outcome2.length;
    
    // settle_time: i64
    const settleTime = BigInt(Date.now() / 1000 + 86400 * 30); // 30 days from now
    createMarketData.writeBigInt64LE(settleTime, offset);
    offset += 8;
    
    // initial_liquidity: u64
    createMarketData.writeBigUInt64LE(BigInt(1000000), offset); // 1 USDC
    offset += 8;
    
    // amm_type: AMMType (0 = LMSR)
    createMarketData.writeUInt8(0, offset);
    offset += 1;
    
    // oracle_authority: Option<Pubkey> (None)
    createMarketData.writeUInt8(0, offset); // None
    offset += 1;
    
    // b_parameter: Option<u64> (Some)
    createMarketData.writeUInt8(1, offset); // Some
    offset += 1;
    createMarketData.writeBigUInt64LE(BigInt(1000000), offset); // b = 1.0
    offset += 8;
    
    // l_parameter: Option<u64> (None)
    createMarketData.writeUInt8(0, offset); // None
    
    const createMarketIx = {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: marketPDA, isSigner: false, isWritable: true },
        { pubkey: versePDA, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: createMarketData.subarray(0, offset + 1),
    };
    
    console.log('Creating market...');
    const marketTx = new Transaction().add(createMarketIx);
    const marketSig = await sendAndConfirmTransaction(connection, marketTx, [payer]);
    console.log('✅ Market created successfully!');
    console.log('Transaction signature:', marketSig);
    console.log('Market ID:', marketId.toString());
    console.log('Market PDA:', marketPDA.toBase58());
    
  } catch (error) {
    console.error('❌ Failed to create market:', error);
    if (error.logs) {
      console.log('Transaction logs:', error.logs);
    }
  }
}

createTestMarket().catch(console.error);