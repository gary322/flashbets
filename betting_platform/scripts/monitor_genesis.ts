import * as anchor from "@coral-xyz/anchor";
import { PublicKey, Connection } from "@solana/web3.js";

async function monitorGenesis() {
    const connection = new Connection("http://localhost:8899", "confirmed");
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    
    const program = anchor.workspace.BettingPlatform;
    const [globalConfig] = await PublicKey.findProgramAddress(
        [Buffer.from("global_config")],
        program.programId
    );

    const startSlot = await connection.getSlot();
    console.log(`Monitoring genesis from slot ${startSlot}`);

    let lastVault = 0;
    let lastOI = 0;
    let trades = 0;

    setInterval(async () => {
        try {
            const global = await program.account.globalConfigPda.fetch(globalConfig);
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
        } catch (error) {
            console.error("Error fetching global config:", error);
        }
    }, 1000);
}

function calculateMaxLeverage(coverage: any, depth: number, n: number): number {
    // Simplified calculation for monitoring
    const coverageNum = Number(coverage.toString()) / 1e18;
    if (coverageNum === Number.MAX_VALUE) return 0;
    if (coverageNum > 10) return 100;
    if (coverageNum > 5) return 50;
    if (coverageNum > 2) return 20;
    if (coverageNum > 1) return 10;
    return 5;
}

monitorGenesis().catch(console.error);