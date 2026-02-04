"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const anchor = __importStar(require("@coral-xyz/anchor"));
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const chai_1 = require("chai");
const bn_js_1 = __importDefault(require("bn.js"));
describe("Trading Engine Integration Tests", () => {
    // Configure the client to use the local cluster
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.BettingPlatform;
    // Test accounts
    let globalConfig;
    let usdcMint;
    let mmtMint;
    let vaultTokenAccount;
    let authority;
    let user1;
    let user2;
    let keeper;
    // Verse and proposal accounts
    let verse;
    let proposal;
    let priceCache;
    before(() => __awaiter(void 0, void 0, void 0, function* () {
        // Initialize test keypairs
        authority = web3_js_1.Keypair.generate();
        user1 = web3_js_1.Keypair.generate();
        user2 = web3_js_1.Keypair.generate();
        keeper = web3_js_1.Keypair.generate();
        // Airdrop SOL to test accounts
        yield Promise.all([
            provider.connection.requestAirdrop(authority.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL),
            provider.connection.requestAirdrop(user1.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL),
            provider.connection.requestAirdrop(user2.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL),
            provider.connection.requestAirdrop(keeper.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL),
        ]);
        // Wait for airdrops to confirm
        yield new Promise(resolve => setTimeout(resolve, 1000));
        // Create USDC mint
        usdcMint = yield (0, spl_token_1.createMint)(provider.connection, authority, authority.publicKey, null, 6 // USDC has 6 decimals
        );
        // Initialize global config
        const [globalConfigPda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("global_config")], program.programId);
        globalConfig = globalConfigPda;
        yield program.methods
            .initialize(new bn_js_1.default(1))
            .accounts({
            globalConfig,
            authority: authority.publicKey,
            systemProgram: web3_js_1.SystemProgram.programId,
        })
            .signers([authority])
            .rpc();
    }));
    describe("User Journey 1: Basic Trading Flow", () => {
        let user1TokenAccount;
        let user1MapEntry;
        before(() => __awaiter(void 0, void 0, void 0, function* () {
            // Create token account for user1
            user1TokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, user1, usdcMint, user1.publicKey);
            // Mint USDC to user1
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, user1TokenAccount, authority, 1000 * Math.pow(10, 6) // 1000 USDC
            );
            // Create verse
            const verseId = new bn_js_1.default(Date.now());
            const [versePda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("verse"), verseId.toArrayLike(Buffer, "le", 16)], program.programId);
            verse = versePda;
            // Initialize verse
            yield program.methods
                .createVerse(verseId, null, new bn_js_1.default(0))
                .accounts({
                creator: authority.publicKey,
                verse: versePda,
                globalConfig,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([authority])
                .rpc();
            // Create proposal
            const proposalId = new bn_js_1.default(Date.now() + 1);
            const [proposalPda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("proposal"), proposalId.toArrayLike(Buffer, "le", 16)], program.programId);
            proposal = proposalPda;
            // Initialize proposal with LMSR AMM
            yield program.methods
                .createProposal(proposalId, verseId, { lmsr: {} }, 2)
                .accounts({
                creator: authority.publicKey,
                proposal: proposalPda,
                verse: versePda,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([authority])
                .rpc();
            // Create price cache
            const [priceCachePda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("price_cache"), proposalId.toArrayLike(Buffer, "le", 16)], program.programId);
            priceCache = priceCachePda;
            // Initialize price cache
            yield program.methods
                .initializePriceCache(proposalId, new bn_js_1.default(500000000)) // 0.5 in fixed point
                .accounts({
                authority: authority.publicKey,
                priceCache: priceCachePda,
                proposal: proposalPda,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([authority])
                .rpc();
            // Find user map entry PDA
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                user1.publicKey.toBuffer(),
                verseId.toArrayLike(Buffer, "le", 16)
            ], program.programId);
            user1MapEntry = mapEntryPda;
        }));
        it("should open a long position with 10x leverage", () => __awaiter(void 0, void 0, void 0, function* () {
            const amount = new bn_js_1.default(100 * Math.pow(10, 6)); // 100 USDC
            const leverage = new bn_js_1.default(10);
            // Get initial balances
            const initialUserBalance = yield (0, spl_token_1.getAccount)(provider.connection, user1TokenAccount);
            // Open position
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([user1])
                .rpc();
            // Verify position created
            const userMap = yield program.account.mapEntryPda.fetch(user1MapEntry);
            chai_1.assert.equal(userMap.positions.length, 1);
            chai_1.assert.equal(userMap.positions[0].leverage.toNumber(), 10);
            chai_1.assert.equal(userMap.positions[0].isLong, true);
            // Verify collateral transferred
            const finalUserBalance = yield (0, spl_token_1.getAccount)(provider.connection, user1TokenAccount);
            const collateralUsed = Number(initialUserBalance.amount) - Number(finalUserBalance.amount);
            chai_1.assert.isAbove(collateralUsed, 0);
        }));
        it("should calculate correct health factor", () => __awaiter(void 0, void 0, void 0, function* () {
            const userMap = yield program.account.mapEntryPda.fetch(user1MapEntry);
            // Health factor should be above minimum threshold
            chai_1.assert.isAbove(userMap.healthFactor.toNumber(), 10000); // 1.0 in basis points
        }));
        it("should close position with profit", () => __awaiter(void 0, void 0, void 0, function* () {
            // Simulate price increase
            yield program.methods
                .updatePrice(new bn_js_1.default(600000000)) // Price increased to 0.6
                .accounts({
                authority: authority.publicKey,
                priceCache,
            })
                .signers([authority])
                .rpc();
            // Close position
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([user1])
                .rpc();
            // Verify position closed
            const userMap = yield program.account.mapEntryPda.fetch(user1MapEntry);
            chai_1.assert.equal(userMap.positions.length, 0);
            // Verify profit received
            const finalBalance = yield (0, spl_token_1.getAccount)(provider.connection, user1TokenAccount);
            chai_1.assert.isAbove(Number(finalBalance.amount), 100 * Math.pow(10, 6)); // More than initial position
        }));
    });
    describe("User Journey 2: Liquidation Scenario", () => {
        let riskyUser;
        let riskyUserTokenAccount;
        let riskyUserMapEntry;
        before(() => __awaiter(void 0, void 0, void 0, function* () {
            riskyUser = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(riskyUser.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL);
            yield new Promise(resolve => setTimeout(resolve, 1000));
            // Create token account
            riskyUserTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, riskyUser, usdcMint, riskyUser.publicKey);
            // Mint USDC
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, riskyUserTokenAccount, authority, 100 * Math.pow(10, 6) // 100 USDC
            );
            // Find map entry PDA
            const verseId = new bn_js_1.default(Date.now());
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                riskyUser.publicKey.toBuffer(),
                verseId.toArrayLike(Buffer, "le", 16)
            ], program.programId);
            riskyUserMapEntry = mapEntryPda;
        }));
        it("should open high leverage position", () => __awaiter(void 0, void 0, void 0, function* () {
            // Open position with maximum leverage
            const amount = new bn_js_1.default(50 * Math.pow(10, 6)); // 50 USDC
            const leverage = new bn_js_1.default(50); // High leverage
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([riskyUser])
                .rpc();
        }));
        it("should trigger partial liquidation when price drops", () => __awaiter(void 0, void 0, void 0, function* () {
            // Simulate significant price drop
            yield program.methods
                .updatePrice(new bn_js_1.default(400000000)) // Price dropped to 0.4
                .accounts({
                authority: authority.publicKey,
                priceCache,
            })
                .signers([authority])
                .rpc();
            // Keeper performs partial liquidation
            const keeperTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, keeper, usdcMint, keeper.publicKey);
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([keeper])
                .rpc();
            // Verify position partially liquidated
            const userMap = yield program.account.mapEntryPda.fetch(riskyUserMapEntry);
            chai_1.assert.isAbove(userMap.positions.length, 0); // Position still exists
            chai_1.assert.isBelow(userMap.positions[0].size.toNumber(), 50 * Math.pow(10, 6)); // Size reduced
            // Verify keeper received reward
            const keeperBalance = yield (0, spl_token_1.getAccount)(provider.connection, keeperTokenAccount);
            chai_1.assert.isAbove(Number(keeperBalance.amount), 0);
        }));
    });
    describe("User Journey 3: Chaining Engine for 500x+ Leverage", () => {
        let chainUser;
        let chainUserTokenAccount;
        let chainState;
        let verseLiquidityPool;
        let verseStakingPool;
        before(() => __awaiter(void 0, void 0, void 0, function* () {
            chainUser = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(chainUser.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL);
            yield new Promise(resolve => setTimeout(resolve, 1000));
            // Create token account
            chainUserTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, chainUser, usdcMint, chainUser.publicKey);
            // Mint USDC
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, chainUserTokenAccount, authority, 1000 * Math.pow(10, 6) // 1000 USDC
            );
            // Create liquidity pool for verse
            const [liquidityPoolPda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("liquidity_pool"), verse.toBuffer()], program.programId);
            verseLiquidityPool = liquidityPoolPda;
            // Create staking pool for verse
            const [stakingPoolPda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("staking_pool"), verse.toBuffer()], program.programId);
            verseStakingPool = stakingPoolPda;
            // Initialize pools
            yield program.methods
                .initializeLiquidityPool(verse)
                .accounts({
                authority: authority.publicKey,
                verseLiquidityPool,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([authority])
                .rpc();
            yield program.methods
                .initializeStakingPool(verse)
                .accounts({
                authority: authority.publicKey,
                verseStakingPool,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([authority])
                .rpc();
            // Find chain state PDA
            const [chainStatePda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("chain_state"),
                chainUser.publicKey.toBuffer(),
                verse.toBuffer()
            ], program.programId);
            chainState = chainStatePda;
        }));
        it("should execute 5-step chain for maximum leverage", () => __awaiter(void 0, void 0, void 0, function* () {
            const deposit = new bn_js_1.default(100 * Math.pow(10, 6)); // 100 USDC
            const steps = [
                { borrow: {} },
                { liquidity: {} },
                { stake: {} },
                { borrow: {} },
                { liquidity: {} },
            ];
            // Execute auto chain
            yield program.methods
                .autoChain(verse, deposit, steps)
                .accounts({
                user: chainUser.publicKey,
                globalConfig,
                versePda: verse,
                chainState,
                verseLiquidityPool,
                verseStakingPool,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([chainUser])
                .rpc();
            // Verify chain created
            const chain = yield program.account.chainStatePda.fetch(chainState);
            chai_1.assert.equal(chain.stepsCompleted, 5);
            chai_1.assert.equal(chain.status.active, true);
            // Verify effective leverage achieved
            const effectiveLeverage = chain.effectiveLeverage.value.toNumber() / Math.pow(10, 9);
            console.log(`Achieved effective leverage: ${effectiveLeverage}x`);
            chai_1.assert.isAbove(effectiveLeverage, 2.0); // Should achieve at least 2x
        }));
        it("should prevent dangerous chain configurations", () => __awaiter(void 0, void 0, void 0, function* () {
            const dangerousSteps = [
                { borrow: {} },
                { stake: {} },
                { borrow: {} }, // Potential cycle
            ];
            try {
                yield program.methods
                    .autoChain(verse, new bn_js_1.default(100 * Math.pow(10, 6)), dangerousSteps)
                    .accounts({
                    user: chainUser.publicKey,
                    globalConfig,
                    versePda: verse,
                    chainState,
                    verseLiquidityPool,
                    verseStakingPool,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([chainUser])
                    .rpc();
                chai_1.assert.fail("Should have rejected dangerous chain");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "ChainCycle");
            }
        }));
        it("should unwind chain successfully", () => __awaiter(void 0, void 0, void 0, function* () {
            const chainId = (yield program.account.chainStatePda.fetch(chainState)).chainId;
            // Unwind the chain
            yield program.methods
                .unwindChain(chainId)
                .accounts({
                user: chainUser.publicKey,
                chainState,
                userTokenAccount: chainUserTokenAccount,
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([chainUser])
                .rpc();
            // Verify chain unwound
            const chain = yield program.account.chainStatePda.fetch(chainState);
            chai_1.assert.equal(chain.status.completed, true);
            // Verify funds returned (with some loss from unwinding)
            const finalBalance = yield (0, spl_token_1.getAccount)(provider.connection, chainUserTokenAccount);
            chai_1.assert.isAbove(Number(finalBalance.amount), 80 * Math.pow(10, 6)); // At least 80% recovered
        }));
    });
    describe("User Journey 4: Circuit Breaker and Safety Mechanisms", () => {
        it("should halt trading on excessive price movement", () => __awaiter(void 0, void 0, void 0, function* () {
            // Simulate extreme price movement
            const priceMovement = new bn_js_1.default(600); // 6% movement
            yield program.methods
                .checkCircuitBreakers(priceMovement)
                .accounts({
                globalConfig,
                priceHistory,
            })
                .signers([authority])
                .rpc();
            // Verify system halted
            const config = yield program.account.globalConfigPda.fetch(globalConfig);
            chai_1.assert.equal(config.haltFlag, true);
            // Try to open position while halted
            try {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(100 * Math.pow(10, 6)),
                    leverage: new bn_js_1.default(10),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([user1])
                    .rpc();
                chai_1.assert.fail("Should have rejected trade during halt");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "SystemHalted");
            }
        }));
        it("should enforce leverage limits based on coverage", () => __awaiter(void 0, void 0, void 0, function* () {
            // Reduce coverage by increasing OI
            yield program.methods
                .updateGlobalOi(new bn_js_1.default(1000000 * Math.pow(10, 6))) // Large OI
                .accounts({
                authority: authority.publicKey,
                globalConfig,
            })
                .signers([authority])
                .rpc();
            // Try to open position with high leverage
            try {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(100 * Math.pow(10, 6)),
                    leverage: new bn_js_1.default(100), // Max leverage
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([user2])
                    .rpc();
                chai_1.assert.fail("Should have rejected excessive leverage");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "ExcessiveLeverage");
            }
        }));
    });
    describe("User Journey 5: Fee Distribution and Vault Management", () => {
        it("should calculate and distribute fees correctly", () => __awaiter(void 0, void 0, void 0, function* () {
            const feeAmount = new bn_js_1.default(1000 * Math.pow(10, 6)); // 1000 USDC in fees
            // Get initial balances
            const initialVault = (yield program.account.globalConfigPda.fetch(globalConfig)).vault;
            const initialMmtPool = (yield program.account.globalConfigPda.fetch(globalConfig)).mmtRewardPool;
            // Distribute fees
            yield program.methods
                .distributeFees(feeAmount)
                .accounts({
                globalConfig,
                vaultTokenAccount,
                vaultAuthority,
                usdcMint,
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([authority])
                .rpc();
            // Verify distribution
            const config = yield program.account.globalConfigPda.fetch(globalConfig);
            const vaultIncrease = config.vault.toNumber() - initialVault.toNumber();
            const mmtIncrease = config.mmtRewardPool.toNumber() - initialMmtPool.toNumber();
            // 70% to vault
            chai_1.assert.approximately(vaultIncrease, 700 * Math.pow(10, 6), Math.pow(10, 6));
            // 20% to MMT
            chai_1.assert.approximately(mmtIncrease, 200 * Math.pow(10, 6), Math.pow(10, 6));
            // 10% burned (verify by checking total supply reduction)
        }));
        it("should apply elastic fees based on coverage", () => __awaiter(void 0, void 0, void 0, function* () {
            // High coverage scenario
            const highCoverage = new bn_js_1.default(2 * Math.pow(10, 9)); // 2.0 coverage
            const notional = new bn_js_1.default(1000 * Math.pow(10, 6));
            const highCoverageFee = yield program.methods
                .calculateTradingFee(notional, highCoverage)
                .accounts({
                globalConfig,
            })
                .view();
            // Low coverage scenario
            const lowCoverage = new bn_js_1.default(0.5 * Math.pow(10, 9)); // 0.5 coverage
            const lowCoverageFee = yield program.methods
                .calculateTradingFee(notional, lowCoverage)
                .accounts({
                globalConfig,
            })
                .view();
            // Verify elastic pricing
            chai_1.assert.isBelow(highCoverageFee.toNumber(), lowCoverageFee.toNumber());
            chai_1.assert.isAbove(lowCoverageFee.toNumber(), 3 * notional.toNumber() / 10000); // > 3bp
        }));
    });
    describe("User Journey 6: Multi-User Trading Competition", () => {
        let traders = [];
        const NUM_TRADERS = 5;
        before(() => __awaiter(void 0, void 0, void 0, function* () {
            // Create multiple traders
            for (let i = 0; i < NUM_TRADERS; i++) {
                const trader = web3_js_1.Keypair.generate();
                yield provider.connection.requestAirdrop(trader.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL);
                traders.push(trader);
            }
            yield new Promise(resolve => setTimeout(resolve, 1000));
            // Give each trader USDC
            for (const trader of traders) {
                const tokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, trader, usdcMint, trader.publicKey);
                yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, tokenAccount, authority, 500 * Math.pow(10, 6) // 500 USDC each
                );
            }
        }));
        it("should handle concurrent position openings", () => __awaiter(void 0, void 0, void 0, function* () {
            const promises = traders.map((trader, i) => __awaiter(void 0, void 0, void 0, function* () {
                const amount = new bn_js_1.default((100 + i * 20) * Math.pow(10, 6)); // Different amounts
                const leverage = new bn_js_1.default(10 + i * 5); // Different leverages
                const isLong = i % 2 === 0; // Mix of long and short
                const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                    Buffer.from("map_entry"),
                    trader.publicKey.toBuffer(),
                    verse.toBuffer()
                ], program.programId);
                yield program.methods
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
                    userTokenAccount: yield getAssociatedTokenAddress(usdcMint, trader.publicKey),
                    vaultTokenAccount,
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([trader])
                    .rpc();
            }));
            // Execute all trades concurrently
            yield Promise.all(promises);
            // Verify all positions created
            for (const trader of traders) {
                const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                    Buffer.from("map_entry"),
                    trader.publicKey.toBuffer(),
                    verse.toBuffer()
                ], program.programId);
                const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
                chai_1.assert.equal(userMap.positions.length, 1);
            }
            // Verify global OI updated correctly
            const config = yield program.account.globalConfigPda.fetch(globalConfig);
            chai_1.assert.isAbove(config.totalOi.toNumber(), 0);
        }));
    });
    describe("Edge Cases and Error Scenarios", () => {
        it("should reject position with insufficient collateral", () => __awaiter(void 0, void 0, void 0, function* () {
            const poorUser = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(poorUser.publicKey, 1 * web3_js_1.LAMPORTS_PER_SOL);
            yield new Promise(resolve => setTimeout(resolve, 1000));
            const tokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, poorUser, usdcMint, poorUser.publicKey);
            // Only give 10 USDC
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, tokenAccount, authority, 10 * Math.pow(10, 6));
            try {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(100 * Math.pow(10, 6)), // Wants 100 USDC position
                    leverage: new bn_js_1.default(10),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([poorUser])
                    .rpc();
                chai_1.assert.fail("Should have rejected insufficient collateral");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "InsufficientFunds");
            }
        }));
        it("should reject invalid outcome index", () => __awaiter(void 0, void 0, void 0, function* () {
            try {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(100 * Math.pow(10, 6)),
                    leverage: new bn_js_1.default(10),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([user1])
                    .rpc();
                chai_1.assert.fail("Should have rejected invalid outcome");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "InvalidOutcome");
            }
        }));
        it("should handle verse settlement correctly", () => __awaiter(void 0, void 0, void 0, function* () {
            // Fast forward to settlement slot
            const currentSlot = yield provider.connection.getSlot();
            const settleSlot = currentSlot + 100;
            // Update verse to be settled
            yield program.methods
                .settleVerse(verse, true) // Outcome 0 wins
                .accounts({
                authority: authority.publicKey,
                verse,
            })
                .signers([authority])
                .rpc();
            // Try to open position on settled verse
            try {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(100 * Math.pow(10, 6)),
                    leverage: new bn_js_1.default(10),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([user1])
                    .rpc();
                chai_1.assert.fail("Should have rejected trade on settled verse");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "VerseSettled");
            }
        }));
    });
    describe("Performance and Stress Tests", () => {
        it("should handle maximum positions per user", () => __awaiter(void 0, void 0, void 0, function* () {
            const maxPositionUser = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(maxPositionUser.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL);
            yield new Promise(resolve => setTimeout(resolve, 1000));
            const tokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, maxPositionUser, usdcMint, maxPositionUser.publicKey);
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, tokenAccount, authority, 5000 * Math.pow(10, 6) // 5000 USDC for many positions
            );
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                maxPositionUser.publicKey.toBuffer(),
                verse.toBuffer()
            ], program.programId);
            // Open 50 positions (maximum allowed)
            for (let i = 0; i < 50; i++) {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(10 * Math.pow(10, 6)), // 10 USDC each
                    leverage: new bn_js_1.default(5),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([maxPositionUser])
                    .rpc();
            }
            // Verify all positions created
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            chai_1.assert.equal(userMap.positions.length, 50);
            // Try to open 51st position
            try {
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(10 * Math.pow(10, 6)),
                    leverage: new bn_js_1.default(5),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([maxPositionUser])
                    .rpc();
                chai_1.assert.fail("Should have rejected 51st position");
            }
            catch (err) {
                chai_1.assert.include(err.toString(), "TooManyPositions");
            }
        }));
    });
});
// Helper function to get associated token address
function getAssociatedTokenAddress(mint, owner) {
    return __awaiter(this, void 0, void 0, function* () {
        const [address] = web3_js_1.PublicKey.findProgramAddressSync([owner.toBuffer(), spl_token_1.TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()], new web3_js_1.PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        return address;
    });
}
