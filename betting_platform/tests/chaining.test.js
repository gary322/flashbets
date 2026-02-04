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
Object.defineProperty(exports, "__esModule", { value: true });
const anchor = __importStar(require("@coral-xyz/anchor"));
const chai_1 = require("chai");
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
describe("Chaining Engine", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.BettingPlatform;
    let versePda;
    let chainStatePda;
    let userWallet;
    let globalConfigPda;
    let usdcMint;
    let userTokenAccount;
    let vaultTokenAccount;
    before(() => __awaiter(void 0, void 0, void 0, function* () {
        // Setup test user
        userWallet = web3_js_1.Keypair.generate();
        yield airdrop(provider.connection, userWallet.publicKey, 10);
        // Initialize global config
        globalConfigPda = yield initializeGlobalConfig(program);
        // Create USDC mint
        usdcMint = yield (0, spl_token_1.createMint)(provider.connection, userWallet, provider.wallet.publicKey, null, 6);
        // Create token accounts
        userTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, userWallet, usdcMint, userWallet.publicKey);
        vaultTokenAccount = yield (0, spl_token_1.createAccount)(provider.connection, userWallet, usdcMint, globalConfigPda);
        // Mint tokens to user
        yield (0, spl_token_1.mintTo)(provider.connection, userWallet, usdcMint, userTokenAccount, provider.wallet.publicKey, 10000000000 // 10,000 USDC
        );
        // Create test verse
        versePda = yield createTestVerse(program);
    }));
    describe("Basic Chaining", () => {
        it("should create simple 2-step chain", () => __awaiter(void 0, void 0, void 0, function* () {
            const deposit = 1000000; // 1 USDC
            const steps = [
                { borrow: {} },
                { liquidity: {} }
            ];
            // Derive chain state PDA
            [chainStatePda] = yield web3_js_1.PublicKey.findProgramAddress([
                Buffer.from("chain_state"),
                versePda.toBuffer(),
                userWallet.publicKey.toBuffer()
            ], program.programId);
            const tx = yield program.methods
                .autoChain(versePda, new anchor.BN(deposit), steps)
                .accounts({
                user: userWallet.publicKey,
                globalConfig: globalConfigPda,
                versePda,
                chainState: chainStatePda,
                verseLiquidityPool: null,
                verseStakingPool: null,
                priceCache: web3_js_1.PublicKey.default,
                userTokenAccount,
                vaultTokenAccount,
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .signers([userWallet])
                .rpc();
            // Verify chain created
            const chainState = yield program.account.chainStatePDA.fetch(chainStatePda);
            (0, chai_1.expect)(chainState.stepsCompleted).to.equal(2);
            (0, chai_1.expect)(chainState.effectiveLeverage.value.toNumber()).to.be.greaterThan(1.5e18);
        }));
        it("should enforce maximum 5 steps", () => __awaiter(void 0, void 0, void 0, function* () {
            const steps = Array(6).fill({ borrow: {} });
            try {
                yield program.methods
                    .autoChain(versePda, new anchor.BN(1000000), steps)
                    .accounts({
                    user: userWallet.publicKey,
                    globalConfig: globalConfigPda,
                    versePda,
                    chainState: chainStatePda,
                    verseLiquidityPool: null,
                    verseStakingPool: null,
                    priceCache: web3_js_1.PublicKey.default,
                    userTokenAccount,
                    vaultTokenAccount,
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([userWallet])
                    .rpc();
                chai_1.expect.fail("Should have thrown TooManySteps error");
            }
            catch (err) {
                (0, chai_1.expect)(err.error.errorCode.code).to.equal("TooManySteps");
            }
        }));
    });
    describe("Chain Unwinding", () => {
        it("should unwind chain in reverse order", () => __awaiter(void 0, void 0, void 0, function* () {
            // Create chain first
            const deposit = 1000000;
            const steps = [
                { borrow: {} },
                { stake: {} }
            ];
            const chainId = yield createTestChain(program, userWallet, versePda, steps, deposit);
            // Unwind
            const tx = yield program.methods
                .unwindChain(chainId)
                .accounts({
                user: userWallet.publicKey,
                chainState: chainStatePda,
                globalConfig: globalConfigPda,
                userTokenAccount,
                vaultTokenAccount,
                tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
            })
                .signers([userWallet])
                .rpc();
            // Verify unwound
            const chainState = yield program.account.chainStatePDA.fetch(chainStatePda);
            (0, chai_1.expect)(chainState.status).to.deep.equal({ completed: {} });
        }));
    });
    describe("Safety Mechanisms", () => {
        it("should prevent cycles in chain", () => __awaiter(void 0, void 0, void 0, function* () {
            const steps = [
                { borrow: {} },
                { stake: {} },
                { borrow: {} } // Potential cycle
            ];
            try {
                yield program.methods
                    .autoChain(versePda, new anchor.BN(1000000), steps)
                    .accounts({
                    user: userWallet.publicKey,
                    globalConfig: globalConfigPda,
                    versePda,
                    chainState: chainStatePda,
                    verseLiquidityPool: null,
                    verseStakingPool: null,
                    priceCache: web3_js_1.PublicKey.default,
                    userTokenAccount,
                    vaultTokenAccount,
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([userWallet])
                    .rpc();
                chai_1.expect.fail("Should have thrown ChainCycle error");
            }
            catch (err) {
                (0, chai_1.expect)(err.error.errorCode.code).to.equal("ChainCycle");
            }
        }));
        it("should respect coverage limits", () => __awaiter(void 0, void 0, void 0, function* () {
            // Set low coverage
            yield setGlobalCoverage(program, globalConfigPda, 0.5);
            const steps = [
                { borrow: {} },
                { liquidity: {} },
                { stake: {} },
                { borrow: {} },
                { liquidity: {} }
            ];
            try {
                yield program.methods
                    .autoChain(versePda, new anchor.BN(10000000), steps) // Large deposit
                    .accounts({
                    user: userWallet.publicKey,
                    globalConfig: globalConfigPda,
                    versePda,
                    chainState: chainStatePda,
                    verseLiquidityPool: null,
                    verseStakingPool: null,
                    priceCache: web3_js_1.PublicKey.default,
                    userTokenAccount,
                    vaultTokenAccount,
                    tokenProgram: spl_token_1.TOKEN_PROGRAM_ID,
                    systemProgram: web3_js_1.SystemProgram.programId,
                })
                    .signers([userWallet])
                    .rpc();
                chai_1.expect.fail("Should have thrown ExceedsVerseLimit error");
            }
            catch (err) {
                (0, chai_1.expect)(err.error.errorCode.code).to.equal("ExceedsVerseLimit");
            }
        }));
    });
    describe("Effective Leverage Calculation", () => {
        it("should calculate correct effective leverage", () => __awaiter(void 0, void 0, void 0, function* () {
            const testCases = [
                {
                    steps: [{ borrow: {} }],
                    expectedMin: 1.4,
                    expectedMax: 1.6
                },
                {
                    steps: [{ borrow: {} }, { liquidity: {} }],
                    expectedMin: 1.7,
                    expectedMax: 1.9
                },
                {
                    steps: [{ borrow: {} }, { liquidity: {} }, { stake: {} }],
                    expectedMin: 1.9,
                    expectedMax: 2.2
                }
            ];
            for (const testCase of testCases) {
                const chainId = yield createTestChain(program, userWallet, versePda, testCase.steps, 1000000);
                const chainState = yield program.account.chainStatePDA.fetch(chainStatePda);
                const effectiveLev = chainState.effectiveLeverage.value.toNumber() / 1e18;
                (0, chai_1.expect)(effectiveLev).to.be.at.least(testCase.expectedMin);
                (0, chai_1.expect)(effectiveLev).to.be.at.most(testCase.expectedMax);
            }
        }));
    });
});
// Helper functions
function airdrop(connection, pubkey, amount) {
    return __awaiter(this, void 0, void 0, function* () {
        const sig = yield connection.requestAirdrop(pubkey, amount * 1e9);
        yield connection.confirmTransaction(sig);
    });
}
function initializeGlobalConfig(program) {
    return __awaiter(this, void 0, void 0, function* () {
        const [globalConfigPda] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], program.programId);
        try {
            yield program.methods
                .initialize(new anchor.BN(Date.now()))
                .accounts({
                globalConfig: globalConfigPda,
                authority: program.provider.wallet.publicKey,
                systemProgram: web3_js_1.SystemProgram.programId,
            })
                .rpc();
        }
        catch (e) {
            // Already initialized
        }
        return globalConfigPda;
    });
}
function createTestVerse(program) {
    return __awaiter(this, void 0, void 0, function* () {
        const verseId = new anchor.BN(Date.now());
        const [versePda] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("verse"), verseId.toArrayLike(Buffer, "le", 16)], program.programId);
        yield program.methods
            .createVerse(verseId, null, 0)
            .accounts({
            verse: versePda,
            creator: program.provider.wallet.publicKey,
            systemProgram: web3_js_1.SystemProgram.programId,
        })
            .rpc();
        return versePda;
    });
}
function createTestChain(program, userWallet, versePda, steps, deposit) {
    return __awaiter(this, void 0, void 0, function* () {
        const chainId = new anchor.BN(Date.now());
        yield program.methods
            .autoChain(versePda, new anchor.BN(deposit), steps)
            .accounts({
            user: userWallet.publicKey,
            // ... other accounts
        })
            .signers([userWallet])
            .rpc();
        return chainId;
    });
}
function setGlobalCoverage(program, globalConfigPda, coverage) {
    return __awaiter(this, void 0, void 0, function* () {
        // This would require an admin function in the actual program
        // For testing, we'd need to implement a setCoverage instruction
    });
}
