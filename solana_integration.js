/**
 * Solana Integration Module
 * Native Solana blockchain interactions for the betting platform
 * NO ANCHOR - Direct Solana program interactions only
 */

import { 
    Connection, 
    PublicKey, 
    Transaction, 
    SystemProgram,
    TransactionInstruction,
    Keypair,
    LAMPORTS_PER_SOL
} from '@solana/web3.js';
import * as borsh from 'borsh';

// Program ID - Replace with your deployed program ID
const BETTING_PROGRAM_ID = new PublicKey('BeTT1NGPr0GrAmBeTT1NGPr0GrAmBeTT1NGPr0GrAm11');

// PDA Seeds
const VERSE_SEED = Buffer.from('verse');
const MARKET_SEED = Buffer.from('market');
const POSITION_SEED = Buffer.from('position');
const USER_SEED = Buffer.from('user');
const LIQUIDITY_SEED = Buffer.from('liquidity');

// Instruction discriminators
const INSTRUCTIONS = {
    INITIALIZE_VERSE: 0,
    CREATE_MARKET: 1,
    PLACE_BET: 2,
    RESOLVE_MARKET: 3,
    CLAIM_WINNINGS: 4,
    ADD_LIQUIDITY: 5,
    REMOVE_LIQUIDITY: 6,
    UPDATE_ODDS: 7,
    ACTIVATE_QUANTUM: 8
};

// Account structures matching Rust definitions
class VersePDA {
    constructor(fields) {
        this.verse_id = fields.verse_id;
        this.parent_id = fields.parent_id;
        this.children_root = fields.children_root;
        this.quantum_state = fields.quantum_state;
        this.markets = fields.markets;
        this.creator = fields.creator;
        this.create_time = fields.create_time;
        this.level = fields.level;
        this.active = fields.active;
    }
}

class MarketPDA {
    constructor(fields) {
        this.market_id = fields.market_id;
        this.verse_id = fields.verse_id;
        this.external_id = fields.external_id;
        this.source = fields.source;
        this.outcomes = fields.outcomes;
        this.liquidity_pool = fields.liquidity_pool;
        this.status = fields.status;
        this.resolution = fields.resolution;
        this.create_time = fields.create_time;
        this.resolve_time = fields.resolve_time;
    }
}

class PositionPDA {
    constructor(fields) {
        this.user = fields.user;
        this.market_id = fields.market_id;
        this.outcome = fields.outcome;
        this.amount = fields.amount;
        this.leverage = fields.leverage;
        this.entry_price = fields.entry_price;
        this.quantum_state = fields.quantum_state;
        this.timestamp = fields.timestamp;
    }
}

// Borsh schemas
const VERSE_SCHEMA = new Map([
    [VersePDA, {
        kind: 'struct',
        fields: [
            ['verse_id', 'u128'],
            ['parent_id', { kind: 'option', type: 'u128' }],
            ['children_root', [32]],
            ['quantum_state', { kind: 'option', type: 'QuantumState' }],
            ['markets', ['publicKey']],
            ['creator', 'publicKey'],
            ['create_time', 'i64'],
            ['level', 'u8'],
            ['active', 'bool']
        ]
    }]
]);

// Solana integration class
export class SolanaIntegration {
    constructor(network = 'devnet') {
        this.network = network;
        this.connection = null;
        this.wallet = null;
    }

    /**
     * Initialize connection to Solana
     */
    async initialize() {
        const endpoint = this.getEndpoint();
        this.connection = new Connection(endpoint, 'confirmed');
        
        // Check connection
        const version = await this.connection.getVersion();
        console.log('Connected to Solana:', version);
        
        return true;
    }

    /**
     * Get RPC endpoint based on network
     */
    getEndpoint() {
        switch (this.network) {
            case 'mainnet':
                return 'https://api.mainnet-beta.solana.com';
            case 'devnet':
                return 'https://api.devnet.solana.com';
            case 'testnet':
                return 'https://api.testnet.solana.com';
            case 'localhost':
                return 'http://localhost:8899';
            default:
                return 'https://api.devnet.solana.com';
        }
    }

