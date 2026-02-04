// Phase 20: Quantum Collapse Handler
// Manages quantum state collapses when related markets resolve simultaneously

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
};

/// Quantum collapse configuration
pub const MAX_ENTANGLED_MARKETS: usize = 10;
pub const COLLAPSE_THRESHOLD_BPS: u16 = 9500; // 95% correlation
pub const QUANTUM_RESOLUTION_WINDOW: u64 = 150; // ~1 minute
pub const MIN_ENTANGLEMENT_VOLUME: u64 = 1_000_000_000_000; // $1M
pub const WAVE_FUNCTION_SAMPLES: u32 = 1000;
pub const COHERENCE_DECAY_RATE: u64 = 100; // Basis points per hour

/// Quantum state handler
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct QuantumCollapseHandler {
    pub total_collapses_handled: u64,
    pub active_entanglements: Vec<QuantumEntanglement>,
    pub collapse_history: Vec<CollapseEvent>,
    pub wave_function_state: WaveFunctionState,
    pub coherence_metrics: CoherenceMetrics,
    pub last_measurement_slot: u64,
    pub measurement_backlog: Vec<PendingMeasurement>,
    pub superposition_count: u32,
}

impl QuantumCollapseHandler {
    pub const SIZE: usize = 8 + // total_collapses_handled
        4 + MAX_ENTANGLED_MARKETS * QuantumEntanglement::SIZE + // active_entanglements
        4 + 100 * CollapseEvent::SIZE + // collapse_history (last 100)
        WaveFunctionState::SIZE +
        CoherenceMetrics::SIZE +
        8 + // last_measurement_slot
        4 + 50 * PendingMeasurement::SIZE + // measurement_backlog
        4; // superposition_count

    /// Initialize quantum handler
    pub fn initialize(&mut self) -> ProgramResult {
        self.total_collapses_handled = 0;
        self.active_entanglements = Vec::new();
        self.collapse_history = Vec::new();
        self.wave_function_state = WaveFunctionState::default();
        self.coherence_metrics = CoherenceMetrics::default();
        self.last_measurement_slot = Clock::get()?.slot;
        self.measurement_backlog = Vec::new();
        self.superposition_count = 0;

        msg!("Quantum collapse handler initialized");
        Ok(())
    }

