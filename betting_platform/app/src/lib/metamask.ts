import { ethers } from 'ethers';

export interface MetaMaskProvider {
  isMetaMask?: boolean;
  request: (args: any) => Promise<any>;
  on: (event: string, handler: (...args: any[]) => void) => void;
  removeListener: (event: string, handler: (...args: any[]) => void) => void;
}

declare global {
  interface Window {
    ethereum?: MetaMaskProvider;
  }
}

export interface WalletState {
  isConnected: boolean;
  address: string | null;
  chainId: number | null;
  balance: string | null;
}

export class MetaMaskWallet {
  private provider: ethers.providers.Web3Provider | null = null;
  private signer: ethers.Signer | null = null;
  private accountsChangedHandler: ((accounts: string[]) => void) | null = null;
  private chainChangedHandler: ((chainId: string) => void) | null = null;
  
  /**
   * Check if MetaMask is installed
   */
  isInstalled(): boolean {
    return typeof window !== 'undefined' && Boolean(window.ethereum?.isMetaMask);
  }

  /**
   * Connect to MetaMask
   */
  async connect(): Promise<WalletState> {
    if (!this.isInstalled()) {
      throw new Error('MetaMask is not installed');
    }

    try {
      // Request account access
      const accounts = await window.ethereum!.request({ 
        method: 'eth_requestAccounts' 
      });

      if (accounts.length === 0) {
        throw new Error('No accounts found');
      }

      // Initialize provider and signer
      this.provider = new ethers.providers.Web3Provider(window.ethereum as any);
      this.signer = this.provider.getSigner();

      // Get wallet state
      const address = accounts[0];
      const network = await this.provider.getNetwork();
      const balance = await this.provider.getBalance(address);

      return {
        isConnected: true,
        address,
        chainId: network.chainId,
        balance: ethers.utils.formatEther(balance)
      };
    } catch (error: any) {
      if (error.code === 4001) {
        throw new Error('User rejected connection');
      }
      throw error;
    }
  }

  /**
   * Disconnect wallet (clears local state)
   */
  disconnect(): void {
    this.provider = null;
    this.signer = null;
  }

  /**
   * Get current connected account
   */
  async getAccount(): Promise<string | null> {
    if (!this.isInstalled()) return null;
    if (!this.provider) {
      this.provider = new ethers.providers.Web3Provider(window.ethereum as any);
    }
    
    try {
      const accounts = await this.provider.listAccounts();
      return accounts[0] || null;
    } catch {
      return null;
    }
  }

  /**
   * Switch to Polygon network
   */
  async switchToPolygon(): Promise<void> {
    if (!window.ethereum) throw new Error('MetaMask not installed');

    const polygonChainId = '0x89'; // 137 in hex

    try {
      await window.ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: polygonChainId }],
      });
    } catch (error: any) {
      // Chain not added to MetaMask
      if (error.code === 4902) {
        await this.addPolygonNetwork();
      } else {
        throw error;
      }
    }
  }

  /**
   * Add Polygon network to MetaMask
   */
  private async addPolygonNetwork(): Promise<void> {
    await window.ethereum!.request({
      method: 'wallet_addEthereumChain',
      params: [{
        chainId: '0x89',
        chainName: 'Polygon Mainnet',
        nativeCurrency: {
          name: 'MATIC',
          symbol: 'MATIC',
          decimals: 18
        },
        rpcUrls: ['https://polygon-rpc.com/'],
        blockExplorerUrls: ['https://polygonscan.com/']
      }]
    });
  }

  /**
   * Sign typed data (EIP-712)
   */
  async signTypedData(typedData: any): Promise<string> {
    if (!this.signer) {
      throw new Error('Wallet not connected');
    }

    const address = await this.signer.getAddress();
    
    // MetaMask expects the data in a specific format
    const signature = await window.ethereum!.request({
      method: 'eth_signTypedData_v4',
      params: [address, JSON.stringify(typedData)]
    });

    return signature;
  }

  /**
   * Get USDC balance on Polygon
   */
  async getUSDCBalance(): Promise<string> {
    if (!this.provider || !this.signer) {
      throw new Error('Wallet not connected');
    }

    const USDC_ADDRESS = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174'; // USDC on Polygon
    const USDC_ABI = [
      'function balanceOf(address owner) view returns (uint256)',
      'function decimals() view returns (uint8)'
    ];

    const usdcContract = new ethers.Contract(USDC_ADDRESS, USDC_ABI, this.provider);
    const address = await this.signer.getAddress();
    
    const [balance, decimals] = await Promise.all([
      usdcContract.balanceOf(address),
      usdcContract.decimals()
    ]);

    return ethers.utils.formatUnits(balance, decimals);
  }

  /**
   * Approve USDC spending
   */
  async approveUSDC(spender: string, amount: string): Promise<string> {
    if (!this.signer) {
      throw new Error('Wallet not connected');
    }

    const USDC_ADDRESS = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174';
    const USDC_ABI = [
      'function approve(address spender, uint256 amount) returns (bool)',
      'function decimals() view returns (uint8)'
    ];

    const usdcContract = new ethers.Contract(USDC_ADDRESS, USDC_ABI, this.signer);
    const decimals = await usdcContract.decimals();
    const amountWei = ethers.utils.parseUnits(amount, decimals);

    const tx = await usdcContract.approve(spender, amountWei);
    await tx.wait();

    return tx.hash;
  }

  /**
   * Listen to account changes
   */
  onAccountsChanged(callback: (accounts: string[]) => void): void {
    if (window.ethereum) {
      if (this.accountsChangedHandler) {
        window.ethereum.removeListener('accountsChanged', this.accountsChangedHandler);
      }
      this.accountsChangedHandler = callback;
      window.ethereum.on('accountsChanged', callback);
    }
  }

  /**
   * Listen to chain changes
   */
  onChainChanged(callback: (chainId: string) => void): void {
    if (window.ethereum) {
      if (this.chainChangedHandler) {
        window.ethereum.removeListener('chainChanged', this.chainChangedHandler);
      }
      this.chainChangedHandler = callback;
      window.ethereum.on('chainChanged', callback);
    }
  }

  /**
   * Remove all listeners
   */
  removeAllListeners(): void {
    if (window.ethereum) {
      if (this.accountsChangedHandler) {
        window.ethereum.removeListener('accountsChanged', this.accountsChangedHandler);
        this.accountsChangedHandler = null;
      }
      if (this.chainChangedHandler) {
        window.ethereum.removeListener('chainChanged', this.chainChangedHandler);
        this.chainChangedHandler = null;
      }
    }
  }
}

// Singleton instance
export const metamask = new MetaMaskWallet();
