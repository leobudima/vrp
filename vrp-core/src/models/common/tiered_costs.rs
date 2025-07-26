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
    pub fn tiered_unchecked(tiers: Vec<CostTier>) -> Self {
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

 