    /// Detect quantum entanglement between markets
    pub fn detect_entanglement(
        &mut self,
        market_a: &MarketState,
        market_b: &MarketState,
        correlation_data: &CorrelationData,
    ) -> Result<bool, ProgramError> {
        // Check minimum volume threshold
        if market_a.volume < MIN_ENTANGLEMENT_VOLUME || 
           market_b.volume < MIN_ENTANGLEMENT_VOLUME {
            return Ok(false);
        }

        // Calculate quantum correlation
        let correlation = self.calculate_quantum_correlation(
            market_a,
            market_b,
            correlation_data,
        )?;

        if correlation >= COLLAPSE_THRESHOLD_BPS {
            // Create entanglement
            let entanglement = QuantumEntanglement {
                market_a: market_a.market_id,
                market_b: market_b.market_id,
                correlation_strength: correlation,
                entanglement_type: self.classify_entanglement(correlation)?,
                creation_slot: Clock::get()?.slot,
                coherence_level: 10000, // Start at 100%
                measurement_count: 0,
                last_interaction: Clock::get()?.slot,
            };

            self.active_entanglements.push(entanglement);
            self.superposition_count += 1;

            msg!("Quantum entanglement detected: {} <-> {} ({}bps)", 
                market_a.market_id, 
                market_b.market_id, 
                correlation
            );

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Calculate quantum correlation
    fn calculate_quantum_correlation(
        &self,
        market_a: &MarketState,
        market_b: &MarketState,
        correlation_data: &CorrelationData,
    ) -> Result<u16, ProgramError> {
        // Price correlation
        let price_corr = self.calculate_price_correlation(
            &correlation_data.price_history_a,
            &correlation_data.price_history_b,
        )?;

        // Volume correlation
        let volume_corr = self.calculate_volume_correlation(
            market_a.volume,
            market_b.volume,
            correlation_data.volume_correlation,
        )?;

        // Sentiment correlation (from verse grouping)
        let sentiment_corr = correlation_data.sentiment_correlation;

        // Weighted quantum correlation
        let quantum_corr = (price_corr as u32 * 5 + 
                           volume_corr as u32 * 3 + 
                           sentiment_corr as u32 * 2) / 10;

        Ok(quantum_corr as u16)
    }

    /// Calculate price correlation
    fn calculate_price_correlation(
        &self,
        prices_a: &[u64],
        prices_b: &[u64],
    ) -> Result<u16, ProgramError> {
        if prices_a.len() != prices_b.len() || prices_a.is_empty() {
            return Ok(0);
        }

        let n = prices_a.len() as u128;
        
        // Calculate means
        let mean_a = prices_a.iter().map(|&x| x as u128).sum::<u128>() / n;
        let mean_b = prices_b.iter().map(|&x| x as u128).sum::<u128>() / n;

        // Calculate correlation coefficient
        let mut cov = 0i128;
        let mut var_a = 0u128;
        let mut var_b = 0u128;

        for i in 0..prices_a.len() {
            let diff_a = prices_a[i] as i128 - mean_a as i128;
            let diff_b = prices_b[i] as i128 - mean_b as i128;
            
            cov += diff_a * diff_b;
            var_a += (diff_a * diff_a) as u128;
            var_b += (diff_b * diff_b) as u128;
        }

        if var_a == 0 || var_b == 0 {
            return Ok(0);
        }

        // Correlation = cov / sqrt(var_a * var_b)
        // Scale to basis points
        let correlation = (cov.abs() as u128 * 10000) / 
                         ((var_a as f64).sqrt() * (var_b as f64).sqrt()) as u128;

        Ok(correlation.min(10000) as u16)
    }

    /// Calculate volume correlation
    fn calculate_volume_correlation(
        &self,
        volume_a: u64,
        volume_b: u64,
        historical_correlation: u16,
    ) -> Result<u16, ProgramError> {
        // Volume ratio similarity
        let ratio = if volume_a > volume_b {
            (volume_b as u128 * 10000) / volume_a as u128
        } else {
            (volume_a as u128 * 10000) / volume_b as u128
        };

        // Combine with historical correlation
        let volume_corr = (ratio as u16 + historical_correlation) / 2;

        Ok(volume_corr)
    }

    /// Classify entanglement type
    fn classify_entanglement(&self, correlation: u16) -> Result<EntanglementType, ProgramError> {
        if correlation >= 9900 {
            Ok(EntanglementType::Strong)
        } else if correlation >= 9700 {
            Ok(EntanglementType::Moderate)
        } else {
            Ok(EntanglementType::Weak)
        }
    }

    /// Handle quantum collapse
    pub fn handle_collapse(
        &mut self,
        triggering_market: &Pubkey,
        resolution_outcome: ResolutionOutcome,
    ) -> Result<Vec<CollapsedMarket>, ProgramError> {
        let current_slot = Clock::get()?.slot;
        let mut collapsed_markets = Vec::new();

        // Find all entangled markets
        let entangled: Vec<_> = self.active_entanglements
            .iter()
            .filter(|e| e.market_a == *triggering_market || e.market_b == *triggering_market)
            .cloned()
            .collect();

        msg!("Quantum collapse triggered by {} affecting {} markets", 
            triggering_market, 
            entangled.len()
        );

        // Process wave function collapse
        for entanglement in &entangled {
            let affected_market = if entanglement.market_a == *triggering_market {
                entanglement.market_b
            } else {
                entanglement.market_a
            };

            // Calculate collapse probability
            let collapse_prob = self.calculate_collapse_probability(
                &entanglement,
                &resolution_outcome,
            )?;

            // Determine collapsed state
            let collapsed_state = self.determine_collapsed_state(
                collapse_prob,
                &resolution_outcome,
                entanglement.entanglement_type,
            )?;

            collapsed_markets.push(CollapsedMarket {
                market_id: affected_market,
                collapse_probability: collapse_prob,
                suggested_outcome: collapsed_state,
                confidence_level: self.calculate_confidence(entanglement)?,
                should_auto_resolve: collapse_prob > 9000 && 
                                   entanglement.entanglement_type == EntanglementType::Strong,
            });

            // Update metrics
            self.wave_function_state.collapse_count += 1;
            self.coherence_metrics.update_post_collapse(entanglement.coherence_level);
        }

        // Record collapse event
        self.collapse_history.push(CollapseEvent {
            triggering_market: *triggering_market,
            affected_markets: collapsed_markets.len() as u32,
            collapse_slot: current_slot,
            total_correlation: entangled.iter()
                .map(|e| e.correlation_strength as u32)
                .sum::<u32>() / entangled.len() as u32,
            outcome: resolution_outcome,
        });

        // Clean up entanglements
        self.active_entanglements.retain(|e| {
            e.market_a != *triggering_market && e.market_b != *triggering_market
        });

        self.total_collapses_handled += 1;
        self.superposition_count = self.superposition_count.saturating_sub(entangled.len() as u32);

        Ok(collapsed_markets)
    }

    /// Calculate collapse probability
    fn calculate_collapse_probability(
        &self,
        entanglement: &QuantumEntanglement,
        outcome: &ResolutionOutcome,
    ) -> Result<u16, ProgramError> {
        let base_prob = entanglement.correlation_strength;
        
        // Adjust for coherence decay
        let coherence_factor = entanglement.coherence_level;
        
        // Adjust for outcome strength
        let outcome_factor = match outcome {
            ResolutionOutcome::Yes => 10000,
            ResolutionOutcome::No => 10000,
            ResolutionOutcome::Invalid => 5000,
        };

        let collapse_prob = (base_prob as u32 * coherence_factor as u32 * outcome_factor as u32) 
                           / (10000 * 10000);

        Ok(collapse_prob.min(10000) as u16)
    }

    /// Determine collapsed state
    fn determine_collapsed_state(
        &self,
        collapse_prob: u16,
        trigger_outcome: &ResolutionOutcome,
        entanglement_type: EntanglementType,
    ) -> Result<ResolutionOutcome, ProgramError> {
        match entanglement_type {
            EntanglementType::Strong => {
                // Strong entanglement = same outcome
                Ok(trigger_outcome.clone())
            },
            EntanglementType::Moderate => {
                // Moderate = likely same outcome
                if collapse_prob > 7500 {
                    Ok(trigger_outcome.clone())
                } else {
                    Ok(ResolutionOutcome::Invalid)
                }
            },
            EntanglementType::Weak => {
                // Weak = uncertain
                Ok(ResolutionOutcome::Invalid)
            },
        }
    }

    /// Calculate confidence level
    fn calculate_confidence(&self, entanglement: &QuantumEntanglement) -> Result<u16, ProgramError> {
        let base_confidence = match entanglement.entanglement_type {
            EntanglementType::Strong => 9000,
            EntanglementType::Moderate => 7000,
            EntanglementType::Weak => 5000,
        };

        // Adjust for coherence
        let confidence = (base_confidence as u32 * entanglement.coherence_level as u32) / 10000;

        Ok(confidence as u16)
    }

    /// Update coherence for active entanglements
    pub fn update_coherence(&mut self, current_slot: u64) -> ProgramResult {
        for entanglement in &mut self.active_entanglements {
            let slots_elapsed = current_slot.saturating_sub(entanglement.last_interaction);
            let decay = (slots_elapsed * COHERENCE_DECAY_RATE) / 3600; // Per hour
            
            entanglement.coherence_level = entanglement.coherence_level
                .saturating_sub(decay.min(1000) as u16); // Cap at 10% per update
            
            entanglement.last_interaction = current_slot;
        }

        // Remove decoherent entanglements
        self.active_entanglements.retain(|e| e.coherence_level > 1000); // Keep if >10%

        Ok(())
    }

    /// Measure quantum state
    pub fn measure_state(
        &mut self,
        market_id: &Pubkey,
    ) -> Result<QuantumMeasurement, ProgramError> {
        let entanglements: Vec<_> = self.active_entanglements
            .iter()
            .filter(|e| e.market_a == *market_id || e.market_b == *market_id)
            .collect();

        let superposition_state = !entanglements.is_empty();
        let entanglement_count = entanglements.len() as u32;
        
        let avg_correlation = if entanglement_count > 0 {
            entanglements.iter()
                .map(|e| e.correlation_strength as u32)
                .sum::<u32>() / entanglement_count
        } else {
            0
        };

        let measurement = QuantumMeasurement {
            market_id: *market_id,
            is_superposition: superposition_state,
            entanglement_count,
            average_correlation: avg_correlation as u16,
            measurement_slot: Clock::get()?.slot,
            wave_function_amplitude: self.calculate_amplitude(&entanglements)?,
        };

        // Update measurement count
        for entanglement in self.active_entanglements.iter_mut() {
            if entanglement.market_a == *market_id || entanglement.market_b == *market_id {
                entanglement.measurement_count += 1;
            }
        }

        self.last_measurement_slot = Clock::get()?.slot;

        Ok(measurement)
    }

    /// Calculate wave function amplitude
    fn calculate_amplitude(&self, entanglements: &[&QuantumEntanglement]) -> Result<u64, ProgramError> {
        if entanglements.is_empty() {
            return Ok(0);
        }

        let sum_squares = entanglements.iter()
            .map(|e| (e.correlation_strength as u64).pow(2))
            .sum::<u64>();

        let amplitude = (sum_squares as f64).sqrt() as u64;

        Ok(amplitude)
    }

    /// Predict collapse cascade
    pub fn predict_cascade(
        &self,
        triggering_market: &Pubkey,
    ) -> Result<CascadePrediction, ProgramError> {
        let mut affected_markets = Vec::new();
        let mut visited = vec![*triggering_market];
        let mut to_visit = vec![*triggering_market];
        let mut cascade_depth = 0;

        while !to_visit.is_empty() && cascade_depth < 5 {
            let mut next_layer = Vec::new();

            for market in &to_visit {
                for entanglement in &self.active_entanglements {
                    let connected = if entanglement.market_a == *market {
                        Some(entanglement.market_b)
                    } else if entanglement.market_b == *market {
                        Some(entanglement.market_a)
                    } else {
                        None
                    };

                    if let Some(connected_market) = connected {
                        if !visited.contains(&connected_market) {
                            visited.push(connected_market);
                            next_layer.push(connected_market);
                            
                            affected_markets.push(CascadeMarket {
                                market_id: connected_market,
                                cascade_probability: self.calculate_cascade_probability(
                                    entanglement.correlation_strength,
                                    cascade_depth,
                                )?,
                                depth: cascade_depth + 1,
                            });
                        }
                    }
                }
            }

            to_visit = next_layer;
            cascade_depth += 1;
        }

        let cascade_prob = self.calculate_total_cascade_probability(&affected_markets)?;
        
        Ok(CascadePrediction {
            triggering_market: *triggering_market,
            affected_markets,
            total_affected: visited.len() as u32 - 1,
            max_depth: cascade_depth,
            cascade_probability: cascade_prob,
        })
    }

    /// Calculate cascade probability
    fn calculate_cascade_probability(
        &self,
        correlation: u16,
        depth: u32,
    ) -> Result<u16, ProgramError> {
        // Probability decreases with depth
        let depth_factor = 10000u32.saturating_sub(depth * 2000);
        let cascade_prob = (correlation as u32 * depth_factor) / 10000;
        
        Ok(cascade_prob.min(10000) as u16)
    }

    /// Calculate total cascade probability
    fn calculate_total_cascade_probability(
        &self,
        cascade_markets: &[CascadeMarket],
    ) -> Result<u16, ProgramError> {
        if cascade_markets.is_empty() {
            return Ok(0);
        }

        let avg_prob = cascade_markets.iter()
            .map(|m| m.cascade_probability as u32)
            .sum::<u32>() / cascade_markets.len() as u32;

        Ok(avg_prob as u16)
    }
}

/// Market state for quantum analysis
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketState {
    pub market_id: Pubkey,
    pub volume: u64,
    pub price: u64,
    pub volatility: u16,
}

/// Correlation data
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CorrelationData {
    pub price_history_a: Vec<u64>,
    pub price_history_b: Vec<u64>,
    pub volume_correlation: u16,
    pub sentiment_correlation: u16,
}

/// Quantum entanglement
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct QuantumEntanglement {
    pub market_a: Pubkey,
    pub market_b: Pubkey,
    pub correlation_strength: u16,
    pub entanglement_type: EntanglementType,
    pub creation_slot: u64,
    pub coherence_level: u16,
    pub measurement_count: u32,
    pub last_interaction: u64,
}

impl QuantumEntanglement {
    pub const SIZE: usize = 32 + 32 + 2 + 1 + 8 + 2 + 4 + 8;
}

/// Entanglement types
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq)]
pub enum EntanglementType {
    Strong,   // >99% correlation
    Moderate, // 97-99% correlation
    Weak,     // 95-97% correlation
}

