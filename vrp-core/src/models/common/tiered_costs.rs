use rosomaxa::prelude::Float;

/// Represents a cost tier with a threshold and associated cost.
#[derive(Clone, Debug, PartialEq)]
pub struct CostTier {
    /// The threshold value above which this tier applies.
    pub threshold: Float,
    /// The cost per unit for this tier.
    pub cost: Float,
}

impl CostTier {
    /// Creates a new cost tier with validation.
    pub fn new(threshold: Float, cost: Float) -> Result<Self, String> {
        if threshold < 0.0 {
            return Err(format!("Threshold must be non-negative, got: {}", threshold));
        }
        if cost < 0.0 {
            return Err(format!("Cost must be non-negative, got: {}", cost));
        }
        if !threshold.is_finite() {
            return Err("Threshold must be a finite number".to_string());
        }
        if !cost.is_finite() {
            return Err("Cost must be a finite number".to_string());
        }
        
        Ok(CostTier { threshold, cost })
    }
}

/// Represents either a fixed cost or a list of tiered costs.
/// Tiers are automatically sorted by threshold in ascending order during construction.
#[derive(Clone, Debug, PartialEq)]
pub enum TieredCost {
    /// Fixed cost per unit.
    Fixed(Float),
    /// List of cost tiers, sorted by threshold in ascending order.
    Tiered(Vec<CostTier>),
}

impl TieredCost {
    /// Calculates the cost rate for a given total value using the appropriate tier.
    /// Uses binary search for efficient tier lookup.
    pub fn calculate_rate(&self, total_value: Float) -> Float {
        match self {
            TieredCost::Fixed(cost) => *cost,
            TieredCost::Tiered(tiers) => {
                if tiers.is_empty() {
                    return 0.0;
                }
                
                // Find the highest threshold <= total_value using binary search
                // We want to find the rightmost tier where threshold <= total_value
                let mut left = 0;
                let mut right = tiers.len();
                let mut result_idx = 0;
                
                while left < right {
                    let mid = left + (right - left) / 2;
                    if tiers[mid].threshold <= total_value {
                        result_idx = mid;
                        left = mid + 1;
                    } else {
                        right = mid;
                    }
                }
                
                tiers[result_idx].cost
            }
        }
    }

    /// Creates a fixed cost with validation.
    pub fn fixed(cost: Float) -> Result<Self, String> {
        if cost < 0.0 {
            return Err(format!("Fixed cost must be non-negative, got: {}", cost));
        }
        if !cost.is_finite() {
            return Err("Fixed cost must be a finite number".to_string());
        }
        Ok(TieredCost::Fixed(cost))
    }

    /// Creates a tiered cost from a list of tiers with validation and sorting.
    pub fn tiered(mut tiers: Vec<CostTier>) -> Result<Self, String> {
        if tiers.is_empty() {
            return Err("Tiered costs cannot have empty tier list".to_string());
        }
        
        // Validate that we have a tier starting at 0.0
        let has_zero_threshold = tiers.iter().any(|tier| (tier.threshold - 0.0).abs() < f64::EPSILON);
        if !has_zero_threshold {
            return Err("Tiered costs must include a tier with threshold 0.0".to_string());
        }
        
        // Check for duplicate thresholds
        tiers.sort_by(|a, b| a.threshold.partial_cmp(&b.threshold).unwrap_or(std::cmp::Ordering::Equal));
        for window in tiers.windows(2) {
            if let [tier1, tier2] = window {
                if (tier1.threshold - tier2.threshold).abs() < f64::EPSILON {
                    return Err(format!("Duplicate threshold found: {}", tier1.threshold));
                }
            }
        }
        
        Ok(TieredCost::Tiered(tiers))
    }

    /// Creates a tiered cost from a list of tiers without validation (for internal use).
    /// Assumes tiers are already validated and sorted.
    pub(crate) fn tiered_unchecked(tiers: Vec<CostTier>) -> Self {
        TieredCost::Tiered(tiers)
    }

    /// Returns true if this is a fixed cost.
    pub fn is_fixed(&self) -> bool {
        matches!(self, TieredCost::Fixed(_))
    }

    /// Returns true if this is a tiered cost.
    pub fn is_tiered(&self) -> bool {
        matches!(self, TieredCost::Tiered(_))
    }