    /**
     * Connect wallet (Phantom, Solflare, etc.)
     */
    async connectWallet() {
        if ('solana' in window) {
            const provider = window.solana;
            
            if (provider.isPhantom) {
                try {
                    const response = await provider.connect();
                    this.wallet = response.publicKey;
                    console.log('Wallet connected:', this.wallet.toString());
                    return this.wallet;
                } catch (err) {
                    console.error('Wallet connection failed:', err);
                    throw err;
                }
            }
        } else {
            throw new Error('Solana wallet not found. Please install Phantom or another Solana wallet.');
        }
    }

    /**
     * Disconnect wallet
     */
    async disconnectWallet() {
        if (window.solana && this.wallet) {
            await window.solana.disconnect();
            this.wallet = null;
            console.log('Wallet disconnected');
        }
    }

    /**
     * Get wallet balance
     */
    async getBalance() {
        if (!this.wallet) throw new Error('Wallet not connected');
        
        const balance = await this.connection.getBalance(this.wallet);
        return balance / LAMPORTS_PER_SOL;
    }

    /**
     * Derive PDA for verse
     */
    async deriveVersePDA(verseId) {
        const [pda, bump] = await PublicKey.findProgramAddress(
            [
                VERSE_SEED,
                Buffer.from(verseId.toString())
            ],
            BETTING_PROGRAM_ID
        );
        return { pda, bump };
    }

    /**
     * Derive PDA for market
     */
    async deriveMarketPDA(marketId) {
        const [pda, bump] = await PublicKey.findProgramAddress(
            [
                MARKET_SEED,
                Buffer.from(marketId)
            ],
            BETTING_PROGRAM_ID
        );
        return { pda, bump };
    }

    /**
     * Derive PDA for position
     */
    async derivePositionPDA(user, marketId) {
        const [pda, bump] = await PublicKey.findProgramAddress(
            [
                POSITION_SEED,
                user.toBuffer(),
                Buffer.from(marketId)
            ],
            BETTING_PROGRAM_ID
        );
        return { pda, bump };
    }

    /**
     * Fetch verse account data
     */
    async fetchVerse(verseId) {
        const { pda } = await this.deriveVersePDA(verseId);
        const accountInfo = await this.connection.getAccountInfo(pda);
        
        if (!accountInfo) {
            throw new Error('Verse account not found');
        }

        // Deserialize account data
        const verse = borsh.deserialize(
            VERSE_SCHEMA,
            VersePDA,
            accountInfo.data
        );
        
        return verse;
    }

    /**
     * Create market instruction
     */
    async createMarketInstruction(params) {
        const {
            marketId,
            verseId,
            externalId,
            source,
            outcomes
        } = params;

        const { pda: marketPDA } = await this.deriveMarketPDA(marketId);
        const { pda: versePDA } = await this.deriveVersePDA(verseId);

        // Serialize instruction data
        const data = Buffer.from([
            INSTRUCTIONS.CREATE_MARKET,
            ...Buffer.from(marketId),
            ...Buffer.from(externalId),
            source, // 0 = Polymarket, 1 = Kalshi
            outcomes.length,
            ...outcomes.flatMap(o => Buffer.from(o))
        ]);

        const instruction = new TransactionInstruction({
            keys: [
                { pubkey: this.wallet, isSigner: true, isWritable: true },
                { pubkey: marketPDA, isSigner: false, isWritable: true },
                { pubkey: versePDA, isSigner: false, isWritable: true },
                { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }
            ],
            programId: BETTING_PROGRAM_ID,
            data
        });

        return instruction;
    }

    /**
     * Place bet instruction
     */
    async placeBetInstruction(params) {
        const {
            marketId,
            outcome,
            amount,
            leverage,
            isQuantum
        } = params;

        const { pda: marketPDA } = await this.deriveMarketPDA(marketId);
        const { pda: positionPDA } = await this.derivePositionPDA(this.wallet, marketId);

        // Convert amount to lamports
        const lamports = Math.floor(amount * LAMPORTS_PER_SOL);

        // Serialize instruction data
        const data = Buffer.from([
            INSTRUCTIONS.PLACE_BET,
            outcome,
            ...new BigUint64Array([BigInt(lamports)]).buffer,
            leverage,
            isQuantum ? 1 : 0
        ]);

        const instruction = new TransactionInstruction({
            keys: [
                { pubkey: this.wallet, isSigner: true, isWritable: true },
                { pubkey: positionPDA, isSigner: false, isWritable: true },
                { pubkey: marketPDA, isSigner: false, isWritable: true },
                { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }
            ],
            programId: BETTING_PROGRAM_ID,
            data
        });

        return instruction;
    }

