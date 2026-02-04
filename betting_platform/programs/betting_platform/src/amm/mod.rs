pub mod l2_distribution;
pub mod distribution_editor;
pub mod multimodal_distribution;
pub mod pm_amm;

pub use l2_distribution::*;
pub use distribution_editor::*;
pub use multimodal_distribution::*;
// Don't re-export pm_amm to avoid conflicts
// Users should import directly from pm_amm module if needed

#[cfg(test)]
mod tests {
    use super::*;
    // Use the L2 distribution's FIXED_POINT_SCALE explicitly
    use crate::amm::l2_distribution::FIXED_POINT_SCALE;
    
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