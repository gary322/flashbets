import { QuantumState } from '../components/quantum/QuantumStateDisplay';

// Quantum constants
export const PLANCK_CONSTANT = 6.62607015e-34;
export const SQRT_2 = Math.sqrt(2);
export const QUANTUM_THRESHOLD = 0.95;
export const DEFAULT_COHERENCE_TIME = 3600; // 1 hour
export const MIN_COHERENCE = 0.1;
export const MAX_ENTANGLED_POSITIONS = 5;

// Calculate quantum amplitudes from probabilities
export function calculateAmplitudes(probabilities: number[]): number[] {
  const amplitudes = probabilities.map(p => Math.sqrt(p));
  
  // Normalize to ensure Σ|α_i|² = 1
  const sumSquared = amplitudes.reduce((sum, amp) => sum + amp * amp, 0);
  const normFactor = Math.sqrt(sumSquared);
  
  return amplitudes.map(amp => amp / normFactor);
}

// Calculate quantum enhancement factor
export function calculateQuantumEnhancement(states: QuantumState[]): number {
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
  
  // Additional enhancement for true superposition (3+ states)
  const superpositionBonus = states.length >= 3 ? 0.05 : 0;
  
  return baseEnhancement + superpositionBonus;
}

// Calculate interference between quantum states
export function calculateInterference(
  state1: QuantumState, 
  state2: QuantumState
): number {
  // Calculate overlap
  const phaseDiff = state1.phase - state2.phase;
  const overlap = state1.amplitude * state2.amplitude * Math.cos(phaseDiff);
  
  return Math.abs(overlap);
}

// Generate quantum state vector notation
export function generateStateVector(states: QuantumState[]): string {
  const terms = states.map(state => {
    const amp = state.amplitude.toFixed(3);
    const phaseStr = state.phase !== 0 ? `e^(i${state.phase.toFixed(2)})` : '';
    return `${amp}${phaseStr}|${state.outcome}⟩`;
  });
  
  return `|Ψ⟩ = ${terms.join(' + ')}`;
}

// Simulate quantum measurement (collapse)
export function measureQuantumState(states: QuantumState[]): {
  outcome: string;
  index: number;
  probability: number;
} {
  // Calculate cumulative probabilities
  const cumulative: number[] = [];
  let sum = 0;
  
  for (const state of states) {
    sum += state.probability;
    cumulative.push(sum);
  }
  
  // Random measurement
  const random = Math.random();
  const index = cumulative.findIndex(cum => random <= cum);
  
  return {
    outcome: states[index].outcome,
    index,
    probability: states[index].probability,
  };
}

// Calculate decoherence factor
export function calculateDecoherence(
  createdAt: number, 
  coherenceTime: number
): number {
  const age = (Date.now() - createdAt) / 1000; // Age in seconds
  return Math.exp(-age / coherenceTime);
}

// Check if position should auto-collapse
export function shouldAutoCollapse(
  createdAt: number, 
  coherenceTime: number
): boolean {
  const coherence = calculateDecoherence(createdAt, coherenceTime);
  return coherence < MIN_COHERENCE;
}

// Calculate entanglement correlation
export function calculateEntanglementCorrelation(
  states1: QuantumState[], 
  states2: QuantumState[]
): number {
  if (states1.length !== states2.length) return 0;
  
  let correlation = 0;
  for (let i = 0; i < states1.length; i++) {
    correlation += states1[i].probability * states2[i].probability;
  }
  
  return correlation;
}

// Format quantum probability as percentage
export function formatQuantumProbability(probability: number): string {
  return `${(probability * 100).toFixed(1)}%`;
}

// Get quantum state color based on probability
export function getQuantumStateColor(probability: number): string {
  // High probability = green, medium = yellow, low = red
  if (probability > 0.6) return '#4CD964';
  if (probability > 0.3) return '#FFD60A';
  return '#FF3B30';
}

// Calculate total quantum leverage
export function calculateQuantumLeverage(
  baseLeverage: number,
  quantumEnhancement: number,
  verseMultiplier: number = 1
): number {
  const totalLeverage = baseLeverage * quantumEnhancement * verseMultiplier;
  return Math.min(totalLeverage, 500); // Cap at 500x
}

// Generate quantum position summary
export function generateQuantumSummary(
  states: QuantumState[],
  amount: number,
  leverage: number
): {
  expectedValue: number;
  maxPayout: number;
  minPayout: number;
  riskScore: number;
} {
  const enhancement = calculateQuantumEnhancement(states);
  const totalExposure = amount * leverage * enhancement;
  
  // Calculate expected value
  const expectedValue = states.reduce((sum, state) => {
    const payout = totalExposure * state.probability;
    return sum + payout;
  }, 0);
  
  // Find max and min payouts
  const payouts = states.map(state => totalExposure * state.probability);
  const maxPayout = Math.max(...payouts);
  const minPayout = Math.min(...payouts);
  
  // Calculate risk score (0-100)
  const variance = states.reduce((sum, state) => {
    const payout = totalExposure * state.probability;
    const diff = payout - expectedValue;
    return sum + (diff * diff * state.probability);
  }, 0);
  
  const stdDev = Math.sqrt(variance);
  const riskScore = Math.min(100, (stdDev / expectedValue) * 100);
  
  return {
    expectedValue,
    maxPayout,
    minPayout,
    riskScore,
  };
}