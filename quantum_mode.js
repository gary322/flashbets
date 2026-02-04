/**
 * Quantum Mode Module
 * Implements quantum superposition betting calculations
 * Based on the quantum mechanics principles in the Rust codebase
 */

// Quantum constants
const PLANCK_CONSTANT = 6.62607015e-34;
const SQRT_2 = Math.sqrt(2);
const QUANTUM_THRESHOLD = 0.95; // Measurement threshold

/**
 * Quantum state representation
 */
export class QuantumState {
    constructor(outcomes) {
        this.outcomes = outcomes;
        this.amplitudes = this.calculateAmplitudes(outcomes);
        this.phase = new Array(outcomes.length).fill(0);
        this.entangled = false;
        this.measured = false;
        this.coherence = 1.0;
    }

    /**
     * Calculate quantum amplitudes from probabilities
     * |Ψ⟩ = Σ √p_i |outcome_i⟩
     */
    calculateAmplitudes(outcomes) {
        const amplitudes = outcomes.map(outcome => {
            // Amplitude = square root of probability
            return Math.sqrt(outcome.probability / 10000); // Convert from basis points
        });

        // Normalize to ensure Σ|α_i|² = 1
        const sumSquared = amplitudes.reduce((sum, amp) => sum + amp * amp, 0);
        const normFactor = Math.sqrt(sumSquared);
        
        return amplitudes.map(amp => amp / normFactor);
    }

    /**
     * Get quantum state representation
     */
    getStateVector() {
        if (this.measured) {
            return `|${this.outcomes[this.measuredOutcome].name}⟩`;
        }

        const terms = this.outcomes.map((outcome, i) => {
            const amp = this.amplitudes[i];
            const phase = this.phase[i];
            
            // Format amplitude with phase
            let coefficient = amp.toFixed(3);
            if (phase !== 0) {
                coefficient += `e^(i${phase.toFixed(2)})`;
            }
            
            return `${coefficient}|${outcome.name}⟩`;
        });

        return `|Ψ⟩ = ${terms.join(' + ')}`;
    }

    /**
     * Apply quantum gate operation
     */
    applyGate(gate) {
        if (this.measured) {
            throw new Error('Cannot apply gate to measured state');
        }

        switch (gate) {
            case 'hadamard':
                this.applyHadamard();
                break;
            case 'phase':
                this.applyPhaseShift(Math.PI / 4);
                break;
            case 'rotation':
                this.applyRotation(Math.PI / 6);
                break;
        }
    }

    /**
     * Apply Hadamard gate
     */
    applyHadamard() {
        if (this.outcomes.length !== 2) {
            throw new Error('Hadamard gate only applies to binary outcomes');
        }

        const [a0, a1] = this.amplitudes;
        this.amplitudes[0] = (a0 + a1) / SQRT_2;
        this.amplitudes[1] = (a0 - a1) / SQRT_2;
    }

    /**
     * Apply phase shift
     */
    applyPhaseShift(theta) {
        this.phase = this.phase.map(p => p + theta);
    }

    /**
     * Apply rotation
     */
    applyRotation(theta) {
        if (this.outcomes.length !== 2) return;

        const [a0, a1] = this.amplitudes;
        this.amplitudes[0] = Math.cos(theta) * a0 - Math.sin(theta) * a1;
        this.amplitudes[1] = Math.sin(theta) * a0 + Math.cos(theta) * a1;
    }

    /**
     * Measure the quantum state (collapse)
     */
    measure() {
        if (this.measured) return this.measuredOutcome;

        // Calculate cumulative probabilities
        const probabilities = this.amplitudes.map(amp => amp * amp);
        const cumulative = [];
        let sum = 0;
        
        for (const prob of probabilities) {
            sum += prob;
            cumulative.push(sum);
        }

        // Random measurement
        const random = Math.random();
        const outcome = cumulative.findIndex(cum => random <= cum);

        this.measured = true;
        this.measuredOutcome = outcome;
        this.coherence = 0;

        return outcome;
    }

