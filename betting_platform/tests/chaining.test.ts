import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BettingPlatform } from "../target/types/betting_platform";
import { expect } from "chai";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAccount, mintTo } from "@solana/spl-token";

describe("Chaining Engine", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.BettingPlatform as Program<BettingPlatform>;
    
    let versePda: PublicKey;
    let chainStatePda: PublicKey;
    let userWallet: Keypair;
    let globalConfigPda: PublicKey;
    let usdcMint: PublicKey;
    let userTokenAccount: PublicKey;
    let vaultTokenAccount: PublicKey;

    before(async () => {
        // Setup test user
        userWallet = Keypair.generate();
        await airdrop(provider.connection, userWallet.publicKey, 10);

        // Initialize global config
        globalConfigPda = await initializeGlobalConfig(program);
        
        // Create USDC mint
        usdcMint = await createMint(
            provider.connection,
            userWallet,
            provider.wallet.publicKey,
            null,
            6
        );

        // Create token accounts
        userTokenAccount = await createAccount(
            provider.connection,
            userWallet,
            usdcMint,
            userWallet.publicKey
        );

        vaultTokenAccount = await createAccount(
            provider.connection,
            userWallet,
            usdcMint,
            globalConfigPda
        );

        // Mint tokens to user
        await mintTo(
            provider.connection,
            userWallet,
            usdcMint,
            userTokenAccount,
            provider.wallet.publicKey,
            10_000_000_000 // 10,000 USDC
        );

        // Create test verse
        versePda = await createTestVerse(program);
    });

    describe("Basic Chaining", () => {
        it("should create simple 2-step chain", async () => {
            const deposit = 1_000_000; // 1 USDC
            const steps = [
                { borrow: {} },
                { liquidity: {} }
            ];

            // Derive chain state PDA
            [chainStatePda] = await PublicKey.findProgramAddress(
                [
                    Buffer.from("chain_state"),
                    versePda.toBuffer(),
                    userWallet.publicKey.toBuffer()
                ],
                program.programId
            );

            const tx = await program.methods
                .autoChain(versePda, new anchor.BN(deposit), steps)
                .accounts({
                    user: userWallet.publicKey,
                    globalConfig: globalConfigPda,
                    versePda,
                    chainState: chainStatePda,
                    verseLiquidityPool: null,
                    verseStakingPool: null,
                    priceCache: PublicKey.default,
                    userTokenAccount,
                    vaultTokenAccount,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                })
                .signers([userWallet])
                .rpc();

            // Verify chain created
            const chainState = await program.account.chainStatePDA.fetch(chainStatePda);
            expect(chainState.stepsCompleted).to.equal(2);
            expect(chainState.effectiveLeverage.value.toNumber()).to.be.greaterThan(1.5e18);
        });

        it("should enforce maximum 5 steps", async () => {
            const steps = Array(6).fill({ borrow: {} });

            try {
                await program.methods
                    .autoChain(versePda, new anchor.BN(1_000_000), steps)
                    .accounts({
                        user: userWallet.publicKey,
                        globalConfig: globalConfigPda,
                        versePda,
                        chainState: chainStatePda,
                        verseLiquidityPool: null,
                        verseStakingPool: null,
                        priceCache: PublicKey.default,
                        userTokenAccount,
                        vaultTokenAccount,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([userWallet])
                    .rpc();

                expect.fail("Should have thrown TooManySteps error");
            } catch (err: any) {
                expect(err.error.errorCode.code).to.equal("TooManySteps");
            }
        });
    });

    describe("Chain Unwinding", () => {
        it("should unwind chain in reverse order", async () => {
            // Create chain first
            const deposit = 1_000_000;
            const steps = [
                { borrow: {} },
                { stake: {} }
            ];

            const chainId = await createTestChain(
                program,
                userWallet,
                versePda,
                steps,
                deposit
            );

            // Unwind
            const tx = await program.methods
                .unwindChain(chainId)
                .accounts({
                    user: userWallet.publicKey,
                    chainState: chainStatePda,
                    globalConfig: globalConfigPda,
                    userTokenAccount,
                    vaultTokenAccount,
                    tokenProgram: TOKEN_PROGRAM_ID,
                })
                .signers([userWallet])
                .rpc();

            // Verify unwound
            const chainState = await program.account.chainStatePDA.fetch(chainStatePda);
            expect(chainState.status).to.deep.equal({ completed: {} });
        });
    });

    describe("Safety Mechanisms", () => {
        it("should prevent cycles in chain", async () => {
            const steps = [
                { borrow: {} },
                { stake: {} },
                { borrow: {} } // Potential cycle
            ];

            try {
                await program.methods
                    .autoChain(versePda, new anchor.BN(1_000_000), steps)
                    .accounts({
                        user: userWallet.publicKey,
                        globalConfig: globalConfigPda,
                        versePda,
                        chainState: chainStatePda,
                        verseLiquidityPool: null,
                        verseStakingPool: null,
                        priceCache: PublicKey.default,
                        userTokenAccount,
                        vaultTokenAccount,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([userWallet])
                    .rpc();

                expect.fail("Should have thrown ChainCycle error");
            } catch (err: any) {
                expect(err.error.errorCode.code).to.equal("ChainCycle");
            }
        });

        it("should respect coverage limits", async () => {
            // Set low coverage
            await setGlobalCoverage(program, globalConfigPda, 0.5);

            const steps = [
                { borrow: {} },
                { liquidity: {} },
                { stake: {} },
                { borrow: {} },
                { liquidity: {} }
            ];

            try {
                await program.methods
                    .autoChain(versePda, new anchor.BN(10_000_000), steps) // Large deposit
                    .accounts({
                        user: userWallet.publicKey,
                        globalConfig: globalConfigPda,
                        versePda,
                        chainState: chainStatePda,
                        verseLiquidityPool: null,
                        verseStakingPool: null,
                        priceCache: PublicKey.default,
                        userTokenAccount,
                        vaultTokenAccount,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([userWallet])
                    .rpc();

                expect.fail("Should have thrown ExceedsVerseLimit error");
            } catch (err: any) {
                expect(err.error.errorCode.code).to.equal("ExceedsVerseLimit");
            }
        });
    });

    describe("Effective Leverage Calculation", () => {
        it("should calculate correct effective leverage", async () => {
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
                const chainId = await createTestChain(
                    program,
                    userWallet,
                    versePda,
                    testCase.steps,
                    1_000_000
                );

                const chainState = await program.account.chainStatePDA.fetch(chainStatePda);
                const effectiveLev = chainState.effectiveLeverage.value.toNumber() / 1e18;

                expect(effectiveLev).to.be.at.least(testCase.expectedMin);
                expect(effectiveLev).to.be.at.most(testCase.expectedMax);
            }
        });
    });
});

// Helper functions

async function airdrop(connection: any, pubkey: PublicKey, amount: number) {
    const sig = await connection.requestAirdrop(pubkey, amount * 1e9);
    await connection.confirmTransaction(sig);
}

async function initializeGlobalConfig(program: Program<BettingPlatform>): Promise<PublicKey> {
    const [globalConfigPda] = await PublicKey.findProgramAddress(
        [Buffer.from("global_config")],
        program.programId
    );

    try {
        await program.methods
            .initialize(new anchor.BN(Date.now()))
            .accounts({
                globalConfig: globalConfigPda,
                authority: program.provider.wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();
    } catch (e) {
        // Already initialized
    }

    return globalConfigPda;
}

async function createTestVerse(program: Program<BettingPlatform>): Promise<PublicKey> {
    const verseId = new anchor.BN(Date.now());
    const [versePda] = await PublicKey.findProgramAddress(
        [Buffer.from("verse"), verseId.toArrayLike(Buffer, "le", 16)],
        program.programId
    );

    await program.methods
        .createVerse(verseId, null, 0)
        .accounts({
            verse: versePda,
            creator: program.provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
        })
        .rpc();

    return versePda;
}

async function createTestChain(
    program: Program<BettingPlatform>,
    userWallet: Keypair,
    versePda: PublicKey,
    steps: any[],
    deposit: number
): Promise<anchor.BN> {
    const chainId = new anchor.BN(Date.now());
    
    await program.methods
        .autoChain(versePda, new anchor.BN(deposit), steps)
        .accounts({
            user: userWallet.publicKey,
            // ... other accounts
        })
        .signers([userWallet])
        .rpc();

    return chainId;
}

async function setGlobalCoverage(
    program: Program<BettingPlatform>,
    globalConfigPda: PublicKey,
    coverage: number
) {
    // This would require an admin function in the actual program
    // For testing, we'd need to implement a setCoverage instruction
}