// API Configuration
export const API_CONFIG = {
  baseUrl: process.env.NEXT_PUBLIC_API_URL || '',
  wsUrl: process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8081',
  endpoints: {
    health: '/health',
    markets: '/api/markets',
    market: (id: string) => `/api/markets/${id}`,
    verses: '/api/verses',
    verse: (id: string) => `/api/verses/${id}`,
    placeTrade: '/api/trade/place',
    positions: (wallet: string) => `/api/positions/${wallet}`,
    balance: (wallet: string) => `/api/wallet/balance/${wallet}`,
    createDemo: '/api/wallet/demo/create',
    walletChallenge: (wallet: string) => `/api/wallet/challenge/${wallet}`,
    walletVerify: '/api/wallet/verify',
  },
  websocket: {
    reconnectInterval: 5000,
    maxReconnectAttempts: 5,
  }
};

// Helper to make API calls
export async function apiCall<T>(
  endpoint: string,
  options?: RequestInit
): Promise<T> {
  const url = `${API_CONFIG.baseUrl}${endpoint}`;
  
  try {
    const response = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
    });
    
    if (!response.ok) {
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' })) as any;
      throw new Error(errorData.error?.message || errorData.message || `API Error: ${response.status}`);
    }
    
    return await response.json() as T;
  } catch (error) {
    console.error('API call failed:', error);
    throw error;
  }
}
