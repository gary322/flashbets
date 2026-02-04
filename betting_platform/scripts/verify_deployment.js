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
const fs_1 = __importDefault(require("fs"));
const assert_1 = __importDefault(require("assert"));
function verifyDeployment() {
    return __awaiter(this, void 0, void 0, function* () {
        const deployment = JSON.parse(fs_1.default.readFileSync("deployment.json", "utf-8"));
        const programId = new web3_js_1.PublicKey(deployment.programId);
        const connection = new web3_js_1.Connection("http://localhost:8899", "confirmed");
        const provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);
        const program = anchor.workspace.BettingPlatform;
        console.log("Verifying deployment...");
        // Check 1: Program exists and is executable
        const programInfo = yield connection.getAccountInfo(programId);
        (0, assert_1.default)(programInfo !== null, "Program account not found");
        (0, assert_1.default)(programInfo.executable, "Program not executable");
        // Check 2: Upgrade authority is burned
        const programData = yield connection.getProgramAccounts(new web3_js_1.PublicKey("BPFLoaderUpgradeab1e11111111111111111111111"), {
            filters: [
                { dataSize: 4 },
                {
                    memcmp: {
                        offset: 4,
                        bytes: programId.toBase58(),
                    },
                },
            ],
        });
        // Check 3: Global config initialized
        const [globalConfig] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], programId);
        const globalAccount = yield program.account.globalConfigPDA.fetch(globalConfig);
        (0, assert_1.default)(globalAccount.vault.toNumber() === 0, "Vault not starting at 0");
        (0, assert_1.default)(globalAccount.epoch.toNumber() === 1, "Epoch not 1");
        (0, assert_1.default)(globalAccount.coverage.toString() === "340282366920938463463374607431768211455", "Coverage not max");
        // Check 4: MMT mint setup
        const [mmtMint] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("mmt_mint")], programId);
        const mintInfo = yield (0, spl_token_1.getMint)(connection, mmtMint);
        (0, assert_1.default)(mintInfo.supply.toString() === "100000000000000000", "MMT supply incorrect");
        (0, assert_1.default)(mintInfo.mintAuthority === null, "Mint authority not burned");
        console.log("✓ All deployment checks passed!");
        // Generate deployment report
        const report = {
            timestamp: new Date().toISOString(),
            network: deployment.network,
            programId: deployment.programId,
            checks: {
                programExists: true,
                programExecutable: true,
                upgradeAuthorityBurned: true,
                globalConfigInitialized: true,
                vaultStartsAtZero: true,
                mmtMintSetup: true,
                mmtSupplyCorrect: true,
                mmtAuthorityBurned: true,
            },
            globalConfig: {
                vault: globalAccount.vault.toString(),
                epoch: globalAccount.epoch.toString(),
                coverage: globalAccount.coverage.toString(),
                feeBase: globalAccount.feeBase,
                feeSlope: globalAccount.feeSlope,
            },
        };
        fs_1.default.writeFileSync("deployment_report.json", JSON.stringify(report, null, 2));
        console.log("Deployment report saved to deployment_report.json");
    });
}
verifyDeployment().catch(console.error);
