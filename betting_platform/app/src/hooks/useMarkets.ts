import { useState, useEffect } from 'react';
import { API_CONFIG, apiCall } from '../config/api';
import { Market } from '../ui/types';

interface MarketResponse {
  id: number;
  title: string;
  description: string;
  outcomes: Array<{
    name: string;
    total_stake: number;
  }>;
  total_volume: number;
  total_liquidity: number;
  resolution_time: number;
  verse_id: number;
  amm_type: string;
}

interface MarketsApiResponse {
  count: number;
  markets: MarketResponse[];
  source: string;
}

export function useMarkets() {
  const [markets, setMarkets] = useState<Market[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchMarkets();
  }, []);

  const fetchMarkets = async () => {
    try {
      setLoading(true);
      const response = await apiCall<MarketsApiResponse>(API_CONFIG.endpoints.markets);
      
      // Transform API response to UI Market type
      const transformedMarkets: Market[] = response.markets.map(market => {
        // Calculate price from outcomes (simplified - in real app would be more complex)
        const totalStake = market.outcomes.reduce((sum, o) => sum + o.total_stake, 0);
        const yesStake = market.outcomes.find(o => o.name.toLowerCase() === 'yes')?.total_stake || 
                         market.outcomes[0]?.total_stake || 0;
        const lastPrice = totalStake > 0 ? yesStake / totalStake : 0.5;
        
        // Calculate 24h change (mock for now - would come from API)
        const change24h = (Math.random() - 0.5) * 10;
        
        return {
          id: market.id.toString(),
          name: market.title,
          verseId: market.verse_id.toString(),
          lastPrice,
          volume24h: market.total_volume,
          liquidity: market.total_liquidity,
          change24h,
          volatility: 0.1 + Math.random() * 0.2, // Mock volatility
          resolutionTime: market.resolution_time * 1000, // Convert to milliseconds
        };
      });
      
      setMarkets(transformedMarkets);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch markets:', err);
      setError(err instanceof Error ? err.message : 'Failed to fetch markets');
    } finally {
      setLoading(false);
    }
  };

  return {
    markets,
    loading,
    error,
    refetch: fetchMarkets,
  };
}