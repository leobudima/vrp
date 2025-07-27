use rosomaxa::prelude::Float;

/// Determines how tiered costs are calculated.
#[derive(Clone, Debug, PartialEq)]
pub enum TieredCostCalculationMode {
    /// Uses the rate of the highest applicable tier for the entire amount.
    /// Example: 6h with tiers [(0,2), (3,4), (5,5)] = 6 * 5 = 30
    HighestTier,
    /// Uses cumulative calculation where each tier is applied up to its threshold.
    /// Example: 6h with tiers [(0,2), (3,4), (5,5)] = 3*2 + 2*4 + 1*5 = 19
    Cumulative,
}

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
    /// This method maintains backward compatibility and defaults to HighestTier mode.
    pub fn calculate_rate(&self, total_value: Float) -> Float {
        self.calculate_rate_with_mode(total_value, &TieredCostCalculationMode::HighestTier)
    }

    /// Calculates the cost for a given total value using the specified calculation mode.
    pub fn calculate_cost_with_mode(&self, total_value: Float, mode: &TieredCostCalculationMode) -> Float {
        match mode {
            TieredCostCalculationMode::HighestTier => {
                total_value * self.calculate_rate_with_mode(total_value, mode)
            }
            TieredCostCalculationMode::Cumulative => {
                self.calculate_cumulative_cost(total_value)
            }
        }
    }

    /// Calculates the cost rate for a given total value using the specified calculation mode.
    pub fn calculate_rate_with_mode(&self, total_value: Float, mode: &TieredCostCalculationMode) -> Float {
        match mode {
            TieredCostCalculationMode::HighestTier => {
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
            TieredCostCalculationMode::Cumulative => {
                // For cumulative mode, there's no single rate - the cost is calculated directly
                // Return 0 as rate is not meaningful in cumulative mode
                0.0
            }
        }
    }

    /// Calculates the cumulative cost by applying each tier to its respective portion.
    fn calculate_cumulative_cost(&self, total_value: Float) -> Float {
        match self {
            TieredCost::Fixed(cost) => total_value * cost,
            TieredCost::Tiered(tiers) => {
                if tiers.is_empty() {
                    return 0.0;
                }

                let mut total_cost = 0.0;
                let mut remaining_value = total_value;

                for i in 0..tiers.len() {
                    let current_tier = &tiers[i];
                    
                    // Determine the upper bound for this tier
                    let upper_bound = if i + 1 < tiers.len() {
                        tiers[i + 1].threshold
                    } else {
                        total_value // For the last tier, use the total value
                    };

                    // Calculate how much value applies to this tier
                    let tier_value = if remaining_value > 0.0 {
                        (upper_bound - current_tier.threshold).min(remaining_value)
                    } else {
                        0.0
                    };

                    if tier_value > 0.0 {
                        total_cost += tier_value * current_tier.cost;
                        remaining_value -= tier_value;
                    }

                    if remaining_value <= 0.0 {
                        break;
                    }
                }

                total_cost
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
    /// Calculation mode for tiered costs.
    pub calculation_mode: TieredCostCalculationMode,
}

impl TieredCosts {
    /// Creates a new TieredCosts with the specified calculation mode.
    pub fn new(
        per_distance: TieredCost,
        per_driving_time: TieredCost,
        calculation_mode: TieredCostCalculationMode,
    ) -> Self {
        Self {
            per_distance,
            per_driving_time,
            calculation_mode,
        }
    }

    /// Creates a new TieredCosts with default HighestTier calculation mode for backward compatibility.
    pub fn with_highest_tier_mode(per_distance: TieredCost, per_driving_time: TieredCost) -> Self {
        Self::new(per_distance, per_driving_time, TieredCostCalculationMode::HighestTier)
    }
}

 