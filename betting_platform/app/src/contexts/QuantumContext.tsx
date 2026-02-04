import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { QuantumState } from '../components/quantum/QuantumStateDisplay';

interface QuantumPosition {
  id: string;
  marketId: string;
  states: QuantumState[];
  totalAmount: number;
  leverage: number;
  coherenceTime: number;
  createdAt: number;
  isCollapsed: boolean;
  measuredOutcome?: string;
  entangledWith?: string[];
}

interface QuantumContextState {
  // Quantum mode state
  isQuantumEnabled: boolean;
  setQuantumEnabled: (enabled: boolean) => void;
  
  // Quantum positions
  quantumPositions: Map<string, QuantumPosition>;
  activeQuantumPosition: QuantumPosition | null;
  
  // Quantum calculations
  calculateQuantumStates: (outcomes: any[], amount: number) => QuantumState[];
  createQuantumPosition: (marketId: string, amount: number, leverage: number) => Promise<string>;
  collapseQuantumPosition: (positionId: string) => Promise<string>;
  getQuantumEnhancement: (states: QuantumState[]) => number;
  
  // Entanglement
  entanglePositions: (positionId1: string, positionId2: string) => Promise<void>;
  getEntangledPositions: (positionId: string) => QuantumPosition[];
  
  // Loading states
  isLoading: boolean;
  error: string | null;
}

const QuantumContext = createContext<QuantumContextState | undefined>(undefined);

export function useQuantumContext() {
  const context = useContext(QuantumContext);
  if (!context) {
    throw new Error('useQuantumContext must be used within a QuantumProvider');
  }
  return context;
}

interface QuantumProviderProps {
  children: React.ReactNode;
}

// Quantum constants
const PLANCK_CONSTANT = 6.62607015e-34;
const SQRT_2 = Math.sqrt(2);
const DEFAULT_COHERENCE_TIME = 3600; // 1 hour in seconds

