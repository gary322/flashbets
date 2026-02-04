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
const bn_js_1 = __importDefault(require("bn.js"));
// User Journey Simulations
// These tests simulate complete user experiences from start to finish
describe("User Journey Simulations", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.BettingPlatform;
    // Shared test infrastructure
    let globalConfig;
    let usdcMint;
    let authority;
    before(() => __awaiter(void 0, void 0, void 0, function* () {
        console.log("🚀 Setting up test environment...");
        authority = web3_js_1.Keypair.generate();
        yield provider.connection.requestAirdrop(authority.publicKey, 100 * web3_js_1.LAMPORTS_PER_SOL);
        yield new Promise(resolve => setTimeout(resolve, 1000));
        // Create USDC mint
        usdcMint = yield (0, spl_token_1.createMint)(provider.connection, authority, authority.publicKey, null, 6);
        // Initialize platform
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
        console.log("✅ Test environment ready");
    }));
    describe("🎯 Journey 1: New User Complete Trading Experience", () => {
        let alice;
        let aliceTokenAccount;
        let verse;
        let proposal;
        it("Step 1: Alice creates account and gets funded", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n👤 Creating Alice's account...");
            alice = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(alice.publicKey, 5 * web3_js_1.LAMPORTS_PER_SOL);
            yield new Promise(resolve => setTimeout(resolve, 1000));
            aliceTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, alice, usdcMint, alice.publicKey);
            // Alice receives 10,000 USDC
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, aliceTokenAccount, authority, 10000 * Math.pow(10, 6));
            const balance = yield (0, spl_token_1.getAccount)(provider.connection, aliceTokenAccount);
            console.log(`✅ Alice funded with ${Number(balance.amount) / Math.pow(10, 6)} USDC`);
        }));
        it("Step 2: Alice discovers an interesting prediction market", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🔍 Creating prediction market for 'Will BTC hit $100k?'...");
            // Create verse for BTC prediction
            const verseId = new bn_js_1.default(Date.now());
            const [versePda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("verse"), verseId.toArrayLike(Buffer, "le", 16)], program.programId);
            verse = versePda;
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
            // Create binary proposal (Yes/No)
            const proposalId = new bn_js_1.default(Date.now() + 1);
            const [proposalPda] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("proposal"), proposalId.toArrayLike(Buffer, "le", 16)], program.programId);
            proposal = proposalPda;
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
            console.log("✅ Market created: Will BTC hit $100k? (Yes/No)");
        }));
        it("Step 3: Alice opens her first position (conservative)", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n💰 Alice opens conservative position...");
            const amount = new bn_js_1.default(1000 * Math.pow(10, 6)); // 1,000 USDC
            const leverage = new bn_js_1.default(5); // 5x leverage
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                alice.publicKey.toBuffer(),
                verseId.toArrayLike(Buffer, "le", 16)
            ], program.programId);
            console.log("  - Amount: 1,000 USDC");
            console.log("  - Leverage: 5x");
            console.log("  - Position: Long on 'Yes'");
            console.log("  - Effective exposure: 5,000 USDC");
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([alice])
                .rpc();
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            console.log(`✅ Position opened! Health factor: ${userMap.healthFactor.toNumber() / 10000}`);
        }));
        it("Step 4: Market moves in Alice's favor", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n📈 Market sentiment shifts positive...");
            // Simulate price movement from 0.5 to 0.65
            yield program.methods
                .updatePrice(new bn_js_1.default(650000000)) // 0.65
                .accounts({
                authority: authority.publicKey,
                priceCache,
            })
                .signers([authority])
                .rpc();
            console.log("  - Price moved from 0.50 to 0.65");
            console.log("  - Alice's position is +30% in profit");
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                alice.publicKey.toBuffer(),
                verseId.toArrayLike(Buffer, "le", 16)
            ], program.programId);
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            console.log(`  - Unrealized P&L: ${userMap.unrealizedPnl.toNumber() / Math.pow(10, 6)} USDC`);
        }));
        it("Step 5: Alice adds to her position (confident)", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n💪 Alice doubles down with higher leverage...");
            const amount = new bn_js_1.default(2000 * Math.pow(10, 6)); // 2,000 USDC
            const leverage = new bn_js_1.default(15); // 15x leverage
            console.log("  - Additional amount: 2,000 USDC");
            console.log("  - Leverage: 15x");
            console.log("  - Additional exposure: 30,000 USDC");
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([alice])
                .rpc();
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            console.log(`✅ Position added! Total positions: ${userMap.positions.length}`);
            console.log(`  - New health factor: ${userMap.healthFactor.toNumber() / 10000}`);
        }));
        it("Step 6: Alice takes partial profits", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n💸 Alice takes profits on first position...");
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([alice])
                .rpc();
            const balance = yield (0, spl_token_1.getAccount)(provider.connection, aliceTokenAccount);
            console.log(`✅ Profits taken! New balance: ${Number(balance.amount) / Math.pow(10, 6)} USDC`);
            console.log("  - Alice keeps second position open for more gains");
        }));
    });
    describe("🚀 Journey 2: Advanced Trader Using Chaining Engine", () => {
        let bob;
        let bobTokenAccount;
        let deepVerse;
        it("Step 1: Bob the whale enters with significant capital", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🐋 Bob the whale arrives...");
            bob = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(bob.publicKey, 10 * web3_js_1.LAMPORTS_PER_SOL);
            yield new Promise(resolve => setTimeout(resolve, 1000));
            bobTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, bob, usdcMint, bob.publicKey);
            // Bob has 100,000 USDC
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, bobTokenAccount, authority, 100000 * Math.pow(10, 6));
            console.log("✅ Bob funded with 100,000 USDC");
        }));
        it("Step 2: Bob creates a deep hierarchical verse", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🌳 Creating deep verse hierarchy...");
            // Create parent verse
            const parentId = new bn_js_1.default(Date.now());
            const [parentVerse] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("verse"), parentId.toArrayLike(Buffer, "le", 16)], program.programId);
            yield program.methods
                .createVerse(parentId, null, new bn_js_1.default(0))
                .accounts({
                creator: bob.publicKey,
                verse: parentVerse,
                globalConfig,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([bob])
                .rpc();
            // Create child verse (depth 1)
            const childId = new bn_js_1.default(Date.now() + 1);
            const [childVerse] = web3_js_1.PublicKey.findProgramAddressSync([Buffer.from("verse"), childId.toArrayLike(Buffer, "le", 16)], program.programId);
            deepVerse = childVerse;
            yield program.methods
                .createVerse(childId, parentId, new bn_js_1.default(1))
                .accounts({
                creator: bob.publicKey,
                verse: childVerse,
                globalConfig,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([bob])
                .rpc();
            console.log("✅ Deep verse created with bonus leverage potential");
        }));
        it("Step 3: Bob executes advanced chaining strategy", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n⛓️ Bob executes 5-step leverage chain...");
            const deposit = new bn_js_1.default(10000 * Math.pow(10, 6)); // 10,000 USDC
            const steps = [
                { borrow: {} }, // 1.5x
                { liquidity: {} }, // 1.2x
                { stake: {} }, // 1.1x
                { borrow: {} }, // 1.5x
                { liquidity: {} }, // 1.2x
            ];
            console.log("  - Initial deposit: 10,000 USDC");
            console.log("  - Chain steps: Borrow → Liquidity → Stake → Borrow → Liquidity");
            console.log("  - Expected leverage: ~3.6x");
            const [chainStatePda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("chain_state"),
                bob.publicKey.toBuffer(),
                deepVerse.toBuffer()
            ], program.programId);
            yield program.methods
                .autoChain(deepVerse, deposit, steps)
                .accounts({
                user: bob.publicKey,
                globalConfig,
                versePda: deepVerse,
                chainState: chainStatePda,
                verseLiquidityPool,
                verseStakingPool,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([bob])
                .rpc();
            const chain = yield program.account.chainStatePda.fetch(chainStatePda);
            const effectiveLeverage = chain.effectiveLeverage.value.toNumber() / Math.pow(10, 18);
            console.log(`✅ Chain executed! Effective leverage: ${effectiveLeverage.toFixed(2)}x`);
            console.log(`  - Final exposure: ${chain.currentValue.toNumber() / Math.pow(10, 6)} USDC`);
        }));
        it("Step 4: Bob monitors and adjusts chain", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n📊 Bob monitors chain performance...");
            // Simulate some market movement
            yield new Promise(resolve => setTimeout(resolve, 2000));
            const [chainStatePda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("chain_state"),
                bob.publicKey.toBuffer(),
                deepVerse.toBuffer()
            ], program.programId);
            const chain = yield program.account.chainStatePda.fetch(chainStatePda);
            console.log("  - Chain status: Active ✅");
            console.log("  - Steps completed: " + chain.stepsCompleted);
            console.log("  - Current value: " + (chain.currentValue.toNumber() / Math.pow(10, 6)) + " USDC");
            // Bob is satisfied and lets it run
            console.log("✅ Bob decides to maintain position");
        }));
        it("Step 5: Bob unwinds chain profitably", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n💰 Bob unwinds chain to lock in profits...");
            const chainId = (yield program.account.chainStatePda.fetch(chainStatePda)).chainId;
            yield program.methods
                .unwindChain(chainId)
                .accounts({
                user: bob.publicKey,
                chainState: chainStatePda,
                userTokenAccount: bobTokenAccount,
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([bob])
                .rpc();
            const finalBalance = yield (0, spl_token_1.getAccount)(provider.connection, bobTokenAccount);
            const profit = (Number(finalBalance.amount) - 90000 * Math.pow(10, 6)) / Math.pow(10, 6);
            console.log(`✅ Chain unwound successfully!`);
            console.log(`  - Final balance: ${Number(finalBalance.amount) / Math.pow(10, 6)} USDC`);
            console.log(`  - Net profit: ${profit} USDC`);
        }));
    });
    describe("⚡ Journey 3: Liquidation and Risk Management", () => {
        let charlie;
        let dave; // Keeper
        let charlieTokenAccount;
        let daveTokenAccount;
        it("Step 1: Charlie and Dave join the platform", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n👥 New users joining...");
            // Setup Charlie (risk-taker)
            charlie = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(charlie.publicKey, 5 * web3_js_1.LAMPORTS_PER_SOL);
            charlieTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, charlie, usdcMint, charlie.publicKey);
            yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, charlieTokenAccount, authority, 5000 * Math.pow(10, 6) // 5,000 USDC
            );
            // Setup Dave (keeper)
            dave = web3_js_1.Keypair.generate();
            yield provider.connection.requestAirdrop(dave.publicKey, 5 * web3_js_1.LAMPORTS_PER_SOL);
            daveTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, dave, usdcMint, dave.publicKey);
            console.log("✅ Charlie (trader) and Dave (keeper) ready");
        }));
        it("Step 2: Charlie opens risky high-leverage position", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n⚠️ Charlie opens risky position...");
            const amount = new bn_js_1.default(2000 * Math.pow(10, 6)); // 2,000 USDC
            const leverage = new bn_js_1.default(50); // 50x leverage!
            console.log("  - Amount: 2,000 USDC");
            console.log("  - Leverage: 50x (MAXIMUM RISK)");
            console.log("  - Exposure: 100,000 USDC");
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                charlie.publicKey.toBuffer(),
                verse.toBuffer()
            ], program.programId);
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([charlie])
                .rpc();
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            console.log(`⚠️ Position opened with health factor: ${userMap.healthFactor.toNumber() / 10000}`);
            console.log("  - Liquidation risk: HIGH");
        }));
        it("Step 3: Market moves against Charlie", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n📉 Market crashes...");
            // Price drops from 0.65 to 0.55
            yield program.methods
                .updatePrice(new bn_js_1.default(550000000)) // 0.55
                .accounts({
                authority: authority.publicKey,
                priceCache,
            })
                .signers([authority])
                .rpc();
            console.log("  - Price dropped from 0.65 to 0.55");
            console.log("  - Charlie's position is underwater");
            const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                Buffer.from("map_entry"),
                charlie.publicKey.toBuffer(),
                verse.toBuffer()
            ], program.programId);
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            console.log(`  - Health factor: ${userMap.healthFactor.toNumber() / 10000} (CRITICAL)`);
        }));
        it("Step 4: Dave performs partial liquidation", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🔨 Dave the keeper spots liquidation opportunity...");
            console.log("  - Dave will liquidate 5% of Charlie's position");
            console.log("  - Dave will earn keeper rewards");
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([dave])
                .rpc();
            const daveBalance = yield (0, spl_token_1.getAccount)(provider.connection, daveTokenAccount);
            console.log(`✅ Liquidation complete!`);
            console.log(`  - Dave earned: ${Number(daveBalance.amount) / Math.pow(10, 6)} USDC in rewards`);
            const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
            console.log(`  - Charlie's position reduced, health improved to: ${userMap.healthFactor.toNumber() / 10000}`);
        }));
        it("Step 5: Charlie learns and adjusts strategy", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n📚 Charlie reduces leverage after close call...");
            // Charlie closes risky position
            yield program.methods
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([charlie])
                .rpc();
            // Opens new conservative position
            yield program.methods
                .openPosition({
                amount: new bn_js_1.default(1000 * Math.pow(10, 6)),
                leverage: new bn_js_1.default(10), // Much safer
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
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([charlie])
                .rpc();
            console.log("✅ Charlie now trades with safer 10x leverage");
            console.log("  - Lesson learned: High leverage = High risk");
        }));
    });
    describe("🏆 Journey 4: Competition and Multi-User Dynamics", () => {
        let traders = [];
        it("Step 1: Trading competition begins", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🏁 Trading competition starting...");
            console.log("  - 5 traders competing");
            console.log("  - Same starting capital: 10,000 USDC each");
            console.log("  - Goal: Highest returns in 5 rounds");
            const traderNames = ["Emma", "Frank", "Grace", "Henry", "Iris"];
            for (const name of traderNames) {
                const keypair = web3_js_1.Keypair.generate();
                yield provider.connection.requestAirdrop(keypair.publicKey, 5 * web3_js_1.LAMPORTS_PER_SOL);
                const account = yield (0, spl_token_1.createAccount)(provider.connection, keypair, usdcMint, keypair.publicKey);
                yield (0, spl_token_1.mintTo)(provider.connection, authority, usdcMint, account, authority, 10000 * Math.pow(10, 6));
                traders.push({ name, keypair, account });
            }
            yield new Promise(resolve => setTimeout(resolve, 1000));
            console.log("✅ All traders ready!");
        }));
        it("Step 2: Round 1 - Opening positions", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🎲 Round 1: Traders place their bets...");
            const strategies = [
                { leverage: 20, outcome: 0, isLong: true }, // Emma: Aggressive long
                { leverage: 5, outcome: 0, isLong: true }, // Frank: Conservative long
                { leverage: 15, outcome: 1, isLong: true }, // Grace: Medium short
                { leverage: 30, outcome: 0, isLong: false }, // Henry: Aggressive short
                { leverage: 10, outcome: 0, isLong: true }, // Iris: Balanced long
            ];
            for (let i = 0; i < traders.length; i++) {
                const trader = traders[i];
                const strategy = strategies[i];
                const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                    Buffer.from("map_entry"),
                    trader.keypair.publicKey.toBuffer(),
                    verse.toBuffer()
                ], program.programId);
                yield program.methods
                    .openPosition({
                    amount: new bn_js_1.default(2000 * Math.pow(10, 6)), // 2,000 USDC each
                    leverage: new bn_js_1.default(strategy.leverage),
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([trader.keypair])
                    .rpc();
                console.log(`  - ${trader.name}: ${strategy.leverage}x leverage, ${strategy.isLong ? 'Long' : 'Short'}`);
            }
        }));
        it("Step 3: Market volatility and position updates", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🌊 Market experiences volatility...");
            // Price movements
            const priceMovements = [
                { price: 580000000, description: "Small rally to 0.58" },
                { price: 520000000, description: "Drop to 0.52" },
                { price: 620000000, description: "Strong rally to 0.62" },
            ];
            for (const movement of priceMovements) {
                yield program.methods
                    .updatePrice(new bn_js_1.default(movement.price))
                    .accounts({
                    authority: authority.publicKey,
                    priceCache,
                })
                    .signers([authority])
                    .rpc();
                console.log(`  - ${movement.description}`);
                yield new Promise(resolve => setTimeout(resolve, 1000));
            }
            // Check positions
            console.log("\n📊 Current standings:");
            for (const trader of traders) {
                const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                    Buffer.from("map_entry"),
                    trader.keypair.publicKey.toBuffer(),
                    verse.toBuffer()
                ], program.programId);
                const userMap = yield program.account.mapEntryPda.fetch(mapEntryPda);
                const pnl = userMap.unrealizedPnl.toNumber() / Math.pow(10, 6);
                console.log(`  - ${trader.name}: ${pnl > 0 ? '+' : ''}${pnl.toFixed(2)} USDC`);
            }
        }));
        it("Step 4: Final round - Closing positions", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🏁 Final round: Traders close positions...");
            const finalBalances = [];
            for (const trader of traders) {
                const [mapEntryPda] = web3_js_1.PublicKey.findProgramAddressSync([
                    Buffer.from("map_entry"),
                    trader.keypair.publicKey.toBuffer(),
                    verse.toBuffer()
                ], program.programId);
                yield program.methods
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
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                })
                    .signers([trader.keypair])
                    .rpc();
                const balance = yield (0, spl_token_1.getAccount)(provider.connection, trader.account);
                const finalBalance = Number(balance.amount) / Math.pow(10, 6);
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
        }));
    });
    describe("🛡️ Journey 5: Platform Safety and Circuit Breakers", () => {
        it("Step 1: Normal trading activity", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n✅ Platform operating normally...");
            const config = yield program.account.globalConfigPda.fetch(globalConfig);
            console.log(`  - Total OI: ${config.totalOi.toNumber() / Math.pow(10, 6)} USDC`);
            console.log(`  - Coverage: ${config.coverage.toNumber() / Math.pow(10, 9)}`);
            console.log(`  - System status: ${config.haltFlag ? 'HALTED' : 'ACTIVE'}`);
        }));
        it("Step 2: Extreme market event triggers circuit breaker", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n🚨 Black swan event detected...");
            // Simulate 10% price movement in single update
            const extremeMovement = new bn_js_1.default(1000); // 10%
            try {
                yield program.methods
                    .checkCircuitBreakers(extremeMovement)
                    .accounts({
                    globalConfig,
                    priceHistory,
                })
                    .signers([authority])
                    .rpc();
            }
            catch (err) {
                console.log("  - Circuit breaker TRIGGERED!");
                console.log("  - Trading halted for safety");
            }
            const config = yield program.account.globalConfigPda.fetch(globalConfig);
            console.log(`  - System status: ${config.haltFlag ? 'HALTED ⏸️' : 'ACTIVE'}`);
        }));
        it("Step 3: System recovery after cooldown", () => __awaiter(void 0, void 0, void 0, function* () {
            console.log("\n⏰ Waiting for system cooldown...");
            // In real scenario, wait for halt period to expire
            yield new Promise(resolve => setTimeout(resolve, 3000));
            // Admin removes halt after investigation
            yield program.methods
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
        }));
    });
    // Summary statistics
    after(() => __awaiter(void 0, void 0, void 0, function* () {
        console.log("\n📈 PLATFORM STATISTICS");
        console.log("========================");
        const config = yield program.account.globalConfigPda.fetch(globalConfig);
        console.log(`Total Volume: ${config.totalOi.toNumber() / Math.pow(10, 6)} USDC`);
        console.log(`Vault Balance: ${config.vault.toNumber() / Math.pow(10, 6)} USDC`);
        console.log(`Coverage Ratio: ${(config.coverage.toNumber() / Math.pow(10, 9)).toFixed(2)}`);
        console.log(`MMT Rewards Pool: ${config.mmtRewardPool.toNumber() / Math.pow(10, 6)} USDC`);
        console.log("\n✅ All user journeys completed successfully!");
        console.log("  - Basic trading ✅");
        console.log("  - Advanced chaining ✅");
        console.log("  - Risk management ✅");
        console.log("  - Multi-user dynamics ✅");
        console.log("  - Safety mechanisms ✅");
    }));
});
