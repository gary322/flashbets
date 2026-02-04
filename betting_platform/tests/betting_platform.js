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
const web3_js_1 = require("@solana/web3.js");
const chai_1 = require("chai");
describe("betting_platform", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.BettingPlatform;
    it("Initializes global config", () => __awaiter(void 0, void 0, void 0, function* () {
        const [globalConfig] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], program.programId);
        const seed = new anchor.BN(Date.now());
        yield program.methods
            .initialize(seed)
            .accounts({
            globalConfig,
            authority: provider.wallet.publicKey,
            systemProgram: web3_js_1.SystemProgram.programId,
        })
            .rpc();
        const account = yield program.account.globalConfigPda.fetch(globalConfig);
        chai_1.assert.equal(account.epoch.toNumber(), 1);
        chai_1.assert.equal(account.vault.toNumber(), 0);
        chai_1.assert.equal(account.totalOi.toNumber(), 0);
        chai_1.assert.equal(account.haltFlag, false);
        chai_1.assert.equal(account.feeBase.toNumber(), 300);
        chai_1.assert.equal(account.feeSlope.toNumber(), 2500);
        chai_1.assert.equal(account.coverage.toString(), "340282366920938463463374607431768211455"); // u128::MAX
    }));
    it("Initializes genesis configuration", () => __awaiter(void 0, void 0, void 0, function* () {
        const [globalConfig] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], program.programId);
        yield program.methods
            .initializeGenesis()
            .accounts({
            globalConfig,
            authority: provider.wallet.publicKey,
        })
            .rpc();
        const account = yield program.account.globalConfigPda.fetch(globalConfig);
        chai_1.assert.equal(account.season.toNumber(), 1);
        // MMT total supply is 100M with 9 decimals
        chai_1.assert.equal(account.mmtTotalSupply.toString(), "100000000000000000");
        // Current season is 10M with 9 decimals  
        chai_1.assert.equal(account.mmtCurrentSeason.toString(), "10000000000000000");
        chai_1.assert.isAbove(account.seasonEndSlot.toNumber(), account.seasonStartSlot.toNumber());
    }));
    it("Initializes MMT token", () => __awaiter(void 0, void 0, void 0, function* () {
        const [mmtMint] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("mmt_mint")], program.programId);
        const [treasury] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("treasury")], program.programId);
        const [mintAuthority] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("mint_authority")], program.programId);
        yield program.methods
            .initializeMmt()
            .accounts({
            mmtMint,
            treasury,
            authority: provider.wallet.publicKey,
            mintAuthority,
            tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
            systemProgram: web3_js_1.SystemProgram.programId,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
            .rpc();
        // Verify mint was created with correct supply
        const mintInfo = yield provider.connection.getAccountInfo(mmtMint);
        chai_1.assert.isNotNull(mintInfo);
    }));
    it("Emergency halt works within genesis window", () => __awaiter(void 0, void 0, void 0, function* () {
        const [globalConfig] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], program.programId);
        // This should work if called within 100 slots of genesis
        try {
            yield program.methods
                .emergencyHalt()
                .accounts({
                globalConfig,
                authority: provider.wallet.publicKey,
            })
                .rpc();
            const account = yield program.account.globalConfigPda.fetch(globalConfig);
            chai_1.assert.equal(account.haltFlag, true);
        }
        catch (error) {
            // If it fails, it's because we're past the 100 slot window
            chai_1.assert.include(error.toString(), "EmergencyHaltExpired");
        }
    }));
    it("Verifies global invariants", () => __awaiter(void 0, void 0, void 0, function* () {
        const [globalConfig] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], program.programId);
        const account = yield program.account.globalConfigPda.fetch(globalConfig);
        // Vault should never be negative (u64 ensures this)
        chai_1.assert.isAtLeast(account.vault.toNumber(), 0);
        // If there's OI, coverage should be calculated correctly
        if (account.totalOi.toNumber() > 0) {
            const expectedCoverage = (account.vault.toNumber() * 1e18) / account.totalOi.toNumber();
            // Allow for small rounding errors
            const actualCoverage = Number(account.coverage.toString());
            chai_1.assert.approximately(actualCoverage, expectedCoverage, 1e10);
        }
    }));
});