    /**
     * Calculate decoherence over time
     */
    updateCoherence(deltaTime) {
        if (!this.measured) {
            // Exponential decay of coherence
            const decayRate = 0.1; // Per second
            this.coherence *= Math.exp(-decayRate * deltaTime);
            
            // Force measurement if coherence too low
            if (this.coherence < 0.1) {
                this.measure();
            }
        }
    }
}

/**
 * Quantum betting calculator
 */
export class QuantumBettingCalculator {
    constructor() {
        this.states = new Map();
        this.entanglements = new Map();
    }

    /**
     * Create quantum position
     */
    createQuantumPosition(marketId, outcomes, amount, leverage) {
        const state = new QuantumState(outcomes);
        
        const position = {
            marketId,
            state,
            amount,
            leverage,
            created: Date.now(),
            potentialPayouts: this.calculatePotentialPayouts(state, amount, leverage)
        };

        this.states.set(marketId, position);
        return position;
    }

    /**
     * Calculate potential payouts for each outcome
     */
    calculatePotentialPayouts(state, amount, leverage) {
        return state.outcomes.map((outcome, i) => {
            const amplitude = state.amplitudes[i];
            const probability = amplitude * amplitude;
            
            // Quantum enhancement factor
            const quantumFactor = this.calculateQuantumEnhancement(probability);
            
            // Base payout
            const odds = 1 / (outcome.price || 0.5);
            const basePayout = amount * leverage * odds;
            
            // Apply quantum enhancement
            const quantumPayout = basePayout * quantumFactor;
            
            return {
                outcome: outcome.name,
                probability: probability,
                amplitude: amplitude,
                basePayout: basePayout,
                quantumPayout: quantumPayout,
                enhancement: quantumFactor
            };
        });
    }

    /**
     * Calculate quantum enhancement factor
     */
    calculateQuantumEnhancement(probability) {
        // Enhancement based on superposition principle
        // Maximum enhancement when probability is most uncertain (0.5)
        const uncertainty = 4 * probability * (1 - probability);
        const baseEnhancement = 1 + uncertainty * 0.2; // Up to 20% enhancement
        
        // Additional enhancement for true superposition
        const superpositionBonus = Math.sqrt(uncertainty) * 0.1;
        
        return baseEnhancement + superpositionBonus;
    }

    /**
     * Entangle two positions
     */
    entanglePositions(marketId1, marketId2) {
        const position1 = this.states.get(marketId1);
        const position2 = this.states.get(marketId2);
        
        if (!position1 || !position2) {
            throw new Error('Both positions must exist to entangle');
        }

        if (position1.state.measured || position2.state.measured) {
            throw new Error('Cannot entangle measured states');
        }

        // Create entanglement
        const entanglement = {
            positions: [marketId1, marketId2],
            strength: 1.0,
            created: Date.now()
        };

        position1.state.entangled = true;
        position2.state.entangled = true;

        this.entanglements.set(`${marketId1}-${marketId2}`, entanglement);
        
        return entanglement;
    }

    /**
     * Resolve quantum position
     */
    resolvePosition(marketId, actualOutcome) {
        const position = this.states.get(marketId);
        if (!position) return null;

        const state = position.state;
        
        // If not measured yet, measure now
        let measuredOutcome;
        if (!state.measured) {
            measuredOutcome = state.measure();
        } else {
            measuredOutcome = state.measuredOutcome;
        }

        // Check if quantum measurement matches actual outcome
        const outcomeIndex = state.outcomes.findIndex(o => o.name === actualOutcome);
        const quantumWin = measuredOutcome === outcomeIndex;

        // Calculate payout
        const payoutInfo = position.potentialPayouts[outcomeIndex];
        const finalPayout = quantumWin ? payoutInfo.quantumPayout : 0;

        // Handle entangled positions
        this.resolveEntanglements(marketId);

        return {
            marketId,
            quantumOutcome: state.outcomes[measuredOutcome].name,
            actualOutcome,
            quantumWin,
            amount: position.amount,
            leverage: position.leverage,
            payout: finalPayout,
            quantumEnhancement: payoutInfo.enhancement,
            measurement: {
                amplitude: state.amplitudes[measuredOutcome],
                probability: state.amplitudes[measuredOutcome] ** 2,
                coherence: state.coherence
            }
        };
    }

