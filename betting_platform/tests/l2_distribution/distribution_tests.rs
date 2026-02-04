#[cfg(test)]
mod l2_distribution_tests {
    use anchor_lang::prelude::*;
    use betting_platform::amm::l2_distribution::{
        L2DistributionAMM, Distribution, DistributionPoint, CurveType,
        L2_NORM_K, MAX_F_BOUND, FIXED_POINT_SCALE, SIMPSON_POINTS,
        DistributionPrice, L2Error
    };
    use betting_platform::amm::distribution_editor::{
        DistributionEditor, DistributionConstraints, ControlPoint
    };
    use betting_platform::amm::multimodal_distribution::{
        MultiModalDistribution, Mode, EventType, HistoricalOutcome
    };

    #[test]
    fn test_l2_norm_constraint() {
        let mut amm = L2DistributionAMM::new(
            L2_NORM_K,
            MAX_F_BOUND,
        );

        // Create distribution that violates norm
        let mut dist = create_test_distribution();
        dist.points[0].f_x = 10000 * FIXED_POINT_SCALE; // Very high value

        // Should fail norm check
        let result = amm.price_distribution_bet(
            &dist,
            100 * FIXED_POINT_SCALE,
            (0, FIXED_POINT_SCALE),
        );

        assert!(matches!(result, Err(e) if e.to_string().contains("L2 norm constraint violated")));
    }

    #[test]
    fn test_simpson_integration() {
        let mut amm = L2DistributionAMM::new(
            L2_NORM_K,
            MAX_F_BOUND,
        );

        // Create normal distribution
        let editor = DistributionEditor::new(DistributionConstraints {
            l2_norm: L2_NORM_K,
            max_bound: MAX_F_BOUND,
            must_integrate_to_one: true,
        });
        
        let dist = editor.create_normal_distribution(
            FIXED_POINT_SCALE / 2, // mean = 0.5
            FIXED_POINT_SCALE / 10, // variance = 0.1
            SIMPSON_POINTS,
        ).unwrap();

        // Integrate over full range (should be ~1)
        let integral = amm.integrate_simpson(
            &dist,
            0,
            FIXED_POINT_SCALE,
        ).unwrap();

        // Check that integral is close to 1 (within 1%)
        let expected = FIXED_POINT_SCALE;
        let tolerance = FIXED_POINT_SCALE / 100;
        assert!(
            abs_diff(integral, expected) < tolerance,
            "Integral {} should be close to {}", integral, expected
        );
    }

    #[test]
    fn test_bimodal_distribution() {
        let multimodal = MultiModalDistribution::new(100);

        let mode1 = Mode {
            mean: 30 * FIXED_POINT_SCALE / 100, // 0.3
            variance: 5 * FIXED_POINT_SCALE / 100, // 0.05
            skewness: None,
            kurtosis: None,
        };

        let mode2 = Mode {
            mean: 70 * FIXED_POINT_SCALE / 100, // 0.7
            variance: 5 * FIXED_POINT_SCALE / 100, // 0.05
            skewness: None,
            kurtosis: None,
        };

        let dist = multimodal.create_bimodal(
            mode1,
            mode2,
            60 * FIXED_POINT_SCALE / 100, // weight for first mode = 0.6
        ).unwrap();

        // Should have two peaks
        let peaks = find_local_maxima(&dist.points);
        assert_eq!(peaks.len(), 2, "Bimodal should have two peaks");
    }

    #[test]
    fn test_normal_distribution_creation() {
        let editor = DistributionEditor::new(DistributionConstraints {
            l2_norm: L2_NORM_K,
            max_bound: MAX_F_BOUND,
            must_integrate_to_one: true,
        });

        let mean = 50 * FIXED_POINT_SCALE / 100; // 0.5
        let variance = 10 * FIXED_POINT_SCALE / 100; // 0.1
        
        let dist = editor.create_normal_distribution(mean, variance, 50).unwrap();
        
        assert_eq!(dist.points.len(), 50);
        assert!(matches!(dist.curve_type, CurveType::Normal { .. }));
        
        // Check that peak is around the mean
        let max_point = dist.points.iter().max_by_key(|p| p.f_x).unwrap();
        let distance_from_mean = abs_diff(max_point.x, mean);
        assert!(distance_from_mean < 5 * FIXED_POINT_SCALE / 100); // Within 5%
    }

    #[test]
    fn test_uniform_distribution_creation() {
        let editor = DistributionEditor::new(DistributionConstraints {
            l2_norm: L2_NORM_K,
            max_bound: MAX_F_BOUND,
            must_integrate_to_one: true,
        });

        let min = 20 * FIXED_POINT_SCALE / 100; // 0.2
        let max = 80 * FIXED_POINT_SCALE / 100; // 0.8
        
        let dist = editor.create_uniform_distribution(min, max, 30).unwrap();
        
        assert_eq!(dist.points.len(), 30);
        
        // Check that all values within range have similar height
        let heights: Vec<u64> = dist.points.iter()
            .filter(|p| p.x >= min && p.x <= max)
            .map(|p| p.f_x)
            .collect();
        
        let avg_height = heights.iter().sum::<u64>() / heights.len() as u64;
        for height in heights {
            assert!(abs_diff(height, avg_height) < avg_height / 10); // Within 10%
        }
    }

