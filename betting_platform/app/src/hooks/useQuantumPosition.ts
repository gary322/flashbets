import { useState, useEffect, useCallback } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { QuantumState } from '../components/quantum/QuantumStateDisplay';

interface QuantumPositionData {
  id: string;
  wallet: string;
  marketId: string;
  states: QuantumState[];
  entanglementGroup?: string;
  coherenceTime: number;
  createdAt: number;
  lastMeasured?: number;
  isCollapsed: boolean;
  measurementResult?: QuantumState;
}

interface UseQuantumPositionOptions {
  marketId?: string;
  autoLoad?: boolean;
}

interface UseQuantumPositionResult {
  positions: QuantumPositionData[];
  loading: boolean;
  error: string | null;
  createPosition: (marketId: string, amount: number, leverage: number) => Promise<string>;
  measurePosition: (positionId: string) => Promise<void>;
  refetch: () => Promise<void>;
}

export function useQuantumPosition(options: UseQuantumPositionOptions = {}): UseQuantumPositionResult {
  const { publicKey } = useWallet();
  const { marketId, autoLoad = true } = options;
  const [positions, setPositions] = useState<QuantumPositionData[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchPositions = useCallback(async () => {
    if (!publicKey) return;
    
    setLoading(true);
    setError(null);

    try {
      let url = `/api/quantum/positions/${publicKey.toBase58()}`;
      if (marketId) {
        url += `?market_id=${marketId}`;
      }

      const response = await fetch(url);
      if (!response.ok) {
        throw new Error('Failed to fetch quantum positions');
      }

      const data = await response.json();
      const positionsData = (data.positions || []).map(transformPositionData);
      setPositions(positionsData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load positions');
    } finally {
      setLoading(false);
    }
  }, [publicKey, marketId]);

  const createPosition = useCallback(async (
    marketId: string,
    amount: number,
    leverage: number
  ): Promise<string> => {
    if (!publicKey) {
      throw new Error('Wallet not connected');
    }

    setError(null);

    try {
      const response = await fetch('/api/quantum/create', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          wallet: publicKey.toBase58(),
          market_id: marketId,
          amount,
          leverage,
        }),
      });

      if (!response.ok) {
        throw new Error('Failed to create quantum position');
      }

      const data = await response.json();
      
      // Refetch positions
      await fetchPositions();
      
      return data.position_id;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to create position';
      setError(errorMsg);
      throw err;
    }
  }, [publicKey, fetchPositions]);

  const measurePosition = useCallback(async (positionId: string): Promise<void> => {
    if (!publicKey) {
      throw new Error('Wallet not connected');
    }

    try {
      const response = await fetch(`/api/quantum/measure/${positionId}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          wallet: publicKey.toBase58(),
        }),
      });

      if (!response.ok) {
        throw new Error('Failed to measure quantum position');
      }

      // Refetch positions
      await fetchPositions();
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : 'Failed to measure position';
      setError(errorMsg);
      throw err;
    }
  }, [publicKey, fetchPositions]);

  useEffect(() => {
    if (autoLoad && publicKey) {
      fetchPositions();
    }
  }, [fetchPositions, autoLoad, publicKey]);

  return {
    positions,
    loading,
    error,
    createPosition,
    measurePosition,
    refetch: fetchPositions,
  };
}

// Transform API data to component format
function transformPositionData(apiData: any): QuantumPositionData {
  return {
    id: apiData.id,
    wallet: apiData.wallet,
    marketId: apiData.market_id || apiData.marketId,
    states: (apiData.states || []).map(transformStateData),
    entanglementGroup: apiData.entanglement_group,
    coherenceTime: apiData.coherence_time || 3600,
    createdAt: apiData.created_at,
    lastMeasured: apiData.last_measured,
    isCollapsed: apiData.is_collapsed || false,
    measurementResult: apiData.measurement_result ? 
      transformStateData(apiData.measurement_result) : undefined,
  };
}

function transformStateData(apiData: any): QuantumState {
  return {
    outcome: apiData.outcome,
    amplitude: apiData.amplitude || 0,
    phase: apiData.phase || 0,
    probability: apiData.probability || 0,
    allocation: apiData.allocation || apiData.probability || 0,
  };
}

// Hook for quantum states of a specific market
export function useQuantumStates(marketId: string) {
  const [states, setStates] = useState<QuantumState[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchStates = async () => {
      if (!marketId) return;
      
      setLoading(true);
      setError(null);

      try {
        const response = await fetch(`/api/quantum/states/${marketId}`);
        if (!response.ok) {
          throw new Error('Failed to fetch quantum states');
        }

        const data = await response.json();
        const statesData = (data.states || []).map(transformStateData);
        setStates(statesData);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load states');
      } finally {
        setLoading(false);
      }
    };

    fetchStates();
  }, [marketId]);

  return { states, loading, error };
}

// Hook for quantum statistics
export function useQuantumStats() {
  const { publicKey } = useWallet();
  const [stats, setStats] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const fetchStats = async () => {
      if (!publicKey) return;
      
      setLoading(true);

      try {
        const response = await fetch(`/api/quantum/stats/${publicKey.toBase58()}`);
        if (!response.ok) {
          throw new Error('Failed to fetch quantum stats');
        }

        const data = await response.json();
        setStats(data);
      } catch (err) {
        console.error('Failed to load quantum stats:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchStats();
  }, [publicKey]);

  return { stats, loading };
}
