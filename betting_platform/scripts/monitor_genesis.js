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
function monitorGenesis() {
    return __awaiter(this, void 0, void 0, function* () {
        const connection = new web3_js_1.Connection("http://localhost:8899", "confirmed");
        const provider = anchor.AnchorProvider.env();
        anchor.setProvider(provider);
        const program = anchor.workspace.BettingPlatform;
        const [globalConfig] = yield web3_js_1.PublicKey.findProgramAddress([Buffer.from("global_config")], program.programId);
        const startSlot = yield connection.getSlot();
        console.log(`Monitoring genesis from slot ${startSlot}`);
        let lastVault = 0;
        let lastOI = 0;
        let trades = 0;
        setInterval(() => __awaiter(this, void 0, void 0, function* () {
            try {
                const global = yield program.account.globalConfigPda.fetch(globalConfig);
                const currentVault = global.vault.toNumber();
                const currentOI = global.totalOi.toNumber();
                if (currentVault > lastVault) {
                    trades++;
                    const feeCollected = currentVault - lastVault;
                    const oiAdded = currentOI - lastOI;
                    console.log(`Trade #${trades}:`);
                    console.log(`  Fees collected: ${feeCollected / 1e9} SOL`);
                    console.log(`  OI added: ${oiAdded / 1e9} SOL`);
                    console.log(`  New coverage: ${global.coverage.toString()}`);
                    console.log(`  Max leverage: ${calculateMaxLeverage(global.coverage, 0, 1)}`);
                    lastVault = currentVault;
                    lastOI = currentOI;
                }
            }
            catch (error) {
                console.error("Error fetching global config:", error);
            }
        }), 1000);
    });
}
function calculateMaxLeverage(coverage, depth, n) {
    // Simplified calculation for monitoring
    const coverageNum = Number(coverage.toString()) / 1e18;
    if (coverageNum === Number.MAX_VALUE)
        return 0;
    if (coverageNum > 10)
        return 100;
    if (coverageNum > 5)
        return 50;
    if (coverageNum > 2)
        return 20;
    if (coverageNum > 1)
        return 10;
    return 5;
}
monitorGenesis().catch(console.error);
