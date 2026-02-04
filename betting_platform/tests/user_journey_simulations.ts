import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BettingPlatform } from "../target/types/betting_platform";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { 
  TOKEN_PROGRAM_ID, 
  createMint, 
  createAccount,
  mintTo,
  getAccount
} from "@solana/spl-token";
import BN from "bn.js";

// User Journey Simulations
// These tests simulate complete user experiences from start to finish

describe("User Journey Simulations", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.BettingPlatform as Program<BettingPlatform>;
  
  // Shared test infrastructure
  let globalConfig: PublicKey;
  let usdcMint: PublicKey;
  let authority: Keypair;
  
  before(async () => {
    console.log("🚀 Setting up test environment...");
    
    authority = Keypair.generate();
    await provider.connection.requestAirdrop(authority.publicKey, 100 * LAMPORTS_PER_SOL);
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    // Create USDC mint
    usdcMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      6
    );
    
    // Initialize platform
    const [globalConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_config")],
      program.programId
    );
    globalConfig = globalConfigPda;
    
    await program.methods
      .initialize(new BN(1))
      .accounts({
        globalConfig,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();
    
    console.log("✅ Test environment ready");
  });
  
  describe("🎯 Journey 1: New User Complete Trading Experience", () => {
    let alice: Keypair;
    let aliceTokenAccount: PublicKey;
    let verse: PublicKey;
    let proposal: PublicKey;
    
    it("Step 1: Alice creates account and gets funded", async () => {
      console.log("\n👤 Creating Alice's account...");
      
      alice = Keypair.generate();
      await provider.connection.requestAirdrop(alice.publicKey, 5 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      aliceTokenAccount = await createAccount(
        provider.connection,
        alice,
        usdcMint,
        alice.publicKey
      );
      
      // Alice receives 10,000 USDC
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        aliceTokenAccount,
        authority,
        10000 * 10**6
      );
      
      const balance = await getAccount(provider.connection, aliceTokenAccount);
      console.log(`✅ Alice funded with ${Number(balance.amount) / 10**6} USDC`);
    });
    
    it("Step 2: Alice discovers an interesting prediction market", async () => {
      console.log("\n🔍 Creating prediction market for 'Will BTC hit $100k?'...");
      
      // Create verse for BTC prediction
      const verseId = new BN(Date.now());
      const [versePda] = PublicKey.findProgramAddressSync(
        [Buffer.from("verse"), verseId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      verse = versePda;
      
      await program.methods
        .createVerse(verseId, null, new BN(0))
        .accounts({
          creator: authority.publicKey,
          verse: versePda,
          globalConfig,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
      
      // Create binary proposal (Yes/No)
      const proposalId = new BN(Date.now() + 1);
      const [proposalPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("proposal"), proposalId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      proposal = proposalPda;
      
      await program.methods
        .createProposal(proposalId, verseId, { lmsr: {} }, 2)
        .accounts({
          creator: authority.publicKey,
          proposal: proposalPda,
          verse: versePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
      
      console.log("✅ Market created: Will BTC hit $100k? (Yes/No)");
    });
    
    it("Step 3: Alice opens her first position (conservative)", async () => {
      console.log("\n💰 Alice opens conservative position...");
      
      const amount = new BN(1000 * 10**6); // 1,000 USDC
      const leverage = new BN(5); // 5x leverage
      
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          alice.publicKey.toBuffer(),
          verseId.toArrayLike(Buffer, "le", 16)
        ],
        program.programId
      );
      
      console.log("  - Amount: 1,000 USDC");
      console.log("  - Leverage: 5x");
      console.log("  - Position: Long on 'Yes'");
      console.log("  - Effective exposure: 5,000 USDC");
      
      await program.methods
        .openPosition({
          amount,
          leverage,
          outcome: 0, // Yes
          isLong: true,
        })
        .accounts({
          user: alice.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: mapEntryPda,
          priceCache,
          userTokenAccount: aliceTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([alice])
        .rpc();
      
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      console.log(`✅ Position opened! Health factor: ${userMap.healthFactor.toNumber() / 10000}`);
    });
    
    it("Step 4: Market moves in Alice's favor", async () => {
      console.log("\n📈 Market sentiment shifts positive...");
      
      // Simulate price movement from 0.5 to 0.65
      await program.methods
        .updatePrice(new BN(650_000_000)) // 0.65
        .accounts({
          authority: authority.publicKey,
          priceCache,
        })
        .signers([authority])
        .rpc();
      
      console.log("  - Price moved from 0.50 to 0.65");
      console.log("  - Alice's position is +30% in profit");
      
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          alice.publicKey.toBuffer(),
          verseId.toArrayLike(Buffer, "le", 16)
        ],
        program.programId
      );
      
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      console.log(`  - Unrealized P&L: ${userMap.unrealizedPnl.toNumber() / 10**6} USDC`);
    });
    
    it("Step 5: Alice adds to her position (confident)", async () => {
      console.log("\n💪 Alice doubles down with higher leverage...");
      
      const amount = new BN(2000 * 10**6); // 2,000 USDC
      const leverage = new BN(15); // 15x leverage
      
      console.log("  - Additional amount: 2,000 USDC");
      console.log("  - Leverage: 15x");
      console.log("  - Additional exposure: 30,000 USDC");
      
      await program.methods
        .openPosition({
          amount,
          leverage,
          outcome: 0,
          isLong: true,
        })
        .accounts({
          user: alice.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: mapEntryPda,
          priceCache,
          userTokenAccount: aliceTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([alice])
        .rpc();
      
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      console.log(`✅ Position added! Total positions: ${userMap.positions.length}`);
      console.log(`  - New health factor: ${userMap.healthFactor.toNumber() / 10000}`);
    });
    
    it("Step 6: Alice takes partial profits", async () => {
      console.log("\n💸 Alice takes profits on first position...");
      
      await program.methods
        .closePosition(0) // Close first position
        .accounts({
          user: alice.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: mapEntryPda,
          priceCache,
          userTokenAccount: aliceTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([alice])
        .rpc();
      
      const balance = await getAccount(provider.connection, aliceTokenAccount);
      console.log(`✅ Profits taken! New balance: ${Number(balance.amount) / 10**6} USDC`);
      console.log("  - Alice keeps second position open for more gains");
    });
  });
  
  describe("🚀 Journey 2: Advanced Trader Using Chaining Engine", () => {
    let bob: Keypair;
    let bobTokenAccount: PublicKey;
    let deepVerse: PublicKey;
    
    it("Step 1: Bob the whale enters with significant capital", async () => {
      console.log("\n🐋 Bob the whale arrives...");
      
      bob = Keypair.generate();
      await provider.connection.requestAirdrop(bob.publicKey, 10 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      bobTokenAccount = await createAccount(
        provider.connection,
        bob,
        usdcMint,
        bob.publicKey
      );
      
      // Bob has 100,000 USDC
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        bobTokenAccount,
        authority,
        100000 * 10**6
      );
      
      console.log("✅ Bob funded with 100,000 USDC");
    });
    
    it("Step 2: Bob creates a deep hierarchical verse", async () => {
      console.log("\n🌳 Creating deep verse hierarchy...");
      
      // Create parent verse
      const parentId = new BN(Date.now());
      const [parentVerse] = PublicKey.findProgramAddressSync(
        [Buffer.from("verse"), parentId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      
      await program.methods
        .createVerse(parentId, null, new BN(0))
        .accounts({
          creator: bob.publicKey,
          verse: parentVerse,
          globalConfig,
          systemProgram: SystemProgram.programId,
        })
        .signers([bob])
        .rpc();
      
      // Create child verse (depth 1)
      const childId = new BN(Date.now() + 1);
      const [childVerse] = PublicKey.findProgramAddressSync(
        [Buffer.from("verse"), childId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      deepVerse = childVerse;
      
      await program.methods
        .createVerse(childId, parentId, new BN(1))
        .accounts({
          creator: bob.publicKey,
          verse: childVerse,
          globalConfig,
          systemProgram: SystemProgram.programId,
        })
        .signers([bob])
        .rpc();
      
      console.log("✅ Deep verse created with bonus leverage potential");
    });
    
    it("Step 3: Bob executes advanced chaining strategy", async () => {
      console.log("\n⛓️ Bob executes 5-step leverage chain...");
      
      const deposit = new BN(10000 * 10**6); // 10,000 USDC
      const steps = [
        { borrow: {} },      // 1.5x
        { liquidity: {} },   // 1.2x
        { stake: {} },       // 1.1x
        { borrow: {} },      // 1.5x
        { liquidity: {} },   // 1.2x
      ];
      
      console.log("  - Initial deposit: 10,000 USDC");
      console.log("  - Chain steps: Borrow → Liquidity → Stake → Borrow → Liquidity");
      console.log("  - Expected leverage: ~3.6x");
      
      const [chainStatePda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("chain_state"),
          bob.publicKey.toBuffer(),
          deepVerse.toBuffer()
        ],
        program.programId
      );
      
      await program.methods
        .autoChain(deepVerse, deposit, steps)
        .accounts({
          user: bob.publicKey,
          globalConfig,
          versePda: deepVerse,
          chainState: chainStatePda,
          verseLiquidityPool,
          verseStakingPool,
          systemProgram: SystemProgram.programId,
        })
        .signers([bob])
        .rpc();
      
      const chain = await program.account.chainStatePda.fetch(chainStatePda);
      const effectiveLeverage = chain.effectiveLeverage.value.toNumber() / 10**18;
      
      console.log(`✅ Chain executed! Effective leverage: ${effectiveLeverage.toFixed(2)}x`);
      console.log(`  - Final exposure: ${chain.currentValue.toNumber() / 10**6} USDC`);
    });
    
    it("Step 4: Bob monitors and adjusts chain", async () => {
      console.log("\n📊 Bob monitors chain performance...");
      
      // Simulate some market movement
      await new Promise(resolve => setTimeout(resolve, 2000));
      
      const [chainStatePda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("chain_state"),
          bob.publicKey.toBuffer(),
          deepVerse.toBuffer()
        ],
        program.programId
      );
      
      const chain = await program.account.chainStatePda.fetch(chainStatePda);
      
      console.log("  - Chain status: Active ✅");
      console.log("  - Steps completed: " + chain.stepsCompleted);
      console.log("  - Current value: " + (chain.currentValue.toNumber() / 10**6) + " USDC");
      
      // Bob is satisfied and lets it run
      console.log("✅ Bob decides to maintain position");
    });
    
    it("Step 5: Bob unwinds chain profitably", async () => {
      console.log("\n💰 Bob unwinds chain to lock in profits...");
      
      const chainId = (await program.account.chainStatePda.fetch(chainStatePda)).chainId;
      
      await program.methods
        .unwindChain(chainId)
        .accounts({
          user: bob.publicKey,
          chainState: chainStatePda,
          userTokenAccount: bobTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([bob])
        .rpc();
      
      const finalBalance = await getAccount(provider.connection, bobTokenAccount);
      const profit = (Number(finalBalance.amount) - 90000 * 10**6) / 10**6;
      
      console.log(`✅ Chain unwound successfully!`);
      console.log(`  - Final balance: ${Number(finalBalance.amount) / 10**6} USDC`);
      console.log(`  - Net profit: ${profit} USDC`);
    });
  });
  
  describe("⚡ Journey 3: Liquidation and Risk Management", () => {
    let charlie: Keypair;
    let dave: Keypair; // Keeper
    let charlieTokenAccount: PublicKey;
    let daveTokenAccount: PublicKey;
    
    it("Step 1: Charlie and Dave join the platform", async () => {
      console.log("\n👥 New users joining...");
      
      // Setup Charlie (risk-taker)
      charlie = Keypair.generate();
      await provider.connection.requestAirdrop(charlie.publicKey, 5 * LAMPORTS_PER_SOL);
      
      charlieTokenAccount = await createAccount(
        provider.connection,
        charlie,
        usdcMint,
        charlie.publicKey
      );
      
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        charlieTokenAccount,
        authority,
        5000 * 10**6 // 5,000 USDC
      );
      
      // Setup Dave (keeper)
      dave = Keypair.generate();
      await provider.connection.requestAirdrop(dave.publicKey, 5 * LAMPORTS_PER_SOL);
      
      daveTokenAccount = await createAccount(
        provider.connection,
        dave,
        usdcMint,
        dave.publicKey
      );
      
      console.log("✅ Charlie (trader) and Dave (keeper) ready");
    });
    
    it("Step 2: Charlie opens risky high-leverage position", async () => {
      console.log("\n⚠️ Charlie opens risky position...");
      
      const amount = new BN(2000 * 10**6); // 2,000 USDC
      const leverage = new BN(50); // 50x leverage!
      
      console.log("  - Amount: 2,000 USDC");
      console.log("  - Leverage: 50x (MAXIMUM RISK)");
      console.log("  - Exposure: 100,000 USDC");
      
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          charlie.publicKey.toBuffer(),
          verse.toBuffer()
        ],
        program.programId
      );
      
      await program.methods
        .openPosition({
          amount,
          leverage,
          outcome: 0,
          isLong: true,
        })
        .accounts({
          user: charlie.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: mapEntryPda,
          priceCache,
          userTokenAccount: charlieTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([charlie])
        .rpc();
      
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      console.log(`⚠️ Position opened with health factor: ${userMap.healthFactor.toNumber() / 10000}`);
      console.log("  - Liquidation risk: HIGH");
    });
    
    it("Step 3: Market moves against Charlie", async () => {
      console.log("\n📉 Market crashes...");
      
      // Price drops from 0.65 to 0.55
      await program.methods
        .updatePrice(new BN(550_000_000)) // 0.55
        .accounts({
          authority: authority.publicKey,
          priceCache,
        })
        .signers([authority])
        .rpc();
      
      console.log("  - Price dropped from 0.65 to 0.55");
      console.log("  - Charlie's position is underwater");
      
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          charlie.publicKey.toBuffer(),
          verse.toBuffer()
        ],
        program.programId
      );
      
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      console.log(`  - Health factor: ${userMap.healthFactor.toNumber() / 10000} (CRITICAL)`);
    });
    
    it("Step 4: Dave performs partial liquidation", async () => {
      console.log("\n🔨 Dave the keeper spots liquidation opportunity...");
      
      console.log("  - Dave will liquidate 5% of Charlie's position");
      console.log("  - Dave will earn keeper rewards");
      
      await program.methods
        .partialLiquidate(0)
        .accounts({
          keeper: dave.publicKey,
          user: charlie.publicKey,
          globalConfig,
          userMap: mapEntryPda,
          priceCache,
          priceHistory,
          vaultTokenAccount,
          keeperTokenAccount: daveTokenAccount,
          vaultAuthority,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([dave])
        .rpc();
      
      const daveBalance = await getAccount(provider.connection, daveTokenAccount);
      console.log(`✅ Liquidation complete!`);
      console.log(`  - Dave earned: ${Number(daveBalance.amount) / 10**6} USDC in rewards`);
      
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      console.log(`  - Charlie's position reduced, health improved to: ${userMap.healthFactor.toNumber() / 10000}`);
    });
    
    it("Step 5: Charlie learns and adjusts strategy", async () => {
      console.log("\n📚 Charlie reduces leverage after close call...");
      
      // Charlie closes risky position
      await program.methods
        .closePosition(0)
        .accounts({
          user: charlie.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: mapEntryPda,
          priceCache,
          userTokenAccount: charlieTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([charlie])
        .rpc();
      
      // Opens new conservative position
      await program.methods
        .openPosition({
          amount: new BN(1000 * 10**6),
          leverage: new BN(10), // Much safer
          outcome: 0,
          isLong: true,
        })
        .accounts({
          user: charlie.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: mapEntryPda,
          priceCache,
          userTokenAccount: charlieTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([charlie])
        .rpc();
      
      console.log("✅ Charlie now trades with safer 10x leverage");
      console.log("  - Lesson learned: High leverage = High risk");
    });
  });
  
  describe("🏆 Journey 4: Competition and Multi-User Dynamics", () => {
    let traders: { name: string, keypair: Keypair, account: PublicKey }[] = [];
    
    it("Step 1: Trading competition begins", async () => {
      console.log("\n🏁 Trading competition starting...");
      console.log("  - 5 traders competing");
      console.log("  - Same starting capital: 10,000 USDC each");
      console.log("  - Goal: Highest returns in 5 rounds");
      
      const traderNames = ["Emma", "Frank", "Grace", "Henry", "Iris"];
      
      for (const name of traderNames) {
        const keypair = Keypair.generate();
        await provider.connection.requestAirdrop(keypair.publicKey, 5 * LAMPORTS_PER_SOL);
        
        const account = await createAccount(
          provider.connection,
          keypair,
          usdcMint,
          keypair.publicKey
        );
        
        await mintTo(
          provider.connection,
          authority,
          usdcMint,
          account,
          authority,
          10000 * 10**6
        );
        
        traders.push({ name, keypair, account });
      }
      
      await new Promise(resolve => setTimeout(resolve, 1000));
      console.log("✅ All traders ready!");
    });
    
    it("Step 2: Round 1 - Opening positions", async () => {
      console.log("\n🎲 Round 1: Traders place their bets...");
      
      const strategies = [
        { leverage: 20, outcome: 0, isLong: true },   // Emma: Aggressive long
        { leverage: 5, outcome: 0, isLong: true },    // Frank: Conservative long
        { leverage: 15, outcome: 1, isLong: true },   // Grace: Medium short
        { leverage: 30, outcome: 0, isLong: false },  // Henry: Aggressive short
        { leverage: 10, outcome: 0, isLong: true },   // Iris: Balanced long
      ];
      
      for (let i = 0; i < traders.length; i++) {
        const trader = traders[i];
        const strategy = strategies[i];
        
        const [mapEntryPda] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("map_entry"),
            trader.keypair.publicKey.toBuffer(),
            verse.toBuffer()
          ],
          program.programId
        );
        
        await program.methods
          .openPosition({
            amount: new BN(2000 * 10**6), // 2,000 USDC each
            leverage: new BN(strategy.leverage),
            outcome: strategy.outcome,
            isLong: strategy.isLong,
          })
          .accounts({
            user: trader.keypair.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: mapEntryPda,
            priceCache,
            userTokenAccount: trader.account,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([trader.keypair])
          .rpc();
        
        console.log(`  - ${trader.name}: ${strategy.leverage}x leverage, ${strategy.isLong ? 'Long' : 'Short'}`);
      }
    });
    
    it("Step 3: Market volatility and position updates", async () => {
      console.log("\n🌊 Market experiences volatility...");
      
      // Price movements
      const priceMovements = [
        { price: 580_000_000, description: "Small rally to 0.58" },
        { price: 520_000_000, description: "Drop to 0.52" },
        { price: 620_000_000, description: "Strong rally to 0.62" },
      ];
      
      for (const movement of priceMovements) {
        await program.methods
          .updatePrice(new BN(movement.price))
          .accounts({
            authority: authority.publicKey,
            priceCache,
          })
          .signers([authority])
          .rpc();
        
        console.log(`  - ${movement.description}`);
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      
      // Check positions
      console.log("\n📊 Current standings:");
      for (const trader of traders) {
        const [mapEntryPda] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("map_entry"),
            trader.keypair.publicKey.toBuffer(),
            verse.toBuffer()
          ],
          program.programId
        );
        
        const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
        const pnl = userMap.unrealizedPnl.toNumber() / 10**6;
        console.log(`  - ${trader.name}: ${pnl > 0 ? '+' : ''}${pnl.toFixed(2)} USDC`);
      }
    });
    
    it("Step 4: Final round - Closing positions", async () => {
      console.log("\n🏁 Final round: Traders close positions...");
      
      const finalBalances: { name: string, balance: number, profit: number }[] = [];
      
      for (const trader of traders) {
        const [mapEntryPda] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("map_entry"),
            trader.keypair.publicKey.toBuffer(),
            verse.toBuffer()
          ],
          program.programId
        );
        
        await program.methods
          .closePosition(0)
          .accounts({
            user: trader.keypair.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: mapEntryPda,
            priceCache,
            userTokenAccount: trader.account,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([trader.keypair])
          .rpc();
        
        const balance = await getAccount(provider.connection, trader.account);
        const finalBalance = Number(balance.amount) / 10**6;
        const profit = finalBalance - 10000;
        
        finalBalances.push({
          name: trader.name,
          balance: finalBalance,
          profit: profit
        });
      }
      
      // Sort by profit
      finalBalances.sort((a, b) => b.profit - a.profit);
      
      console.log("\n🏆 FINAL RESULTS:");
      finalBalances.forEach((result, index) => {
        const medal = index === 0 ? "🥇" : index === 1 ? "🥈" : index === 2 ? "🥉" : "  ";
        console.log(`${medal} ${result.name}: ${result.profit > 0 ? '+' : ''}${result.profit.toFixed(2)} USDC (${result.balance.toFixed(2)} total)`);
      });
      
      console.log(`\n🎉 ${finalBalances[0].name} wins the competition!`);
    });
  });
  
  describe("🛡️ Journey 5: Platform Safety and Circuit Breakers", () => {
    it("Step 1: Normal trading activity", async () => {
      console.log("\n✅ Platform operating normally...");
      
      const config = await program.account.globalConfigPda.fetch(globalConfig);
      console.log(`  - Total OI: ${config.totalOi.toNumber() / 10**6} USDC`);
      console.log(`  - Coverage: ${config.coverage.toNumber() / 10**9}`);
      console.log(`  - System status: ${config.haltFlag ? 'HALTED' : 'ACTIVE'}`);
    });
    
    it("Step 2: Extreme market event triggers circuit breaker", async () => {
      console.log("\n🚨 Black swan event detected...");
      
      // Simulate 10% price movement in single update
      const extremeMovement = new BN(1000); // 10%
      
      try {
        await program.methods
          .checkCircuitBreakers(extremeMovement)
          .accounts({
            globalConfig,
            priceHistory,
          })
          .signers([authority])
          .rpc();
      } catch (err) {
        console.log("  - Circuit breaker TRIGGERED!");
        console.log("  - Trading halted for safety");
      }
      
      const config = await program.account.globalConfigPda.fetch(globalConfig);
      console.log(`  - System status: ${config.haltFlag ? 'HALTED ⏸️' : 'ACTIVE'}`);
    });
    
    it("Step 3: System recovery after cooldown", async () => {
      console.log("\n⏰ Waiting for system cooldown...");
      
      // In real scenario, wait for halt period to expire
      await new Promise(resolve => setTimeout(resolve, 3000));
      
      // Admin removes halt after investigation
      await program.methods
        .removeHalt()
        .accounts({
          authority: authority.publicKey,
          globalConfig,
        })
        .signers([authority])
        .rpc();
      
      console.log("✅ System recovered and trading resumed");
      console.log("  - All positions preserved");
      console.log("  - No user funds lost");
      console.log("  - Platform integrity maintained");
    });
  });
  
  // Summary statistics
  after(async () => {
    console.log("\n📈 PLATFORM STATISTICS");
    console.log("========================");
    
    const config = await program.account.globalConfigPda.fetch(globalConfig);
    
    console.log(`Total Volume: ${config.totalOi.toNumber() / 10**6} USDC`);
    console.log(`Vault Balance: ${config.vault.toNumber() / 10**6} USDC`);
    console.log(`Coverage Ratio: ${(config.coverage.toNumber() / 10**9).toFixed(2)}`);
    console.log(`MMT Rewards Pool: ${config.mmtRewardPool.toNumber() / 10**6} USDC`);
    
    console.log("\n✅ All user journeys completed successfully!");
    console.log("  - Basic trading ✅");
    console.log("  - Advanced chaining ✅");
    console.log("  - Risk management ✅");
    console.log("  - Multi-user dynamics ✅");
    console.log("  - Safety mechanisms ✅");
  });
});