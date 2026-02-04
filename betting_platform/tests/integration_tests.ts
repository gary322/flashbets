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
import { assert, expect } from "chai";
import BN from "bn.js";

describe("Trading Engine Integration Tests", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BettingPlatform as Program<BettingPlatform>;
  
  // Test accounts
  let globalConfig: PublicKey;
  let usdcMint: PublicKey;
  let mmtMint: PublicKey;
  let vaultTokenAccount: PublicKey;
  let authority: Keypair;
  let user1: Keypair;
  let user2: Keypair;
  let keeper: Keypair;
  
  // Verse and proposal accounts
  let verse: PublicKey;
  let proposal: PublicKey;
  let priceCache: PublicKey;
  
  before(async () => {
    // Initialize test keypairs
    authority = Keypair.generate();
    user1 = Keypair.generate();
    user2 = Keypair.generate();
    keeper = Keypair.generate();
    
    // Airdrop SOL to test accounts
    await Promise.all([
      provider.connection.requestAirdrop(authority.publicKey, 10 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(user1.publicKey, 10 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(user2.publicKey, 10 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(keeper.publicKey, 10 * LAMPORTS_PER_SOL),
    ]);
    
    // Wait for airdrops to confirm
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    // Create USDC mint
    usdcMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      6 // USDC has 6 decimals
    );
    
    // Initialize global config
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
  });
  
  describe("User Journey 1: Basic Trading Flow", () => {
    let user1TokenAccount: PublicKey;
    let user1MapEntry: PublicKey;
    
    before(async () => {
      // Create token account for user1
      user1TokenAccount = await createAccount(
        provider.connection,
        user1,
        usdcMint,
        user1.publicKey
      );
      
      // Mint USDC to user1
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        user1TokenAccount,
        authority,
        1000 * 10**6 // 1000 USDC
      );
      
      // Create verse
      const verseId = new BN(Date.now());
      const [versePda] = PublicKey.findProgramAddressSync(
        [Buffer.from("verse"), verseId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      verse = versePda;
      
      // Initialize verse
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
      
      // Create proposal
      const proposalId = new BN(Date.now() + 1);
      const [proposalPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("proposal"), proposalId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      proposal = proposalPda;
      
      // Initialize proposal with LMSR AMM
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
      
      // Create price cache
      const [priceCachePda] = PublicKey.findProgramAddressSync(
        [Buffer.from("price_cache"), proposalId.toArrayLike(Buffer, "le", 16)],
        program.programId
      );
      priceCache = priceCachePda;
      
      // Initialize price cache
      await program.methods
        .initializePriceCache(proposalId, new BN(500_000_000)) // 0.5 in fixed point
        .accounts({
          authority: authority.publicKey,
          priceCache: priceCachePda,
          proposal: proposalPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
      
      // Find user map entry PDA
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          user1.publicKey.toBuffer(),
          verseId.toArrayLike(Buffer, "le", 16)
        ],
        program.programId
      );
      user1MapEntry = mapEntryPda;
    });
    
    it("should open a long position with 10x leverage", async () => {
      const amount = new BN(100 * 10**6); // 100 USDC
      const leverage = new BN(10);
      
      // Get initial balances
      const initialUserBalance = await getAccount(provider.connection, user1TokenAccount);
      
      // Open position
      await program.methods
        .openPosition({
          amount,
          leverage,
          outcome: 0,
          isLong: true,
        })
        .accounts({
          user: user1.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: user1MapEntry,
          priceCache,
          userTokenAccount: user1TokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();
      
      // Verify position created
      const userMap = await program.account.mapEntryPda.fetch(user1MapEntry);
      assert.equal(userMap.positions.length, 1);
      assert.equal(userMap.positions[0].leverage.toNumber(), 10);
      assert.equal(userMap.positions[0].isLong, true);
      
      // Verify collateral transferred
      const finalUserBalance = await getAccount(provider.connection, user1TokenAccount);
      const collateralUsed = Number(initialUserBalance.amount) - Number(finalUserBalance.amount);
      assert.isAbove(collateralUsed, 0);
    });
    
    it("should calculate correct health factor", async () => {
      const userMap = await program.account.mapEntryPda.fetch(user1MapEntry);
      
      // Health factor should be above minimum threshold
      assert.isAbove(userMap.healthFactor.toNumber(), 10000); // 1.0 in basis points
    });
    
    it("should close position with profit", async () => {
      // Simulate price increase
      await program.methods
        .updatePrice(new BN(600_000_000)) // Price increased to 0.6
        .accounts({
          authority: authority.publicKey,
          priceCache,
        })
        .signers([authority])
        .rpc();
      
      // Close position
      await program.methods
        .closePosition(0)
        .accounts({
          user: user1.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: user1MapEntry,
          priceCache,
          userTokenAccount: user1TokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user1])
        .rpc();
      
      // Verify position closed
      const userMap = await program.account.mapEntryPda.fetch(user1MapEntry);
      assert.equal(userMap.positions.length, 0);
      
      // Verify profit received
      const finalBalance = await getAccount(provider.connection, user1TokenAccount);
      assert.isAbove(Number(finalBalance.amount), 100 * 10**6); // More than initial position
    });
  });
  
  describe("User Journey 2: Liquidation Scenario", () => {
    let riskyUser: Keypair;
    let riskyUserTokenAccount: PublicKey;
    let riskyUserMapEntry: PublicKey;
    
    before(async () => {
      riskyUser = Keypair.generate();
      await provider.connection.requestAirdrop(riskyUser.publicKey, 10 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // Create token account
      riskyUserTokenAccount = await createAccount(
        provider.connection,
        riskyUser,
        usdcMint,
        riskyUser.publicKey
      );
      
      // Mint USDC
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        riskyUserTokenAccount,
        authority,
        100 * 10**6 // 100 USDC
      );
      
      // Find map entry PDA
      const verseId = new BN(Date.now());
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          riskyUser.publicKey.toBuffer(),
          verseId.toArrayLike(Buffer, "le", 16)
        ],
        program.programId
      );
      riskyUserMapEntry = mapEntryPda;
    });
    
    it("should open high leverage position", async () => {
      // Open position with maximum leverage
      const amount = new BN(50 * 10**6); // 50 USDC
      const leverage = new BN(50); // High leverage
      
      await program.methods
        .openPosition({
          amount,
          leverage,
          outcome: 0,
          isLong: true,
        })
        .accounts({
          user: riskyUser.publicKey,
          globalConfig,
          verse,
          proposal,
          userMap: riskyUserMapEntry,
          priceCache,
          userTokenAccount: riskyUserTokenAccount,
          vaultTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([riskyUser])
        .rpc();
    });
    
    it("should trigger partial liquidation when price drops", async () => {
      // Simulate significant price drop
      await program.methods
        .updatePrice(new BN(400_000_000)) // Price dropped to 0.4
        .accounts({
          authority: authority.publicKey,
          priceCache,
        })
        .signers([authority])
        .rpc();
      
      // Keeper performs partial liquidation
      const keeperTokenAccount = await createAccount(
        provider.connection,
        keeper,
        usdcMint,
        keeper.publicKey
      );
      
      await program.methods
        .partialLiquidate(0)
        .accounts({
          keeper: keeper.publicKey,
          user: riskyUser.publicKey,
          globalConfig,
          userMap: riskyUserMapEntry,
          priceCache,
          priceHistory,
          vaultTokenAccount,
          keeperTokenAccount,
          vaultAuthority,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([keeper])
        .rpc();
      
      // Verify position partially liquidated
      const userMap = await program.account.mapEntryPda.fetch(riskyUserMapEntry);
      assert.isAbove(userMap.positions.length, 0); // Position still exists
      assert.isBelow(userMap.positions[0].size.toNumber(), 50 * 10**6); // Size reduced
      
      // Verify keeper received reward
      const keeperBalance = await getAccount(provider.connection, keeperTokenAccount);
      assert.isAbove(Number(keeperBalance.amount), 0);
    });
  });
  
  describe("User Journey 3: Chaining Engine for 500x+ Leverage", () => {
    let chainUser: Keypair;
    let chainUserTokenAccount: PublicKey;
    let chainState: PublicKey;
    let verseLiquidityPool: PublicKey;
    let verseStakingPool: PublicKey;
    
    before(async () => {
      chainUser = Keypair.generate();
      await provider.connection.requestAirdrop(chainUser.publicKey, 10 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // Create token account
      chainUserTokenAccount = await createAccount(
        provider.connection,
        chainUser,
        usdcMint,
        chainUser.publicKey
      );
      
      // Mint USDC
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        chainUserTokenAccount,
        authority,
        1000 * 10**6 // 1000 USDC
      );
      
      // Create liquidity pool for verse
      const [liquidityPoolPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("liquidity_pool"), verse.toBuffer()],
        program.programId
      );
      verseLiquidityPool = liquidityPoolPda;
      
      // Create staking pool for verse
      const [stakingPoolPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("staking_pool"), verse.toBuffer()],
        program.programId
      );
      verseStakingPool = stakingPoolPda;
      
      // Initialize pools
      await program.methods
        .initializeLiquidityPool(verse)
        .accounts({
          authority: authority.publicKey,
          verseLiquidityPool,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
      
      await program.methods
        .initializeStakingPool(verse)
        .accounts({
          authority: authority.publicKey,
          verseStakingPool,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
      
      // Find chain state PDA
      const [chainStatePda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("chain_state"),
          chainUser.publicKey.toBuffer(),
          verse.toBuffer()
        ],
        program.programId
      );
      chainState = chainStatePda;
    });
    
    it("should execute 5-step chain for maximum leverage", async () => {
      const deposit = new BN(100 * 10**6); // 100 USDC
      const steps = [
        { borrow: {} },
        { liquidity: {} },
        { stake: {} },
        { borrow: {} },
        { liquidity: {} },
      ];
      
      // Execute auto chain
      await program.methods
        .autoChain(verse, deposit, steps)
        .accounts({
          user: chainUser.publicKey,
          globalConfig,
          versePda: verse,
          chainState,
          verseLiquidityPool,
          verseStakingPool,
          systemProgram: SystemProgram.programId,
        })
        .signers([chainUser])
        .rpc();
      
      // Verify chain created
      const chain = await program.account.chainStatePda.fetch(chainState);
      assert.equal(chain.stepsCompleted, 5);
      assert.equal(chain.status.active, true);
      
      // Verify effective leverage achieved
      const effectiveLeverage = chain.effectiveLeverage.value.toNumber() / 10**9;
      console.log(`Achieved effective leverage: ${effectiveLeverage}x`);
      assert.isAbove(effectiveLeverage, 2.0); // Should achieve at least 2x
    });
    
    it("should prevent dangerous chain configurations", async () => {
      const dangerousSteps = [
        { borrow: {} },
        { stake: {} },
        { borrow: {} }, // Potential cycle
      ];
      
      try {
        await program.methods
          .autoChain(verse, new BN(100 * 10**6), dangerousSteps)
          .accounts({
            user: chainUser.publicKey,
            globalConfig,
            versePda: verse,
            chainState,
            verseLiquidityPool,
            verseStakingPool,
            systemProgram: SystemProgram.programId,
          })
          .signers([chainUser])
          .rpc();
        
        assert.fail("Should have rejected dangerous chain");
      } catch (err) {
        assert.include(err.toString(), "ChainCycle");
      }
    });
    
    it("should unwind chain successfully", async () => {
      const chainId = (await program.account.chainStatePda.fetch(chainState)).chainId;
      
      // Unwind the chain
      await program.methods
        .unwindChain(chainId)
        .accounts({
          user: chainUser.publicKey,
          chainState,
          userTokenAccount: chainUserTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([chainUser])
        .rpc();
      
      // Verify chain unwound
      const chain = await program.account.chainStatePda.fetch(chainState);
      assert.equal(chain.status.completed, true);
      
      // Verify funds returned (with some loss from unwinding)
      const finalBalance = await getAccount(provider.connection, chainUserTokenAccount);
      assert.isAbove(Number(finalBalance.amount), 80 * 10**6); // At least 80% recovered
    });
  });
  
  describe("User Journey 4: Circuit Breaker and Safety Mechanisms", () => {
    it("should halt trading on excessive price movement", async () => {
      // Simulate extreme price movement
      const priceMovement = new BN(600); // 6% movement
      
      await program.methods
        .checkCircuitBreakers(priceMovement)
        .accounts({
          globalConfig,
          priceHistory,
        })
        .signers([authority])
        .rpc();
      
      // Verify system halted
      const config = await program.account.globalConfigPda.fetch(globalConfig);
      assert.equal(config.haltFlag, true);
      
      // Try to open position while halted
      try {
        await program.methods
          .openPosition({
            amount: new BN(100 * 10**6),
            leverage: new BN(10),
            outcome: 0,
            isLong: true,
          })
          .accounts({
            user: user1.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: user1MapEntry,
            priceCache,
            userTokenAccount: user1TokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        
        assert.fail("Should have rejected trade during halt");
      } catch (err) {
        assert.include(err.toString(), "SystemHalted");
      }
    });
    
    it("should enforce leverage limits based on coverage", async () => {
      // Reduce coverage by increasing OI
      await program.methods
        .updateGlobalOi(new BN(1000000 * 10**6)) // Large OI
        .accounts({
          authority: authority.publicKey,
          globalConfig,
        })
        .signers([authority])
        .rpc();
      
      // Try to open position with high leverage
      try {
        await program.methods
          .openPosition({
            amount: new BN(100 * 10**6),
            leverage: new BN(100), // Max leverage
            outcome: 0,
            isLong: true,
          })
          .accounts({
            user: user2.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: user2MapEntry,
            priceCache,
            userTokenAccount: user2TokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user2])
          .rpc();
        
        assert.fail("Should have rejected excessive leverage");
      } catch (err) {
        assert.include(err.toString(), "ExcessiveLeverage");
      }
    });
  });
  
  describe("User Journey 5: Fee Distribution and Vault Management", () => {
    it("should calculate and distribute fees correctly", async () => {
      const feeAmount = new BN(1000 * 10**6); // 1000 USDC in fees
      
      // Get initial balances
      const initialVault = (await program.account.globalConfigPda.fetch(globalConfig)).vault;
      const initialMmtPool = (await program.account.globalConfigPda.fetch(globalConfig)).mmtRewardPool;
      
      // Distribute fees
      await program.methods
        .distributeFees(feeAmount)
        .accounts({
          globalConfig,
          vaultTokenAccount,
          vaultAuthority,
          usdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([authority])
        .rpc();
      
      // Verify distribution
      const config = await program.account.globalConfigPda.fetch(globalConfig);
      const vaultIncrease = config.vault.toNumber() - initialVault.toNumber();
      const mmtIncrease = config.mmtRewardPool.toNumber() - initialMmtPool.toNumber();
      
      // 70% to vault
      assert.approximately(vaultIncrease, 700 * 10**6, 10**6);
      // 20% to MMT
      assert.approximately(mmtIncrease, 200 * 10**6, 10**6);
      // 10% burned (verify by checking total supply reduction)
    });
    
    it("should apply elastic fees based on coverage", async () => {
      // High coverage scenario
      const highCoverage = new BN(2 * 10**9); // 2.0 coverage
      const notional = new BN(1000 * 10**6);
      
      const highCoverageFee = await program.methods
        .calculateTradingFee(notional, highCoverage)
        .accounts({
          globalConfig,
        })
        .view();
      
      // Low coverage scenario
      const lowCoverage = new BN(0.5 * 10**9); // 0.5 coverage
      
      const lowCoverageFee = await program.methods
        .calculateTradingFee(notional, lowCoverage)
        .accounts({
          globalConfig,
        })
        .view();
      
      // Verify elastic pricing
      assert.isBelow(highCoverageFee.toNumber(), lowCoverageFee.toNumber());
      assert.isAbove(lowCoverageFee.toNumber(), 3 * notional.toNumber() / 10000); // > 3bp
    });
  });
  
  describe("User Journey 6: Multi-User Trading Competition", () => {
    let traders: Keypair[] = [];
    const NUM_TRADERS = 5;
    
    before(async () => {
      // Create multiple traders
      for (let i = 0; i < NUM_TRADERS; i++) {
        const trader = Keypair.generate();
        await provider.connection.requestAirdrop(trader.publicKey, 10 * LAMPORTS_PER_SOL);
        traders.push(trader);
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // Give each trader USDC
      for (const trader of traders) {
        const tokenAccount = await createAccount(
          provider.connection,
          trader,
          usdcMint,
          trader.publicKey
        );
        
        await mintTo(
          provider.connection,
          authority,
          usdcMint,
          tokenAccount,
          authority,
          500 * 10**6 // 500 USDC each
        );
      }
    });
    
    it("should handle concurrent position openings", async () => {
      const promises = traders.map(async (trader, i) => {
        const amount = new BN((100 + i * 20) * 10**6); // Different amounts
        const leverage = new BN(10 + i * 5); // Different leverages
        const isLong = i % 2 === 0; // Mix of long and short
        
        const [mapEntryPda] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("map_entry"),
            trader.publicKey.toBuffer(),
            verse.toBuffer()
          ],
          program.programId
        );
        
        await program.methods
          .openPosition({
            amount,
            leverage,
            outcome: i % 2, // Different outcomes
            isLong,
          })
          .accounts({
            user: trader.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: mapEntryPda,
            priceCache,
            userTokenAccount: await getAssociatedTokenAddress(usdcMint, trader.publicKey),
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([trader])
          .rpc();
      });
      
      // Execute all trades concurrently
      await Promise.all(promises);
      
      // Verify all positions created
      for (const trader of traders) {
        const [mapEntryPda] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("map_entry"),
            trader.publicKey.toBuffer(),
            verse.toBuffer()
          ],
          program.programId
        );
        
        const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
        assert.equal(userMap.positions.length, 1);
      }
      
      // Verify global OI updated correctly
      const config = await program.account.globalConfigPda.fetch(globalConfig);
      assert.isAbove(config.totalOi.toNumber(), 0);
    });
  });
  
  describe("Edge Cases and Error Scenarios", () => {
    it("should reject position with insufficient collateral", async () => {
      const poorUser = Keypair.generate();
      await provider.connection.requestAirdrop(poorUser.publicKey, 1 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      const tokenAccount = await createAccount(
        provider.connection,
        poorUser,
        usdcMint,
        poorUser.publicKey
      );
      
      // Only give 10 USDC
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        tokenAccount,
        authority,
        10 * 10**6
      );
      
      try {
        await program.methods
          .openPosition({
            amount: new BN(100 * 10**6), // Wants 100 USDC position
            leverage: new BN(10),
            outcome: 0,
            isLong: true,
          })
          .accounts({
            user: poorUser.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: poorUserMapEntry,
            priceCache,
            userTokenAccount: tokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([poorUser])
          .rpc();
        
        assert.fail("Should have rejected insufficient collateral");
      } catch (err) {
        assert.include(err.toString(), "InsufficientFunds");
      }
    });
    
    it("should reject invalid outcome index", async () => {
      try {
        await program.methods
          .openPosition({
            amount: new BN(100 * 10**6),
            leverage: new BN(10),
            outcome: 5, // Invalid outcome for binary market
            isLong: true,
          })
          .accounts({
            user: user1.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: user1MapEntry,
            priceCache,
            userTokenAccount: user1TokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        
        assert.fail("Should have rejected invalid outcome");
      } catch (err) {
        assert.include(err.toString(), "InvalidOutcome");
      }
    });
    
    it("should handle verse settlement correctly", async () => {
      // Fast forward to settlement slot
      const currentSlot = await provider.connection.getSlot();
      const settleSlot = currentSlot + 100;
      
      // Update verse to be settled
      await program.methods
        .settleVerse(verse, true) // Outcome 0 wins
        .accounts({
          authority: authority.publicKey,
          verse,
        })
        .signers([authority])
        .rpc();
      
      // Try to open position on settled verse
      try {
        await program.methods
          .openPosition({
            amount: new BN(100 * 10**6),
            leverage: new BN(10),
            outcome: 0,
            isLong: true,
          })
          .accounts({
            user: user1.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: user1MapEntry,
            priceCache,
            userTokenAccount: user1TokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        
        assert.fail("Should have rejected trade on settled verse");
      } catch (err) {
        assert.include(err.toString(), "VerseSettled");
      }
    });
  });
  
  describe("Performance and Stress Tests", () => {
    it("should handle maximum positions per user", async () => {
      const maxPositionUser = Keypair.generate();
      await provider.connection.requestAirdrop(maxPositionUser.publicKey, 10 * LAMPORTS_PER_SOL);
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      const tokenAccount = await createAccount(
        provider.connection,
        maxPositionUser,
        usdcMint,
        maxPositionUser.publicKey
      );
      
      await mintTo(
        provider.connection,
        authority,
        usdcMint,
        tokenAccount,
        authority,
        5000 * 10**6 // 5000 USDC for many positions
      );
      
      const [mapEntryPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("map_entry"),
          maxPositionUser.publicKey.toBuffer(),
          verse.toBuffer()
        ],
        program.programId
      );
      
      // Open 50 positions (maximum allowed)
      for (let i = 0; i < 50; i++) {
        await program.methods
          .openPosition({
            amount: new BN(10 * 10**6), // 10 USDC each
            leverage: new BN(5),
            outcome: i % 2,
            isLong: i % 3 === 0,
          })
          .accounts({
            user: maxPositionUser.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: mapEntryPda,
            priceCache,
            userTokenAccount: tokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([maxPositionUser])
          .rpc();
      }
      
      // Verify all positions created
      const userMap = await program.account.mapEntryPda.fetch(mapEntryPda);
      assert.equal(userMap.positions.length, 50);
      
      // Try to open 51st position
      try {
        await program.methods
          .openPosition({
            amount: new BN(10 * 10**6),
            leverage: new BN(5),
            outcome: 0,
            isLong: true,
          })
          .accounts({
            user: maxPositionUser.publicKey,
            globalConfig,
            verse,
            proposal,
            userMap: mapEntryPda,
            priceCache,
            userTokenAccount: tokenAccount,
            vaultTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([maxPositionUser])
          .rpc();
        
        assert.fail("Should have rejected 51st position");
      } catch (err) {
        assert.include(err.toString(), "TooManyPositions");
      }
    });
  });
});

// Helper function to get associated token address
async function getAssociatedTokenAddress(
  mint: PublicKey,
  owner: PublicKey
): Promise<PublicKey> {
  const [address] = PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
  );
  return address;
}