export function QuantumProvider({ children }: QuantumProviderProps) {
  const { publicKey } = useWallet();
  const [isQuantumEnabled, setQuantumEnabled] = useState(false);
  const [quantumPositions, setQuantumPositions] = useState<Map<string, QuantumPosition>>(new Map());
  const [activeQuantumPosition, setActiveQuantumPosition] = useState<QuantumPosition | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Calculate quantum states from market outcomes
  const calculateQuantumStates = useCallback((outcomes: any[], amount: number): QuantumState[] => {
    if (!outcomes || outcomes.length === 0) return [];

    // Calculate amplitudes based on outcome probabilities
    const totalProbability = outcomes.reduce((sum, o) => sum + (o.probability || 5000), 0);
    
    const states: QuantumState[] = outcomes.map((outcome, index) => {
      const probability = (outcome.probability || 5000) / totalProbability;
      const amplitude = Math.sqrt(probability);
      const phase = (index * Math.PI) / outcomes.length; // Distribute phases evenly
      
      return {
        outcome: outcome.name,
        amplitude,
        phase,
        probability,
        allocation: probability,
      };
    });

    // Normalize amplitudes
    const sumSquaredAmplitudes = states.reduce((sum, s) => sum + s.amplitude ** 2, 0);
    const normFactor = Math.sqrt(sumSquaredAmplitudes);
    
    return states.map(state => ({
      ...state,
      amplitude: state.amplitude / normFactor,
    }));
  }, []);

  // Create a quantum position
  const createQuantumPosition = useCallback(async (
    marketId: string, 
    amount: number, 
    leverage: number
  ): Promise<string> => {
    if (!publicKey) {
      throw new Error('Wallet not connected');
    }

    setIsLoading(true);
    setError(null);

    try {
      // Call API to create quantum position
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
      
      // Create local quantum position
      const position: QuantumPosition = {
        id: data.position_id,
        marketId,
        states: calculateQuantumStates(data.outcomes, amount),
        totalAmount: amount,
        leverage,
        coherenceTime: DEFAULT_COHERENCE_TIME,
        createdAt: Date.now(),
        isCollapsed: false,
      };

      setQuantumPositions(prev => {
        const next = new Map(prev);
        next.set(position.id, position);
        return next;
      });

      setActiveQuantumPosition(position);
      
      return position.id;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create quantum position');
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, [publicKey, calculateQuantumStates]);

  // Collapse a quantum position (measure it)
  const collapseQuantumPosition = useCallback(async (positionId: string): Promise<string> => {
    const position = quantumPositions.get(positionId);
    if (!position) {
      throw new Error('Position not found');
    }

    if (position.isCollapsed) {
      return position.measuredOutcome!;
    }

    // Calculate cumulative probabilities
    const cumulative: number[] = [];
    let sum = 0;
    
    for (const state of position.states) {
      sum += state.probability;
      cumulative.push(sum);
    }

    // Random measurement
    const random = Math.random();
    const outcomeIndex = cumulative.findIndex(cum => random <= cum);
    const measuredOutcome = position.states[outcomeIndex].outcome;

    // Update position
    setQuantumPositions(prev => {
      const next = new Map(prev);
      const updatedPosition = {
        ...position,
        isCollapsed: true,
        measuredOutcome,
      };
      next.set(positionId, updatedPosition);
      return next;
    });

    // Handle entangled positions
    if (position.entangledWith) {
      for (const entangledId of position.entangledWith) {
        await collapseQuantumPosition(entangledId);
      }
    }

    return measuredOutcome;
  }, [quantumPositions]);

  // Calculate quantum enhancement factor
  const getQuantumEnhancement = useCallback((states: QuantumState[]): number => {
    if (states.length <= 1) return 1;

    // Calculate Shannon entropy
    const entropy = states.reduce((sum, state) => {
      if (state.probability > 0) {
        return sum - state.probability * Math.log2(state.probability);
      }
      return sum;
    }, 0);

    // Maximum entropy for uniform distribution
    const maxEntropy = Math.log2(states.length);
    
    // Enhancement based on entropy (more uncertainty = more enhancement)
    const uncertaintyFactor = entropy / maxEntropy;
    const baseEnhancement = 1 + uncertaintyFactor * 0.15; // Up to 15% enhancement
    
    // Additional enhancement for true superposition
    const superpositionBonus = states.length > 2 ? 0.05 : 0;
    
    return baseEnhancement + superpositionBonus;
  }, []);

  // Entangle two positions
  const entanglePositions = useCallback(async (
    positionId1: string, 
    positionId2: string
  ): Promise<void> => {
    const position1 = quantumPositions.get(positionId1);
    const position2 = quantumPositions.get(positionId2);
    
    if (!position1 || !position2) {
      throw new Error('Both positions must exist');
    }

    if (position1.isCollapsed || position2.isCollapsed) {
      throw new Error('Cannot entangle collapsed positions');
    }

    // Update positions with entanglement
    setQuantumPositions(prev => {
      const next = new Map(prev);
      
      const updated1 = {
        ...position1,
        entangledWith: [...(position1.entangledWith || []), positionId2],
      };
      
      const updated2 = {
        ...position2,
        entangledWith: [...(position2.entangledWith || []), positionId1],
      };
      
      next.set(positionId1, updated1);
      next.set(positionId2, updated2);
      
      return next;
    });
  }, [quantumPositions]);

  // Get entangled positions
  const getEntangledPositions = useCallback((positionId: string): QuantumPosition[] => {
    const position = quantumPositions.get(positionId);
    if (!position || !position.entangledWith) return [];

    const entangled: QuantumPosition[] = [];
    
    for (const id of position.entangledWith) {
      const entangledPosition = quantumPositions.get(id);
      if (entangledPosition) {
        entangled.push(entangledPosition);
      }
    }
    
    return entangled;
  }, [quantumPositions]);

  // Update coherence over time
  useEffect(() => {
    if (!isQuantumEnabled) return;

    const interval = setInterval(() => {
      setQuantumPositions(prev => {
        const next = new Map(prev);
        
        for (const [id, position] of next) {
          if (!position.isCollapsed) {
            const age = (Date.now() - position.createdAt) / 1000;
            const coherence = Math.exp(-age / position.coherenceTime);
            
            // Auto-collapse if coherence too low
            if (coherence < 0.1) {
              collapseQuantumPosition(id);
            }
          }
        }
        
        return next;
      });
    }, 1000);

    return () => clearInterval(interval);
  }, [isQuantumEnabled, collapseQuantumPosition]);

  const value: QuantumContextState = {
    isQuantumEnabled,
    setQuantumEnabled,
    quantumPositions,
    activeQuantumPosition,
    calculateQuantumStates,
    createQuantumPosition,
    collapseQuantumPosition,
    getQuantumEnhancement,
    entanglePositions,
    getEntangledPositions,
    isLoading,
    error,
  };

  return (
    <QuantumContext.Provider value={value}>
      {children}
    </QuantumContext.Provider>
  );
}
