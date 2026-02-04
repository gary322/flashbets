use anchor_lang::prelude::*;
use crate::amm::l2_distribution::{
    Distribution, DistributionPoint, CurveType, FIXED_POINT_SCALE
};
use crate::amm::distribution_editor::DistributionEditor;

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct Mode {
    pub mean: u64,
    pub variance: u64,
    pub skewness: Option<u64>,
    pub kurtosis: Option<u64>,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub enum EventType {
    ElectionWithTies,
    ProductLaunch,
    EconomicIndicator,
    Sports,
    Weather,
    Custom,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct HistoricalOutcome {
    pub value: u64,
    pub frequency: u64,
    pub event_type: EventType,
}

#[error_code]
pub enum MultiModalError {
    #[msg("Invalid number of modes")]
    InvalidModeCount,
    
    #[msg("Weights must sum to 1")]
    InvalidWeights,
    
    #[msg("Failed to generate mode distribution")]
    ModeGenerationFailed,
    
    #[msg("Insufficient historical data")]
    InsufficientData,
}

pub struct MultiModalDistribution {
    pub modes: Vec<Mode>,
    pub mixture_weights: Vec<u64>,
    pub total_points: usize,
}

impl MultiModalDistribution {
    pub fn new(total_points: usize) -> Self {
        Self {
            modes: Vec::new(),
            mixture_weights: Vec::new(),
            total_points,
        }
    }

    /// Create a bimodal distribution
    pub fn create_bimodal(
        &self,
        mode1: Mode,
        mode2: Mode,
        weight1: u64,
    ) -> Result<Distribution> {
        let weight2 = FIXED_POINT_SCALE.saturating_sub(weight1);

        // Generate points for each mode
        let dist1 = self.generate_mode_distribution(&mode1)?;
        let dist2 = self.generate_mode_distribution(&mode2)?;

        // Combine with weights
        let mut combined_points = Vec::new();

        for i in 0..self.total_points {
            let x = self.calculate_x_coordinate(i);

            let f1 = self.evaluate_at_point(&dist1, x)?;
            let f2 = self.evaluate_at_point(&dist2, x)?;

            let combined_f = (f1 * weight1 + f2 * weight2) / FIXED_POINT_SCALE;

            combined_points.push(DistributionPoint {
                x,
                f_x: combined_f,
                weight: FIXED_POINT_SCALE,
            });
        }

        let l2_norm = self.calculate_combined_norm(&combined_points)?;
        
        Ok(Distribution {
            curve_type: CurveType::Bimodal {
                mean1: mode1.mean,
                mean2: mode2.mean,
                variance1: mode1.variance,
                variance2: mode2.variance,
                weight1,
            },
            points: combined_points,
            l2_norm,
        })
    }

    /// Create a trimodal distribution (e.g., for win/lose/tie)
    pub fn create_trimodal(
        &self,
        modes: [Mode; 3],
        weights: [u64; 3],
    ) -> Result<Distribution> {
        // Verify weights sum to 1 (in fixed point)
        let weight_sum: u64 = weights.iter().sum();
        if weight_sum != FIXED_POINT_SCALE {
            return Err(MultiModalError::InvalidWeights.into());
        }

        // Generate distributions for each mode
        let dists: Vec<Distribution> = modes.iter()
            .map(|mode| self.generate_mode_distribution(mode))
            .collect::<Result<Vec<_>>>()?;

        // Combine with weights
        let mut combined_points = Vec::new();

        for i in 0..self.total_points {
            let x = self.calculate_x_coordinate(i);

            let mut combined_f = 0u128;
            for (j, dist) in dists.iter().enumerate() {
                let f = self.evaluate_at_point(dist, x)?;
                combined_f += (f as u128 * weights[j] as u128) / FIXED_POINT_SCALE as u128;
            }

            combined_points.push(DistributionPoint {
                x,
                f_x: combined_f as u64,
                weight: FIXED_POINT_SCALE,
            });
        }

        let custom_points = combined_points.iter()
            .map(|p| (p.x, p.f_x))
            .collect();
        let l2_norm = self.calculate_combined_norm(&combined_points)?;
        
        Ok(Distribution {
            curve_type: CurveType::Custom { 
                points: custom_points
            },
            points: combined_points,
            l2_norm,
        })
    }

    /// Optimize distribution for specific event type
    pub fn optimize_for_event_outcome(
        &self,
        event_type: EventType,
        historical_data: &[HistoricalOutcome],
    ) -> Result<Distribution> {
        match event_type {
            EventType::ElectionWithTies => {
                // Create trimodal for win/lose/tie
                self.create_trimodal_election_distribution(historical_data)
            }
            EventType::ProductLaunch => {
                // Skewed distribution for launch dates
                self.create_skewed_launch_distribution(historical_data)
            }
            EventType::EconomicIndicator => {
                // Fat-tailed distribution for economic events
                self.create_fat_tailed_distribution(historical_data)
            }
            _ => {
                // Default to normal
                self.create_default_normal_distribution()
            }
        }
    }

    fn create_trimodal_election_distribution(
        &self,
        historical_data: &[HistoricalOutcome],
    ) -> Result<Distribution> {
        // Analyze historical data for election outcomes
        let (win_prob, lose_prob, tie_prob) = self.analyze_election_probabilities(historical_data)?;

        // Create three modes centered around typical outcomes
        let modes = [
            Mode { 
                mean: 25 * FIXED_POINT_SCALE / 100, // 25% (lose)
                variance: 5 * FIXED_POINT_SCALE / 100,
                skewness: None,
                kurtosis: None,
            },
            Mode { 
                mean: 50 * FIXED_POINT_SCALE / 100, // 50% (tie)
                variance: 2 * FIXED_POINT_SCALE / 100,
                skewness: None,
                kurtosis: None,
            },
            Mode { 
                mean: 75 * FIXED_POINT_SCALE / 100, // 75% (win)
                variance: 5 * FIXED_POINT_SCALE / 100,
                skewness: None,
                kurtosis: None,
            },
        ];

        let weights = [lose_prob, tie_prob, win_prob];

        self.create_trimodal(modes, weights)
    }

    fn create_skewed_launch_distribution(
        &self,
        historical_data: &[HistoricalOutcome],
    ) -> Result<Distribution> {
        // Product launches tend to be delayed (right-skewed)
        let mean_delay = self.calculate_mean_from_historical(historical_data)?;
        
        // Create a log-normal-like distribution
        let mode = Mode {
            mean: mean_delay,
            variance: mean_delay / 4, // Higher variance for uncertainty
            skewness: Some(2 * FIXED_POINT_SCALE), // Positive skew
            kurtosis: None,
        };

        self.generate_skewed_distribution(&mode)
    }

    fn create_fat_tailed_distribution(
        &self,
        historical_data: &[HistoricalOutcome],
    ) -> Result<Distribution> {
        // Economic indicators often have fat tails (extreme events)
        let (mean, variance) = self.calculate_moments_from_historical(historical_data)?;
        
        // Create a distribution with higher kurtosis
        let mode = Mode {
            mean,
            variance: variance * 2, // Increase variance for fat tails
            skewness: None,
            kurtosis: Some(6 * FIXED_POINT_SCALE), // Excess kurtosis
        };

        self.generate_fat_tailed_distribution(&mode)
    }

    fn create_default_normal_distribution(&self) -> Result<Distribution> {
        let mode = Mode {
            mean: FIXED_POINT_SCALE / 2, // 0.5
            variance: FIXED_POINT_SCALE / 10, // 0.1
            skewness: None,
            kurtosis: None,
        };

        self.generate_mode_distribution(&mode)
    }

    fn generate_mode_distribution(&self, mode: &Mode) -> Result<Distribution> {
        let editor = DistributionEditor::new(crate::amm::distribution_editor::DistributionConstraints {
            l2_norm: 100_000 * FIXED_POINT_SCALE,
            max_bound: 1_000 * FIXED_POINT_SCALE,
            must_integrate_to_one: true,
        });

        editor.create_normal_distribution(mode.mean, mode.variance, self.total_points)
    }

    fn generate_skewed_distribution(&self, mode: &Mode) -> Result<Distribution> {
        // Generate base normal distribution
        let mut dist = self.generate_mode_distribution(mode)?;

        // Apply skewness transformation
        if let Some(skewness) = mode.skewness {
            for point in &mut dist.points {
                // Simple skewness transformation: f'(x) = f(x) * (1 + α * (x - μ))
                let x_centered = if point.x > mode.mean {
                    point.x - mode.mean
                } else {
                    0
                };
                
                let skew_factor = FIXED_POINT_SCALE + (skewness * x_centered) / (10 * FIXED_POINT_SCALE);
                point.f_x = (point.f_x as u128 * skew_factor as u128 / FIXED_POINT_SCALE as u128) as u64;
            }
        }

        Ok(dist)
    }

    fn generate_fat_tailed_distribution(&self, mode: &Mode) -> Result<Distribution> {
        // Use Student's t-distribution approximation for fat tails
        let mut dist = self.generate_mode_distribution(mode)?;

        if let Some(kurtosis) = mode.kurtosis {
            // Degrees of freedom from excess kurtosis
            let df = 4 * FIXED_POINT_SCALE / kurtosis.max(1);
            
            for point in &mut dist.points {
                // Transform to t-distribution shape
                let x_standardized = if point.x > mode.mean {
                    (point.x - mode.mean) * FIXED_POINT_SCALE / mode.variance.max(1)
                } else {
                    0
                };
                
                // t-distribution has heavier tails
                let tail_factor = FIXED_POINT_SCALE + x_standardized * x_standardized / df;
                point.f_x = (point.f_x as u128 * FIXED_POINT_SCALE as u128 / tail_factor as u128) as u64;
            }
        }

        Ok(dist)
    }

    fn evaluate_at_point(&self, dist: &Distribution, x: u64) -> Result<u64> {
        // Find nearest points and interpolate
        let mut left_idx = 0;
        let mut right_idx = 0;

        for (i, point) in dist.points.iter().enumerate() {
            if point.x <= x {
                left_idx = i;
            }
            if point.x >= x {
                right_idx = i;
                break;
            }
        }

        if left_idx == right_idx {
            return Ok(dist.points[left_idx].f_x);
        }

        // Linear interpolation
        let left = &dist.points[left_idx];
        let right = &dist.points[right_idx];
        
        let t = (x - left.x) * FIXED_POINT_SCALE / (right.x - left.x);
        let interpolated = left.f_x + (right.f_x - left.f_x) * t / FIXED_POINT_SCALE;

        Ok(interpolated)
    }

    fn calculate_x_coordinate(&self, index: usize) -> u64 {
        index as u64 * FIXED_POINT_SCALE / (self.total_points as u64 - 1)
    }

    fn calculate_combined_norm(&self, points: &[DistributionPoint]) -> Result<u64> {
        let mut sum_squared = 0u128;

        for point in points {
            let f_squared = (point.f_x as u128) * (point.f_x as u128);
            sum_squared = sum_squared.saturating_add(f_squared / FIXED_POINT_SCALE as u128);
        }

        Ok(self.fixed_sqrt(sum_squared as u64)?)
    }

    fn fixed_sqrt(&self, x: u64) -> Result<u64> {
        if x == 0 {
            return Ok(0);
        }

        let mut guess = x / 2;
        for _ in 0..10 {
            let new_guess = (guess + x / guess) / 2;
            if self.abs_diff(guess, new_guess) < 1 {
                break;
            }
            guess = new_guess;
        }

        Ok(guess)
    }

    fn abs_diff(&self, a: u64, b: u64) -> u64 {
        if a > b { a - b } else { b - a }
    }

    fn analyze_election_probabilities(
        &self,
        historical_data: &[HistoricalOutcome],
    ) -> Result<(u64, u64, u64)> {
        if historical_data.is_empty() {
            // Default probabilities
            return Ok((
                30 * FIXED_POINT_SCALE / 100, // 30% lose
                10 * FIXED_POINT_SCALE / 100, // 10% tie
                60 * FIXED_POINT_SCALE / 100, // 60% win
            ));
        }

        let mut win_count = 0u64;
        let mut lose_count = 0u64;
        let mut tie_count = 0u64;
        let mut total = 0u64;

        for outcome in historical_data {
            total += outcome.frequency;
            if outcome.value > 60 * FIXED_POINT_SCALE / 100 {
                win_count += outcome.frequency;
            } else if outcome.value < 40 * FIXED_POINT_SCALE / 100 {
                lose_count += outcome.frequency;
            } else {
                tie_count += outcome.frequency;
            }
        }

        let lose_prob = lose_count * FIXED_POINT_SCALE / total;
        let tie_prob = tie_count * FIXED_POINT_SCALE / total;
        let win_prob = win_count * FIXED_POINT_SCALE / total;

        Ok((lose_prob, tie_prob, win_prob))
    }

    fn calculate_mean_from_historical(
        &self,
        historical_data: &[HistoricalOutcome],
    ) -> Result<u64> {
        if historical_data.is_empty() {
            return Ok(FIXED_POINT_SCALE / 2); // Default to 0.5
        }

        let mut weighted_sum = 0u128;
        let mut total_weight = 0u128;

        for outcome in historical_data {
            weighted_sum += outcome.value as u128 * outcome.frequency as u128;
            total_weight += outcome.frequency as u128;
        }

        Ok((weighted_sum / total_weight) as u64)
    }

    fn calculate_moments_from_historical(
        &self,
        historical_data: &[HistoricalOutcome],
    ) -> Result<(u64, u64)> {
        let mean = self.calculate_mean_from_historical(historical_data)?;
        
        if historical_data.len() < 2 {
            // Default variance
            return Ok((mean, FIXED_POINT_SCALE / 10));
        }

        let mut variance_sum = 0u128;
        let mut total_weight = 0u128;

        for outcome in historical_data {
            let diff = if outcome.value > mean {
                outcome.value - mean
            } else {
                mean - outcome.value
            };
            
            let diff_squared = (diff as u128 * diff as u128) / FIXED_POINT_SCALE as u128;
            variance_sum += diff_squared * outcome.frequency as u128;
            total_weight += outcome.frequency as u128;
        }

        let variance = (variance_sum / total_weight) as u64;

        Ok((mean, variance))
    }
}