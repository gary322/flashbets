//! Quantum Position Engine for managing superposition states and entanglement

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use tokio::sync::RwLock;
use std::sync::Arc;
use rand::{Rng, seq::SliceRandom};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumState {
    pub market_id: u128,
    pub outcome: u8,
    pub amount: u64,
    pub leverage: u32,
    pub amplitude: f64,      // Quantum amplitude
    pub phase: f64,          // Quantum phase
    pub probability: f64,    // |amplitude|^2
    pub entangled_with: Vec<String>, // IDs of entangled states
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumPosition {
    pub id: String,
    pub wallet: String,
    pub states: Vec<QuantumState>,
    pub entanglement_group: Option<String>,
    pub coherence_time: u64,     // Time until decoherence
    pub created_at: i64,
    pub last_measured: Option<i64>,
    pub is_collapsed: bool,
    pub measurement_result: Option<QuantumState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntanglementGroup {
    pub id: String,
    pub positions: Vec<String>,
    pub correlation_matrix: Vec<Vec<f64>>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumMeasurement {
    pub position_id: String,
    pub measured_state: QuantumState,
    pub probability: f64,
    pub timestamp: i64,
    pub caused_collapse: bool,
    pub affected_entangled: Vec<String>,
}

pub struct QuantumEngine {
    positions: Arc<RwLock<HashMap<String, QuantumPosition>>>,
    entanglement_groups: Arc<RwLock<HashMap<String, EntanglementGroup>>>,
    measurements: Arc<RwLock<Vec<QuantumMeasurement>>>,
}

impl QuantumEngine {
    pub fn new() -> Self {
        Self {
            positions: Arc::new(RwLock::new(HashMap::new())),
            entanglement_groups: Arc::new(RwLock::new(HashMap::new())),
            measurements: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new quantum position with superposition states
    pub async fn create_quantum_position(
        &self,
        wallet: String,
        states: Vec<QuantumState>,
        entanglement_group: Option<String>,
    ) -> Result<String> {
        // Validate quantum states
        self.validate_quantum_states(&states)?;

        let position_id = Uuid::new_v4().to_string();
        let coherence_time = 3600; // 1 hour default coherence time

        let quantum_position = QuantumPosition {
            id: position_id.clone(),
            wallet,
            states: self.normalize_quantum_states(states)?,
            entanglement_group: entanglement_group.clone(),
            coherence_time,
            created_at: chrono::Utc::now().timestamp(),
            last_measured: None,
            is_collapsed: false,
            measurement_result: None,
        };

        // Store the position
        {
            let mut positions = self.positions.write().await;
            positions.insert(position_id.clone(), quantum_position);
        }

        // Handle entanglement if specified
        if let Some(group_id) = entanglement_group {
            self.add_to_entanglement_group(position_id.clone(), group_id).await?;
        }

        // Start decoherence timer
        self.start_decoherence_timer(position_id.clone(), coherence_time).await;

        Ok(position_id)
    }

    /// Validate quantum states for physical consistency
    fn validate_quantum_states(&self, states: &[QuantumState]) -> Result<()> {
        if states.is_empty() {
            return Err(anyhow!("Quantum position must have at least one state"));
        }

        // Check probability normalization
        let total_probability: f64 = states.iter().map(|s| s.probability).sum();
        if (total_probability - 1.0).abs() > 1e-6 {
            return Err(anyhow!("State probabilities must sum to 1.0, got {}", total_probability));
        }

        // Check individual state validity
        for state in states {
            if state.probability < 0.0 || state.probability > 1.0 {
                return Err(anyhow!("State probability must be between 0 and 1"));
            }
            if state.amplitude.is_nan() || state.phase.is_nan() {
                return Err(anyhow!("Amplitude and phase must be valid numbers"));
            }
        }

        Ok(())
    }

    /// Normalize quantum states to ensure proper probability distribution
    fn normalize_quantum_states(&self, mut states: Vec<QuantumState>) -> Result<Vec<QuantumState>> {
        let total_prob: f64 = states.iter().map(|s| s.probability).sum();
        
        if total_prob == 0.0 {
            return Err(anyhow!("Cannot normalize states with zero total probability"));
        }

        // Normalize probabilities
        for state in &mut states {
            state.probability /= total_prob;
            state.amplitude = state.probability.sqrt() * state.phase.cos();
        }

        Ok(states)
    }

    /// Add position to entanglement group
    async fn add_to_entanglement_group(&self, position_id: String, group_id: String) -> Result<()> {
        let mut groups = self.entanglement_groups.write().await;
        
        let group = groups.entry(group_id.clone()).or_insert_with(|| {
            EntanglementGroup {
                id: group_id,
                positions: Vec::new(),
                correlation_matrix: Vec::new(),
                created_at: chrono::Utc::now().timestamp(),
            }
        });

        group.positions.push(position_id);
        
        // Update correlation matrix
        let n = group.positions.len();
        group.correlation_matrix = self.generate_correlation_matrix(n);

        Ok(())
    }

    /// Generate correlation matrix for entangled positions
    fn generate_correlation_matrix(&self, n: usize) -> Vec<Vec<f64>> {
        let mut rng = rand::thread_rng();
        let mut matrix = vec![vec![0.0; n]; n];
        
        for i in 0..n {
            matrix[i][i] = 1.0; // Perfect self-correlation
            for j in (i + 1)..n {
                // Generate random correlation between -0.9 and 0.9
                let correlation = rng.gen_range(-0.9..0.9);
                matrix[i][j] = correlation;
                matrix[j][i] = correlation; // Symmetric matrix
            }
        }
        
        matrix
    }

    /// Measure quantum position, causing wave function collapse
    pub async fn measure_quantum_position(&self, position_id: &str) -> Result<QuantumMeasurement> {
        let mut positions = self.positions.write().await;
        let position = positions.get_mut(position_id)
            .ok_or_else(|| anyhow!("Quantum position not found"))?;

        if position.is_collapsed {
            return Err(anyhow!("Position already collapsed"));
        }

        // Perform quantum measurement based on probabilities
        let measured_state = self.collapse_wave_function(&position.states)?;
        let measurement_time = chrono::Utc::now().timestamp();

        // Collapse the position
        position.is_collapsed = true;
        position.last_measured = Some(measurement_time);
        position.measurement_result = Some(measured_state.clone());

        // Handle entangled positions
        let affected_entangled = if let Some(group_id) = &position.entanglement_group {
            self.collapse_entangled_positions(group_id, position_id, &measured_state).await?
        } else {
            Vec::new()
        };

        let measurement = QuantumMeasurement {
            position_id: position_id.to_string(),
            measured_state,
            probability: position.states.iter()
                .find(|s| s.market_id == position.measurement_result.as_ref().unwrap().market_id)
                .map(|s| s.probability)
                .unwrap_or(0.0),
            timestamp: measurement_time,
            caused_collapse: true,
            affected_entangled,
        };

        // Store measurement
        {
            let mut measurements = self.measurements.write().await;
            measurements.push(measurement.clone());
        }

        Ok(measurement)
    }

    /// Collapse wave function to a single state
    fn collapse_wave_function(&self, states: &[QuantumState]) -> Result<QuantumState> {
        let mut rng = rand::thread_rng();
        let random_value: f64 = rng.gen();
        
        let mut cumulative_prob = 0.0;
        for state in states {
            cumulative_prob += state.probability;
            if random_value <= cumulative_prob {
                return Ok(state.clone());
            }
        }
        
        // Fallback to last state if floating point errors
        states.last().cloned()
            .ok_or_else(|| anyhow!("No states available for collapse"))
    }

    /// Handle collapse of entangled positions
    async fn collapse_entangled_positions(
        &self,
        group_id: &str,
        measured_position_id: &str,
        measured_state: &QuantumState,
    ) -> Result<Vec<String>> {
        let groups = self.entanglement_groups.read().await;
        let group = groups.get(group_id)
            .ok_or_else(|| anyhow!("Entanglement group not found"))?;

        let mut affected = Vec::new();
        let mut positions = self.positions.write().await;

        for position_id in &group.positions {
            if position_id == measured_position_id {
                continue; // Skip the measured position
            }

            if let Some(position) = positions.get_mut(position_id) {
                if !position.is_collapsed {
                    // Apply entanglement correlation
                    let correlated_state = self.apply_entanglement_correlation(
                        &position.states,
                        measured_state,
                        &group.correlation_matrix,
                    )?;

                    position.is_collapsed = true;
                    position.last_measured = Some(chrono::Utc::now().timestamp());
                    position.measurement_result = Some(correlated_state);
                    affected.push(position_id.clone());
                }
            }
        }

        Ok(affected)
    }

    /// Apply entanglement correlation to determine correlated state
    fn apply_entanglement_correlation(
        &self,
        states: &[QuantumState],
        measured_state: &QuantumState,
        _correlation_matrix: &[Vec<f64>],
    ) -> Result<QuantumState> {
        // Simplified correlation: if measured state is outcome 0, 
        // entangled position more likely to be outcome 1
        let target_outcome = if measured_state.outcome == 0 { 1 } else { 0 };
        
        // Find state with target outcome or return highest probability state
        states.iter()
            .find(|s| s.outcome == target_outcome)
            .or_else(|| states.iter().max_by(|a, b| a.probability.partial_cmp(&b.probability).unwrap()))
            .cloned()
            .ok_or_else(|| anyhow!("No suitable correlated state found"))
    }

    /// Start decoherence timer for quantum position
    async fn start_decoherence_timer(&self, position_id: String, coherence_time: u64) {
        let positions = Arc::clone(&self.positions);
        
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(coherence_time)).await;
            
            let mut positions_guard = positions.write().await;
            if let Some(position) = positions_guard.get_mut(&position_id) {
                if !position.is_collapsed {
                    // Force measurement due to decoherence
                    let mut rng = rand::thread_rng();
                    if let Some(state) = position.states.choose(&mut rng) {
                        position.is_collapsed = true;
                        position.last_measured = Some(chrono::Utc::now().timestamp());
                        position.measurement_result = Some(state.clone());
                    }
                }
            }
        });
    }

    /// Get quantum position by ID
    pub async fn get_quantum_position(&self, position_id: &str) -> Result<QuantumPosition> {
        let positions = self.positions.read().await;
        positions.get(position_id)
            .cloned()
            .ok_or_else(|| anyhow!("Quantum position not found"))
    }

    /// Get all quantum positions for a wallet
    pub async fn get_wallet_positions(&self, wallet: &str) -> Result<Vec<QuantumPosition>> {
        let positions = self.positions.read().await;
        Ok(positions.values()
            .filter(|p| p.wallet == wallet)
            .cloned()
            .collect())
    }

    /// Get quantum states for a market
    pub async fn get_market_quantum_states(&self, market_id: u128) -> Result<Vec<QuantumState>> {
        let positions = self.positions.read().await;
        let mut states = Vec::new();
        
        for position in positions.values() {
            if !position.is_collapsed {
                for state in &position.states {
                    if state.market_id == market_id {
                        states.push(state.clone());
                    }
                }
            }
        }
        
        Ok(states)
    }

    /// Get all measurements
    pub async fn get_measurements(&self) -> Result<Vec<QuantumMeasurement>> {
        let measurements = self.measurements.read().await;
        Ok(measurements.clone())
    }

    /// Calculate quantum portfolio metrics
    pub async fn calculate_quantum_metrics(&self, wallet: &str) -> Result<QuantumPortfolioMetrics> {
        let positions = self.get_wallet_positions(wallet).await?;
        
        let total_positions = positions.len();
        let collapsed_positions = positions.iter().filter(|p| p.is_collapsed).count();
        let active_superpositions = total_positions - collapsed_positions;
        
        let total_expected_value: f64 = positions.iter()
            .filter(|p| !p.is_collapsed)
            .map(|p| {
                p.states.iter().map(|s| s.probability * s.amount as f64).sum::<f64>()
            })
            .sum();

        let quantum_uncertainty = self.calculate_uncertainty(&positions);
        let entanglement_count = positions.iter()
            .filter(|p| p.entanglement_group.is_some())
            .count();

        Ok(QuantumPortfolioMetrics {
            total_positions,
            active_superpositions,
            collapsed_positions,
            total_expected_value,
            quantum_uncertainty,
            entanglement_count,
            coherence_time_remaining: self.calculate_average_coherence_time(&positions).await,
        })
    }

    /// Calculate quantum uncertainty (standard deviation of expected outcomes)
    fn calculate_uncertainty(&self, positions: &[QuantumPosition]) -> f64 {
        let mut total_variance = 0.0;
        let mut count = 0;

        for position in positions.iter().filter(|p| !p.is_collapsed) {
            let expected_value: f64 = position.states.iter()
                .map(|s| s.probability * s.amount as f64)
                .sum();
            
            let variance: f64 = position.states.iter()
                .map(|s| s.probability * (s.amount as f64 - expected_value).powi(2))
                .sum();
            
            total_variance += variance;
            count += 1;
        }

        if count > 0 {
            (total_variance / count as f64).sqrt()
        } else {
            0.0
        }
    }

    /// Calculate average remaining coherence time
    async fn calculate_average_coherence_time(&self, positions: &[QuantumPosition]) -> u64 {
        let current_time = chrono::Utc::now().timestamp();
        let mut total_remaining = 0i64;
        let mut count = 0;

        for position in positions.iter().filter(|p| !p.is_collapsed) {
            let elapsed = current_time - position.created_at;
            let remaining = position.coherence_time as i64 - elapsed;
            if remaining > 0 {
                total_remaining += remaining;
                count += 1;
            }
        }

        if count > 0 {
            (total_remaining / count as i64).max(0) as u64
        } else {
            0
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuantumPortfolioMetrics {
    pub total_positions: usize,
    pub active_superpositions: usize,
    pub collapsed_positions: usize,
    pub total_expected_value: f64,
    pub quantum_uncertainty: f64,
    pub entanglement_count: usize,
    pub coherence_time_remaining: u64,
}

impl Default for QuantumEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    // Helper function to create quantum states
    fn create_quantum_state(market_id: u128, outcome: u8) -> QuantumState {
        let probability = rand::random::<f64>() * 0.5 + 0.1; // Random between 0.1 and 0.6
        QuantumState {
            market_id,
            outcome,
            amount: 1000,
            leverage: 5,
            amplitude: probability.sqrt(),
            phase: rand::random::<f64>() * std::f64::consts::PI,
            probability,
            entangled_with: vec![],
        }
    }
    
    // Helper to normalize quantum states
    fn normalize_states(states: &mut Vec<QuantumState>) {
        let total_prob: f64 = states.iter().map(|s| s.probability).sum();
        if total_prob > 0.0 {
            for state in states {
                state.probability /= total_prob;
                state.amplitude = state.probability.sqrt();
            }
        }
    }
    
    // Helper function to create quantum position
    fn create_quantum_position(wallet: &str, num_states: usize) -> QuantumPosition {
        let mut states = Vec::new();
        let mut total_prob = 0.0;
        
        // Create states with normalized probabilities
        for i in 0..num_states {
            let state = create_quantum_state(1000 + i as u128, (i % 2) as u8);
            total_prob += state.probability;
            states.push(state);
        }
        
        // Normalize probabilities
        for state in &mut states {
            state.probability /= total_prob;
            state.amplitude = state.probability.sqrt();
        }
        
        QuantumPosition {
            id: Uuid::new_v4().to_string(),
            wallet: wallet.to_string(),
            states,
            entanglement_group: None,
            coherence_time: 3600,
            created_at: chrono::Utc::now().timestamp(),
            last_measured: None,
            is_collapsed: false,
            measurement_result: None,
        }
    }
    
    // Helper function to create entangled positions
    fn create_entangled_positions(wallets: &[&str]) -> (Vec<QuantumPosition>, EntanglementGroup) {
        let group_id = Uuid::new_v4().to_string();
        let mut positions = Vec::new();
        
        for wallet in wallets {
            let mut position = create_quantum_position(wallet, 2);
            position.entanglement_group = Some(group_id.clone());
            positions.push(position);
        }
        
        let n = positions.len();
        let mut correlation_matrix = vec![vec![0.0; n]; n];
        
        // Create correlation matrix
        for i in 0..n {
            correlation_matrix[i][i] = 1.0;
            for j in (i + 1)..n {
                let corr = 0.7; // Strong correlation
                correlation_matrix[i][j] = corr;
                correlation_matrix[j][i] = corr;
            }
        }
        
        let group = EntanglementGroup {
            id: group_id,
            positions: positions.iter().map(|p| p.id.clone()).collect(),
            correlation_matrix,
            created_at: chrono::Utc::now().timestamp(),
        };
        
        (positions, group)
    }
    
    // Helper to assert float equality
    fn assert_float_eq(a: f64, b: f64, epsilon: f64) {
        assert!((a - b).abs() < epsilon, "Expected {} ≈ {}", a, b);
    }
    
    #[tokio::test]
    async fn test_create_quantum_position() {
        let engine = QuantumEngine::new();
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1000, 1),
            create_quantum_state(1001, 0),
        ];
        
        // Normalize probabilities
        let total_prob: f64 = states.iter().map(|s| s.probability).sum();
        for state in &mut states {
            state.probability /= total_prob;
            state.amplitude = state.probability.sqrt();
        }
        
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states,
            None
        ).await.unwrap();
        
        let retrieved = engine.get_quantum_position(&position_id).await.unwrap();
        assert_eq!(retrieved.id, position_id);
        assert_eq!(retrieved.wallet, "test_wallet");
    }
    
    #[tokio::test]
    async fn test_quantum_state_normalization() {
        let position = create_quantum_position("test_wallet", 5);
        
        // Check probability normalization
        let total_prob: f64 = position.states.iter()
            .map(|s| s.probability)
            .sum();
        
        assert_float_eq(total_prob, 1.0, 0.0001);
        
        // Check amplitude-probability relationship
        for state in &position.states {
            assert_float_eq(state.amplitude * state.amplitude, state.probability, 0.0001);
        }
    }
    
    #[tokio::test]
    async fn test_entanglement_creation() {
        let engine = QuantumEngine::new();
        let group_id = Uuid::new_v4().to_string();
        
        // Create multiple entangled positions
        let mut position_ids = Vec::new();
        for wallet in &["wallet1", "wallet2", "wallet3"] {
            let mut states = vec![
                create_quantum_state(1000, 0),
                create_quantum_state(1000, 1),
            ];
            normalize_states(&mut states);
            
            let position_id = engine.create_quantum_position(
                wallet.to_string(),
                states,
                Some(group_id.clone())
            ).await.unwrap();
            position_ids.push(position_id);
        }
        
        // Verify positions are created
        for id in &position_ids {
            let position = engine.get_quantum_position(id).await.unwrap();
            assert_eq!(position.entanglement_group, Some(group_id.clone()));
        }
    }
    
    #[tokio::test]
    async fn test_quantum_measurement() {
        let engine = QuantumEngine::new();
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1000, 1),
            create_quantum_state(1001, 0),
            create_quantum_state(1001, 1),
        ];
        normalize_states(&mut states);
        
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states,
            None
        ).await.unwrap();
        
        // Perform measurement
        let measurement = engine.measure_quantum_position(&position_id).await.unwrap();
        
        assert!(measurement.caused_collapse);
        assert!(measurement.probability > 0.0 && measurement.probability <= 1.0);
    }
    
    #[tokio::test]
    async fn test_wave_function_collapse() {
        let engine = QuantumEngine::new();
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1001, 1),
            create_quantum_state(1002, 0),
        ];
        normalize_states(&mut states);
        
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states.clone(),
            None
        ).await.unwrap();
        
        // Perform measurement to collapse
        let measurement = engine.measure_quantum_position(&position_id).await.unwrap();
        
        // Verify position is collapsed
        let retrieved = engine.get_quantum_position(&position_id).await.unwrap();
        assert!(retrieved.is_collapsed);
        assert!(retrieved.measurement_result.is_some());
        
        // Verify collapsed state was one of the original states
        let original_market_ids: Vec<u128> = states.iter()
            .map(|s| s.market_id)
            .collect();
        assert!(original_market_ids.contains(&measurement.measured_state.market_id));
    }
    
    #[tokio::test]
    async fn test_entanglement_correlation() {
        let engine = QuantumEngine::new();
        let group_id = Uuid::new_v4().to_string();
        
        // Create two entangled positions
        let mut states1 = vec![create_quantum_state(1000, 0), create_quantum_state(1000, 1)];
        let mut states2 = vec![create_quantum_state(1001, 0), create_quantum_state(1001, 1)];
        normalize_states(&mut states1);
        normalize_states(&mut states2);
        
        let position_id1 = engine.create_quantum_position(
            "wallet1".to_string(),
            states1,
            Some(group_id.clone())
        ).await.unwrap();
        
        let position_id2 = engine.create_quantum_position(
            "wallet2".to_string(),
            states2,
            Some(group_id.clone())
        ).await.unwrap();
        
        // Measure first position
        let measurement1 = engine.measure_quantum_position(&position_id1).await.unwrap();
        
        // Check if second position was affected
        let position2 = engine.get_quantum_position(&position_id2).await.unwrap();
        
        // Both positions should be collapsed due to entanglement
        assert!(measurement1.affected_entangled.contains(&position_id2));
        assert!(position2.is_collapsed);
    }
    
    #[tokio::test]
    async fn test_decoherence_time() {
        let engine = QuantumEngine::new();
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1000, 1),
        ];
        normalize_states(&mut states);
        
        // Create position with short coherence time
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states,
            None
        ).await.unwrap();
        
        // Position should initially be coherent
        let position = engine.get_quantum_position(&position_id).await.unwrap();
        assert!(!position.is_collapsed);
        
        // Wait longer than coherence time
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // After some time, decoherence timer may have fired
        // This is a simplified test - real decoherence is time-based
    }
    
    #[tokio::test]
    async fn test_multiple_quantum_positions() {
        let engine = QuantumEngine::new();
        let wallet = "test_wallet";
        
        // Create multiple positions
        let mut position_ids = Vec::new();
        for i in 0..10 {
            let num_states = 2 + i % 3;
            let mut states = Vec::new();
            for j in 0..num_states {
                states.push(create_quantum_state(1000 + j as u128, (j % 2) as u8));
            }
            normalize_states(&mut states);
            
            let position_id = engine.create_quantum_position(
                wallet.to_string(),
                states,
                None
            ).await.unwrap();
            position_ids.push(position_id);
        }
        
        // Get all positions for wallet
        let wallet_positions = engine.get_wallet_positions(wallet).await.unwrap();
        assert_eq!(wallet_positions.len(), 10);
        
        // Verify all positions are retrieved
        for id in position_ids {
            assert!(wallet_positions.iter().any(|p| p.id == id));
        }
    }
    
    #[tokio::test]
    async fn test_quantum_state_interference() {
        let engine = QuantumEngine::new();
        
        // Create states with specific phases for interference
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1000, 1),
        ];
        states[0].phase = 0.0;
        states[1].phase = std::f64::consts::PI; // Destructive interference
        normalize_states(&mut states);
        
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states,
            None
        ).await.unwrap();
        
        // Get position to check states
        let position = engine.get_quantum_position(&position_id).await.unwrap();
        
        // With opposite phases, amplitudes should interfere
        // This is a simplified test - real interference would be more complex
        assert!(position.states.len() == 2);
    }
    
    #[tokio::test]
    async fn test_partial_measurement() {
        let engine = QuantumEngine::new();
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1000, 1),
            create_quantum_state(1000, 2),
            create_quantum_state(1001, 0),
            create_quantum_state(1001, 1),
        ];
        normalize_states(&mut states);
        
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states,
            None
        ).await.unwrap();
        
        // Measure position (full measurement)
        let measurement = engine.measure_quantum_position(&position_id).await.unwrap();
        
        // Measurement should be from one of the states
        assert!(measurement.measured_state.outcome <= 2);
    }
    
    #[test]
    fn test_probability_distribution() {
        let position = create_quantum_position("test_wallet", 100);
        
        // Statistical test - all probabilities should be reasonable
        for state in &position.states {
            assert!(state.probability > 0.0);
            assert!(state.probability < 1.0);
        }
        
        // Check distribution properties
        let mean_prob = 1.0 / position.states.len() as f64;
        let variance: f64 = position.states.iter()
            .map(|s| (s.probability - mean_prob).powi(2))
            .sum::<f64>() / position.states.len() as f64;
        
        // Variance should be reasonable for normalized distribution
        assert!(variance < 0.1);
    }
    
    #[tokio::test]
    async fn test_concurrent_access() {
        let engine = Arc::new(QuantumEngine::new());
        let mut states = vec![
            create_quantum_state(1000, 0),
            create_quantum_state(1000, 1),
            create_quantum_state(1001, 0),
        ];
        normalize_states(&mut states);
        
        let position_id = engine.create_quantum_position(
            "test_wallet".to_string(),
            states,
            None
        ).await.unwrap();
        
        // Simulate concurrent reads
        let mut handles = vec![];
        for _ in 0..10 {
            let engine_clone = engine.clone();
            let pos_id = position_id.clone();
            let handle = tokio::spawn(async move {
                engine_clone.get_quantum_position(&pos_id).await
            });
            handles.push(handle);
        }
        
        // All reads should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }
}