    /**
     * Resolve entangled positions
     */
    resolveEntanglements(marketId) {
        for (const [key, entanglement] of this.entanglements) {
            if (entanglement.positions.includes(marketId)) {
                // Collapse entangled states
                for (const otherId of entanglement.positions) {
                    if (otherId !== marketId) {
                        const otherPosition = this.states.get(otherId);
                        if (otherPosition && !otherPosition.state.measured) {
                            // Force measurement with correlation
                            otherPosition.state.measure();
                        }
                    }
                }
                
                this.entanglements.delete(key);
            }
        }
    }

    /**
     * Calculate quantum interference
     */
    calculateInterference(marketIds) {
        const positions = marketIds.map(id => this.states.get(id)).filter(p => p);
        
        if (positions.length < 2) return 0;

        // Calculate interference pattern
        let totalInterference = 0;
        
        for (let i = 0; i < positions.length - 1; i++) {
            for (let j = i + 1; j < positions.length; j++) {
                const state1 = positions[i].state;
                const state2 = positions[j].state;
                
                if (!state1.measured && !state2.measured) {
                    // Calculate overlap
                    const overlap = this.calculateStateOverlap(state1, state2);
                    totalInterference += overlap * state1.coherence * state2.coherence;
                }
            }
        }

        return totalInterference;
    }

    /**
     * Calculate overlap between quantum states
     */
    calculateStateOverlap(state1, state2) {
        if (state1.outcomes.length !== state2.outcomes.length) return 0;

        let overlap = 0;
        for (let i = 0; i < state1.amplitudes.length; i++) {
            const phaseDiff = state1.phase[i] - state2.phase[i];
            overlap += state1.amplitudes[i] * state2.amplitudes[i] * Math.cos(phaseDiff);
        }

        return Math.abs(overlap);
    }

    /**
     * Get quantum statistics
     */
    getQuantumStats() {
        const stats = {
            totalPositions: this.states.size,
            superpositionPositions: 0,
            measuredPositions: 0,
            entangledPairs: this.entanglements.size,
            averageCoherence: 0,
            totalQuantumEnhancement: 0
        };

        for (const position of this.states.values()) {
            if (position.state.measured) {
                stats.measuredPositions++;
            } else {
                stats.superpositionPositions++;
                stats.averageCoherence += position.state.coherence;
            }

            // Sum quantum enhancements
            for (const payout of position.potentialPayouts) {
                stats.totalQuantumEnhancement += payout.enhancement - 1;
            }
        }

        if (stats.superpositionPositions > 0) {
            stats.averageCoherence /= stats.superpositionPositions;
        }

        return stats;
    }

    /**
     * Visualize quantum state
     */
    visualizeState(marketId) {
        const position = this.states.get(marketId);
        if (!position) return null;

        const state = position.state;
        
        return {
            stateVector: state.getStateVector(),
            amplitudes: state.amplitudes.map((amp, i) => ({
                outcome: state.outcomes[i].name,
                amplitude: amp,
                probability: amp * amp,
                phase: state.phase[i]
            })),
            coherence: state.coherence,
            entangled: state.entangled,
            measured: state.measured,
            measuredOutcome: state.measured ? state.outcomes[state.measuredOutcome].name : null
        };
    }

    /**
     * Update all quantum states (decoherence)
     */
    updateQuantumStates(deltaTime) {
        for (const position of this.states.values()) {
            if (!position.state.measured) {
                position.state.updateCoherence(deltaTime);
            }
        }
    }
}

// Export singleton instance
export const quantumCalculator = new QuantumBettingCalculator();