/// Resolution outcome
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum ResolutionOutcome {
    Yes,
    No,
    Invalid,
}

/// Collapsed market info
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CollapsedMarket {
    pub market_id: Pubkey,
    pub collapse_probability: u16,
    pub suggested_outcome: ResolutionOutcome,
    pub confidence_level: u16,
    pub should_auto_resolve: bool,
}

/// Collapse event
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CollapseEvent {
    pub triggering_market: Pubkey,
    pub affected_markets: u32,
    pub collapse_slot: u64,
    pub total_correlation: u32,
    pub outcome: ResolutionOutcome,
}

impl CollapseEvent {
    pub const SIZE: usize = 32 + 4 + 8 + 4 + 1;
}

/// Wave function state
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct WaveFunctionState {
    pub superposition_count: u32,
    pub collapse_count: u32,
    pub total_measurements: u64,
    pub average_coherence: u16,
}

impl WaveFunctionState {
    pub const SIZE: usize = 4 + 4 + 8 + 2;
}

/// Coherence metrics
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct CoherenceMetrics {
    pub total_coherence: u64,
    pub measurements: u32,
    pub decoherence_events: u32,
}

impl CoherenceMetrics {
    pub const SIZE: usize = 8 + 4 + 4;

    pub fn update_post_collapse(&mut self, coherence_level: u16) {
        self.total_coherence += coherence_level as u64;
        self.measurements += 1;
        if coherence_level < 5000 {
            self.decoherence_events += 1;
        }
    }
}