    /// Returns the number of tiers (1 for fixed costs, actual count for tiered costs).
    pub fn tier_count(&self) -> usize {
        match self {
            TieredCost::Fixed(_) => 1,
            TieredCost::Tiered(tiers) => tiers.len(),
        }
    }
}

/// Represents tiered operating costs for driver and vehicle.
/// This is used alongside the regular Costs structure for backward compatibility.
#[derive(Clone, Debug)]
pub struct TieredCosts {
    /// Cost per distance unit - can be tiered.
    pub per_distance: TieredCost,
    /// Cost per driving time unit - can be tiered.
    pub per_driving_time: TieredCost,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiered_cost_calculation() {
        // Test case matching our validation scenario
        let distance_tiers = vec![
            CostTier { threshold: 0.0, cost: 1.0 },
            CostTier { threshold: 5000.0, cost: 2.0 },
            CostTier { threshold: 10000.0, cost: 3.0 },
        ];
        
        let time_tiers = vec![
            CostTier { threshold: 0.0, cost: 0.5 },
            CostTier { threshold: 600.0, cost: 1.0 },
            CostTier { threshold: 1200.0, cost: 1.5 },
        ];
        
        let distance_tiered_cost = TieredCost::tiered(distance_tiers).unwrap();
        let time_tiered_cost = TieredCost::tiered(time_tiers).unwrap();
        
        // Test values from our validation case
        let total_distance = 7976.0;
        let total_time = 798.0;
        
        let distance_rate = distance_tiered_cost.calculate_rate(total_distance);
        let time_rate = time_tiered_cost.calculate_rate(total_time);
        
        // Verify tier selection
        assert_eq!(distance_rate, 2.0, "Distance {} should use tier rate 2.0", total_distance);
        assert_eq!(time_rate, 1.0, "Time {} should use tier rate 1.0", total_time);
        
        // Calculate expected total cost
        let distance_cost = total_distance * distance_rate;
        let time_cost = total_time * time_rate;
        let service_cost = 300.0 * time_rate; // Service uses same rate as time
        let fixed_cost = 100.0;
        
        let expected_total = fixed_cost + distance_cost + time_cost + service_cost;
        
        println!("Tiered cost calculation test:");
        println!("Distance: {} * {} = {}", total_distance, distance_rate, distance_cost);
        println!("Driving time: {} * {} = {}", total_time, time_rate, time_cost);
        println!("Service time: {} * {} = {}", 300.0, time_rate, service_cost);
        println!("Fixed: {}", fixed_cost);
        println!("Expected total: {}", expected_total);
        
        assert_eq!(expected_total, 17150.0, "Expected total cost should be 17150.0");
    }

    #[test]
    fn test_tier_boundary_selection() {
        let distance_cost = TieredCost::tiered(vec![
            CostTier { threshold: 0.0, cost: 1.0 },
            CostTier { threshold: 100.0, cost: 2.0 },
            CostTier { threshold: 200.0, cost: 3.0 },
        ]).unwrap();

        // Test tier boundaries
        assert_eq!(distance_cost.calculate_rate(0.0), 1.0);
        assert_eq!(distance_cost.calculate_rate(50.0), 1.0);
        assert_eq!(distance_cost.calculate_rate(99.9), 1.0);
        assert_eq!(distance_cost.calculate_rate(100.0), 2.0);
        assert_eq!(distance_cost.calculate_rate(150.0), 2.0);
        assert_eq!(distance_cost.calculate_rate(199.9), 2.0);
        assert_eq!(distance_cost.calculate_rate(200.0), 3.0);
        assert_eq!(distance_cost.calculate_rate(500.0), 3.0);
    }

    #[test]
    fn test_validation() {
        // Test negative threshold
        assert!(CostTier::new(-1.0, 1.0).is_err());
        
        // Test negative cost
        assert!(CostTier::new(1.0, -1.0).is_err());
        
        // Test infinite values
        assert!(CostTier::new(Float::INFINITY, 1.0).is_err());
        assert!(CostTier::new(1.0, Float::INFINITY).is_err());
        
        // Test missing zero threshold
        assert!(TieredCost::tiered(vec![
            CostTier { threshold: 10.0, cost: 1.0 },
        ]).is_err());
        
        // Test duplicate thresholds
        assert!(TieredCost::tiered(vec![
            CostTier { threshold: 0.0, cost: 1.0 },
            CostTier { threshold: 0.0, cost: 2.0 },
        ]).is_err());
    }
} 