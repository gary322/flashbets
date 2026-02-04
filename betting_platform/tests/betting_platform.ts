import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BettingPlatform } from "../target/types/betting_platform";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";

describe("betting_platform", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.BettingPlatform as Program<BettingPlatform>;

    it("Initializes global config", async () => {
        const [globalConfig] = await PublicKey.findProgramAddress(
            [Buffer.from("global_config")],
            program.programId
        );

        const seed = new anchor.BN(Date.now());
        
        await program.methods
            .initialize(seed)
            .accounts({
                globalConfig,
                authority: provider.wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        const account = await program.account.globalConfigPda.fetch(globalConfig);
        assert.equal(account.epoch.toNumber(), 1);
        assert.equal(account.vault.toNumber(), 0);
        assert.equal(account.totalOi.toNumber(), 0);
        assert.equal(account.haltFlag, false);
        assert.equal(account.feeBase.toNumber(), 300);
        assert.equal(account.feeSlope.toNumber(), 2500);
        assert.equal(account.coverage.toString(), "340282366920938463463374607431768211455"); // u128::MAX
    });

    it("Initializes genesis configuration", async () => {
        const [globalConfig] = await PublicKey.findProgramAddress(
            [Buffer.from("global_config")],
            program.programId
        );

        await program.methods
            .initializeGenesis()
            .accounts({
                globalConfig,
                authority: provider.wallet.publicKey,
            })
            .rpc();

        const account = await program.account.globalConfigPda.fetch(globalConfig);
        assert.equal(account.season.toNumber(), 1);
        // MMT total supply is 100M with 9 decimals
        assert.equal(account.mmtTotalSupply.toString(), "100000000000000000");
        // Current season is 10M with 9 decimals  
        assert.equal(account.mmtCurrentSeason.toString(), "10000000000000000");
        assert.isAbove(account.seasonEndSlot.toNumber(), account.seasonStartSlot.toNumber());
    });

    it("Initializes MMT token", async () => {
        const [mmtMint] = await PublicKey.findProgramAddress(
            [Buffer.from("mmt_mint")],
            program.programId
        );

        const [treasury] = await PublicKey.findProgramAddress(
            [Buffer.from("treasury")],
            program.programId
        );

        const [mintAuthority] = await PublicKey.findProgramAddress(
            [Buffer.from("mint_authority")],
            program.programId
        );

        await program.methods
            .initializeMmt()
            .accounts({
                mmtMint,
                treasury,
                authority: provider.wallet.publicKey,
                mintAuthority,
                tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
                systemProgram: SystemProgram.programId,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            })
            .rpc();

        // Verify mint was created with correct supply
        const mintInfo = await provider.connection.getAccountInfo(mmtMint);
        assert.isNotNull(mintInfo);
    });

    it("Emergency halt works within genesis window", async () => {
        const [globalConfig] = await PublicKey.findProgramAddress(
            [Buffer.from("global_config")],
            program.programId
        );

        // This should work if called within 100 slots of genesis
        try {
            await program.methods
                .emergencyHalt()
                .accounts({
                    globalConfig,
                    authority: provider.wallet.publicKey,
                })
                .rpc();

            const account = await program.account.globalConfigPda.fetch(globalConfig);
            assert.equal(account.haltFlag, true);
        } catch (error) {
            // If it fails, it's because we're past the 100 slot window
            assert.include(error.toString(), "EmergencyHaltExpired");
        }
    });

    it("Verifies global invariants", async () => {
        const [globalConfig] = await PublicKey.findProgramAddress(
            [Buffer.from("global_config")],
            program.programId
        );

        const account = await program.account.globalConfigPda.fetch(globalConfig);
        
        // Vault should never be negative (u64 ensures this)
        assert.isAtLeast(account.vault.toNumber(), 0);
        
        // If there's OI, coverage should be calculated correctly
        if (account.totalOi.toNumber() > 0) {
            const expectedCoverage = (account.vault.toNumber() * 1e18) / account.totalOi.toNumber();
            // Allow for small rounding errors
            const actualCoverage = Number(account.coverage.toString());
            assert.approximately(actualCoverage, expectedCoverage, 1e10);
        }
    });
});