    /**
     * Execute transaction
     */
    async executeTransaction(instructions) {
        if (!this.wallet) throw new Error('Wallet not connected');

        const transaction = new Transaction();
        instructions.forEach(ix => transaction.add(ix));

        // Get recent blockhash
        const { blockhash } = await this.connection.getRecentBlockhash();
        transaction.recentBlockhash = blockhash;
        transaction.feePayer = this.wallet;

        // Sign and send transaction
        const signedTx = await window.solana.signTransaction(transaction);
        const txid = await this.connection.sendRawTransaction(signedTx.serialize());

        // Confirm transaction
        const confirmation = await this.connection.confirmTransaction(txid);
        
        return {
            signature: txid,
            confirmation
        };
    }

    /**
     * Subscribe to market updates
     */
    subscribeToMarket(marketId, callback) {
        this.deriveMarketPDA(marketId).then(({ pda }) => {
            const subscriptionId = this.connection.onAccountChange(
                pda,
                (accountInfo) => {
                    // Deserialize and callback with market data
                    try {
                        const market = this.deserializeMarket(accountInfo.data);
                        callback(market);
                    } catch (err) {
                        console.error('Failed to deserialize market:', err);
                    }
                },
                'confirmed'
            );

            return subscriptionId;
        });
    }

    /**
     * Unsubscribe from account updates
     */
    unsubscribe(subscriptionId) {
        this.connection.removeAccountChangeListener(subscriptionId);
    }

    /**
     * Get all markets in a verse
     */
    async getVerseMarkets(verseId) {
        const verse = await this.fetchVerse(verseId);
        const markets = [];

        for (const marketPubkey of verse.markets) {
            try {
                const accountInfo = await this.connection.getAccountInfo(marketPubkey);
                if (accountInfo) {
                    const market = this.deserializeMarket(accountInfo.data);
                    markets.push(market);
                }
            } catch (err) {
                console.error('Failed to fetch market:', marketPubkey.toString(), err);
            }
        }

        return markets;
    }

    /**
     * Get user positions
     */
    async getUserPositions() {
        if (!this.wallet) throw new Error('Wallet not connected');

        // Use getProgramAccounts to find all positions for user
        const filters = [
            {
                dataSize: 200 // Approximate size of Position account
            },
            {
                memcmp: {
                    offset: 0, // User pubkey at start of account
                    bytes: this.wallet.toBase58()
                }
            }
        ];

        const accounts = await this.connection.getProgramAccounts(
            BETTING_PROGRAM_ID,
            { filters }
        );

        const positions = accounts.map(({ account }) => {
            return this.deserializePosition(account.data);
        });

        return positions;
    }

    /**
     * Deserialize market account data
     */
    deserializeMarket(data) {
        // Implementation depends on exact Rust struct layout
        // This is a simplified version
        return {
            marketId: data.slice(0, 32).toString(),
            status: data[32],
            outcomes: [], // Parse outcomes
            liquidity: new BigUint64Array(data.buffer, 40, 1)[0]
        };
    }

    /**
     * Deserialize position account data
     */
    deserializePosition(data) {
        // Implementation depends on exact Rust struct layout
        return {
            user: new PublicKey(data.slice(0, 32)),
            marketId: data.slice(32, 64).toString(),
            outcome: data[64],
            amount: new BigUint64Array(data.buffer, 65, 1)[0],
            leverage: data[73]
        };
    }

    /**
     * Estimate transaction fee
     */
    async estimateFee(instructions) {
        const transaction = new Transaction();
        instructions.forEach(ix => transaction.add(ix));
        
        const { blockhash } = await this.connection.getRecentBlockhash();
        transaction.recentBlockhash = blockhash;
        transaction.feePayer = this.wallet || Keypair.generate().publicKey;

        const fee = await transaction.getEstimatedFee(this.connection);
        return fee / LAMPORTS_PER_SOL;
    }
}

// Export singleton instance
export const solanaIntegration = new SolanaIntegration();