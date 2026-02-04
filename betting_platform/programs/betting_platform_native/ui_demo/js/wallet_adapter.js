// Solana Wallet Adapter Integration

class WalletAdapter {
    constructor() {
        this.wallet = null;
        this.publicKey = null;
        this.connected = false;
        this.walletType = null;
    }

    async connect(walletType = 'phantom') {
        this.walletType = walletType;
        
        switch (walletType) {
            case 'phantom':
                return this.connectPhantom();
            case 'solflare':
                return this.connectSolflare();
            case 'demo':
                return this.connectDemo();
            default:
                throw new Error(`Unsupported wallet: ${walletType}`);
        }
    }

    async connectPhantom() {
        if ('phantom' in window) {
            const phantom = window.phantom?.solana;
            
            if (phantom?.isPhantom) {
                try {
                    const response = await phantom.connect();
                    this.wallet = phantom;
                    this.publicKey = response.publicKey.toString();
                    this.connected = true;
                    
                    // Listen for disconnect
                    phantom.on('disconnect', () => {
                        this.disconnect();
                    });
                    
                    return {
                        publicKey: this.publicKey,
                        walletType: 'phantom'
                    };
                } catch (err) {
                    throw new Error(`Failed to connect Phantom: ${err.message}`);
                }
            }
        }
        
        throw new Error('Phantom wallet not found. Please install Phantom extension.');
    }

    async connectSolflare() {
        if ('solflare' in window) {
            const solflare = window.solflare;
            
            if (solflare?.isSolflare) {
                try {
                    await solflare.connect();
                    this.wallet = solflare;
                    this.publicKey = solflare.publicKey.toString();
                    this.connected = true;
                    
                    return {
                        publicKey: this.publicKey,
                        walletType: 'solflare'
                    };
                } catch (err) {
                    throw new Error(`Failed to connect Solflare: ${err.message}`);
                }
            }
        }
        
        throw new Error('Solflare wallet not found. Please install Solflare extension.');
    }

    async connectDemo() {
        // Create demo wallet via API
        try {
            const response = await window.bettingAPI.createDemoAccount();
            
            this.publicKey = response.wallet;
            this.connected = true;
            this.walletType = 'demo';
            
            // Store demo wallet in localStorage
            localStorage.setItem('demoWallet', this.publicKey);
            
            return {
                publicKey: this.publicKey,
                walletType: 'demo'
            };
        } catch (err) {
            throw new Error(`Failed to create demo account: ${err.message}`);
        }
    }

    disconnect() {
        if (this.wallet && this.wallet.disconnect) {
            this.wallet.disconnect();
        }
        
        this.wallet = null;
        this.publicKey = null;
        this.connected = false;
        this.walletType = null;
        
        // Clear demo wallet
        localStorage.removeItem('demoWallet');
    }

    async signTransaction(transaction) {
        if (!this.connected) {
            throw new Error('Wallet not connected');
        }
        
        if (this.walletType === 'demo') {
            // Demo transactions are signed server-side
            return transaction;
        }
        
        return this.wallet.signTransaction(transaction);
    }

    async signAllTransactions(transactions) {
        if (!this.connected) {
            throw new Error('Wallet not connected');
        }
        
        if (this.walletType === 'demo') {
            return transactions;
        }
        
        return this.wallet.signAllTransactions(transactions);
    }

    async signMessage(message) {
        if (!this.connected) {
            throw new Error('Wallet not connected');
        }
        
        if (this.walletType === 'demo') {
            // Return mock signature for demo
            return new Uint8Array(64);
        }
        
        return this.wallet.signMessage(message);
    }

    // Helper method to check if wallet is installed
    static isWalletInstalled(walletType) {
        switch (walletType) {
            case 'phantom':
                return window.phantom?.solana?.isPhantom || false;
            case 'solflare':
                return window.solflare?.isSolflare || false;
            case 'demo':
                return true; // Always available
            default:
                return false;
        }
    }

    // Get wallet balance
    async getBalance() {
        if (!this.connected || !this.publicKey) {
            throw new Error('Wallet not connected');
        }
        
        return window.bettingAPI.getBalance(this.publicKey);
    }

    // Get positions
    async getPositions() {
        if (!this.connected || !this.publicKey) {
            throw new Error('Wallet not connected');
        }
        
        return window.bettingAPI.getPositions(this.publicKey);
    }

    // Place trade
    async placeTrade(marketId, amount, outcome, leverage) {
        if (!this.connected || !this.publicKey) {
            throw new Error('Wallet not connected');
        }
        
        return window.bettingAPI.placeTrade({
            market_id: marketId,
            amount,
            outcome,
            leverage,
        });
    }
}

// Create global instance
window.walletAdapter = new WalletAdapter();