    #[test]
    fn test_distribution_editor_drag() {
        let mut editor = DistributionEditor::new(DistributionConstraints {
            l2_norm: L2_NORM_K,
            max_bound: MAX_F_BOUND,
            must_integrate_to_one: false,
        });

        // Add control points
        editor.add_control_point(0, FIXED_POINT_SCALE / 10, false);
        editor.add_control_point(FIXED_POINT_SCALE / 2, FIXED_POINT_SCALE / 2, false);
        editor.add_control_point(FIXED_POINT_SCALE, FIXED_POINT_SCALE / 10, false);

        // Drag middle point higher
        let new_value = 80 * FIXED_POINT_SCALE / 100;
        let dist = editor.drag_curve_point(1, new_value).unwrap();
        
        // Check that the distribution was updated
        assert!(dist.points.len() > 0);
        
        // Find point closest to middle
        let mid_x = FIXED_POINT_SCALE / 2;
        let mid_point = dist.points.iter()
            .min_by_key(|p| abs_diff(p.x, mid_x))
            .unwrap();
        
        // Should be higher than original
        assert!(mid_point.f_x > FIXED_POINT_SCALE / 2);
    }

    #[test]
    fn test_election_distribution() {
        let multimodal = MultiModalDistribution::new(100);
        
        let historical_data = vec![
            HistoricalOutcome {
                value: 25 * FIXED_POINT_SCALE / 100, // Lose
                frequency: 30,
                event_type: EventType::ElectionWithTies,
            },
            HistoricalOutcome {
                value: 50 * FIXED_POINT_SCALE / 100, // Tie
                frequency: 10,
                event_type: EventType::ElectionWithTies,
            },
            HistoricalOutcome {
                value: 75 * FIXED_POINT_SCALE / 100, // Win
                frequency: 60,
                event_type: EventType::ElectionWithTies,
            },
        ];

        let dist = multimodal.optimize_for_event_outcome(
            EventType::ElectionWithTies,
            &historical_data,
        ).unwrap();

        // Should have three distinct regions
        let peaks = find_local_maxima(&dist.points);
        assert!(peaks.len() >= 2, "Election distribution should have multiple peaks");
    }

    #[test]
    fn test_max_bound_constraint() {
        let mut amm = L2DistributionAMM::new(
            L2_NORM_K,
            1000, // Very low max bound
        );

        let dist = create_high_value_distribution();
        
        // Apply max bound
        let bounded = amm.apply_max_bound(&dist).unwrap();
        
        // Check all points respect max bound
        for point in &bounded.points {
            assert!(point.f_x <= 1000, "Point {} exceeds max bound", point.f_x);
        }
    }

    #[test]
    fn test_distribution_pricing() {
        let mut amm = L2DistributionAMM::new(
            L2_NORM_K,
            MAX_F_BOUND,
        );

        let dist = create_test_distribution();
        let bet_amount = 100 * FIXED_POINT_SCALE;
        let outcome_range = (40 * FIXED_POINT_SCALE / 100, 60 * FIXED_POINT_SCALE / 100);

        let price = amm.price_distribution_bet(&dist, bet_amount, outcome_range).unwrap();

        assert!(price.price > 0);
        assert!(price.price <= FIXED_POINT_SCALE); // Price should be <= 1
        assert!(price.probability > 0);
        assert!(price.probability <= FIXED_POINT_SCALE); // Probability should be <= 1
        assert!(price.slippage >= 0);
    }

    #[test]
    fn test_fixed_point_sqrt() {
        let amm = L2DistributionAMM::new(L2_NORM_K, MAX_F_BOUND);
        
        // Test sqrt(4) = 2
        let result = amm.fixed_sqrt(4 * FIXED_POINT_SCALE).unwrap();
        let expected = 2000; // 2 * 1000 (adjustment factor)
        assert!(abs_diff(result, expected) < 10);
        
        // Test sqrt(9) = 3
        let result = amm.fixed_sqrt(9 * FIXED_POINT_SCALE).unwrap();
        let expected = 3000; // 3 * 1000
        assert!(abs_diff(result, expected) < 10);
    }

    // Helper functions
    fn create_test_distribution() -> Distribution {
        let points = (0..SIMPSON_POINTS)
            .map(|i| {
                let x = i as u64 * FIXED_POINT_SCALE / (SIMPSON_POINTS as u64 - 1);
                let f_x = if i == SIMPSON_POINTS / 2 {
                    FIXED_POINT_SCALE / 2 // Peak in middle
                } else {
                    FIXED_POINT_SCALE / 10
                };
                DistributionPoint {
                    x,
                    f_x,
                    weight: FIXED_POINT_SCALE,
                }
            })
            .collect();

        Distribution {
            curve_type: CurveType::Custom { points: vec![] },
            points,
            l2_norm: L2_NORM_K,
        }
    }

    fn create_high_value_distribution() -> Distribution {
        let points = (0..20)
            .map(|i| {
                let x = i as u64 * FIXED_POINT_SCALE / 19;
                let f_x = 5000; // High value
                DistributionPoint {
                    x,
                    f_x,
                    weight: FIXED_POINT_SCALE,
                }
            })
            .collect();

        Distribution {
            curve_type: CurveType::Custom { points: vec![] },
            points,
            l2_norm: L2_NORM_K,
        }
    }

    fn find_local_maxima(points: &[DistributionPoint]) -> Vec<usize> {
        let mut maxima = vec![];
        
        for i in 1..points.len() - 1 {
            if points[i].f_x > points[i - 1].f_x && points[i].f_x > points[i + 1].f_x {
                maxima.push(i);
            }
        }
        
        maxima
    }

    fn abs_diff(a: u64, b: u64) -> u64 {
        if a > b { a - b } else { b - a }
    }
}