/// Pending measurement
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PendingMeasurement {
    pub market_id: Pubkey,
    pub requested_slot: u64,
    pub priority: u8,
}

impl PendingMeasurement {
    pub const SIZE: usize = 32 + 8 + 1;
}

/// Quantum measurement result
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct QuantumMeasurement {
    pub market_id: Pubkey,
    pub is_superposition: bool,
    pub entanglement_count: u32,
    pub average_correlation: u16,
    pub measurement_slot: u64,
    pub wave_function_amplitude: u64,
}

/// Cascade prediction
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CascadePrediction {
    pub triggering_market: Pubkey,
    pub affected_markets: Vec<CascadeMarket>,
    pub total_affected: u32,
    pub max_depth: u32,
    pub cascade_probability: u16,
}

/// Cascade market
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CascadeMarket {
    pub market_id: Pubkey,
    pub cascade_probability: u16,
    pub depth: u32,
}

/// Process quantum collapse instructions
pub fn process_quantum_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_quantum(program_id, accounts),
        1 => process_detect_entanglement(program_id, accounts, &instruction_data[1..]),
        2 => process_handle_collapse(program_id, accounts, &instruction_data[1..]),
        3 => process_measure_state(program_id, accounts, &instruction_data[1..]),
        4 => process_predict_cascade(program_id, accounts, &instruction_data[1..]),
        5 => process_update_coherence(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_quantum(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let quantum_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut handler = QuantumCollapseHandler::try_from_slice(&quantum_account.data.borrow())?;
    handler.initialize()?;
    handler.serialize(&mut &mut quantum_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_detect_entanglement(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let quantum_account = next_account_info(account_iter)?;
    let market_a_account = next_account_info(account_iter)?;
    let market_b_account = next_account_info(account_iter)?;

    let market_a: MarketState = BorshDeserialize::try_from_slice(&data[..100])?;
    let market_b: MarketState = BorshDeserialize::try_from_slice(&data[100..200])?;
    let correlation_data: CorrelationData = BorshDeserialize::try_from_slice(&data[200..])?;

    let mut handler = QuantumCollapseHandler::try_from_slice(&quantum_account.data.borrow())?;
    let is_entangled = handler.detect_entanglement(&market_a, &market_b, &correlation_data)?;
    
    if is_entangled {
        handler.serialize(&mut &mut quantum_account.data.borrow_mut()[..])?;
    }

    Ok(())
}

fn process_handle_collapse(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let quantum_account = next_account_info(account_iter)?;
    let triggering_market_account = next_account_info(account_iter)?;

    let triggering_market = Pubkey::new_from_array(data[0..32].try_into().unwrap());
    let resolution_outcome = match data[32] {
        0 => ResolutionOutcome::Yes,
        1 => ResolutionOutcome::No,
        _ => ResolutionOutcome::Invalid,
    };

    let mut handler = QuantumCollapseHandler::try_from_slice(&quantum_account.data.borrow())?;
    let collapsed_markets = handler.handle_collapse(&triggering_market, resolution_outcome)?;
    
    msg!("Quantum collapse handled: {} markets affected", collapsed_markets.len());
    
    handler.serialize(&mut &mut quantum_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_measure_state(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let quantum_account = next_account_info(account_iter)?;

    let market_id = Pubkey::new_from_array(data[0..32].try_into().unwrap());

    let mut handler = QuantumCollapseHandler::try_from_slice(&quantum_account.data.borrow())?;
    let measurement = handler.measure_state(&market_id)?;
    
    msg!("Quantum measurement: superposition={}, entanglements={}", 
        measurement.is_superposition, 
        measurement.entanglement_count
    );
    
    handler.serialize(&mut &mut quantum_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_predict_cascade(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let quantum_account = next_account_info(account_iter)?;

    let triggering_market = Pubkey::new_from_array(data[0..32].try_into().unwrap());

    let handler = QuantumCollapseHandler::try_from_slice(&quantum_account.data.borrow())?;
    let prediction = handler.predict_cascade(&triggering_market)?;
    
    msg!("Cascade prediction: {} markets affected, max depth {}, probability {}bps", 
        prediction.total_affected,
        prediction.max_depth,
        prediction.cascade_probability
    );

    Ok(())
}

fn process_update_coherence(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let quantum_account = next_account_info(account_iter)?;

    let mut handler = QuantumCollapseHandler::try_from_slice(&quantum_account.data.borrow())?;
    handler.update_coherence(Clock::get()?.slot)?;
    handler.serialize(&mut &mut quantum_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;