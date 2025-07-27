use vrp_core::models::common::{CostTier, TieredCost, TieredCostCalculationMode, TieredCosts};
use rosomaxa::prelude::Float;

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

#[test]
fn test_highest_tier_calculation_mode() {
    // Create a tiered cost structure based on the user's example
    let tiered_cost = TieredCost::tiered(vec![
        CostTier { threshold: 0.0, cost: 2.0 },
        CostTier { threshold: 3.0, cost: 4.0 },
        CostTier { threshold: 5.0, cost: 5.0 },
    ]).unwrap();

    // For a 6h duration with highest tier mode: 6 * 5 = 30
    let cost = tiered_cost.calculate_cost_with_mode(6.0, &TieredCostCalculationMode::HighestTier);
    assert_eq!(cost, 30.0, "6h with highest tier mode should be 6 * 5 = 30");

    // For a 4h duration with highest tier mode: 4 * 4 = 16
    let cost = tiered_cost.calculate_cost_with_mode(4.0, &TieredCostCalculationMode::HighestTier);
    assert_eq!(cost, 16.0, "4h with highest tier mode should be 4 * 4 = 16");

    // For a 2h duration with highest tier mode: 2 * 2 = 4
    let cost = tiered_cost.calculate_cost_with_mode(2.0, &TieredCostCalculationMode::HighestTier);
    assert_eq!(cost, 4.0, "2h with highest tier mode should be 2 * 2 = 4");
}

#[test]
fn test_cumulative_calculation_mode() {
    // Create a tiered cost structure based on the user's example
    let tiered_cost = TieredCost::tiered(vec![
        CostTier { threshold: 0.0, cost: 2.0 },
        CostTier { threshold: 3.0, cost: 4.0 },
        CostTier { threshold: 5.0, cost: 5.0 },
    ]).unwrap();

    // For a 6h duration with cumulative mode: 3*2 + 2*4 + 1*5 = 6 + 8 + 5 = 19
    let cost = tiered_cost.calculate_cost_with_mode(6.0, &TieredCostCalculationMode::Cumulative);
    assert_eq!(cost, 19.0, "6h with cumulative mode should be 3*2 + 2*4 + 1*5 = 19");

    // For a 4h duration with cumulative mode: 3*2 + 1*4 = 6 + 4 = 10
    let cost = tiered_cost.calculate_cost_with_mode(4.0, &TieredCostCalculationMode::Cumulative);
    assert_eq!(cost, 10.0, "4h with cumulative mode should be 3*2 + 1*4 = 10");

    // For a 2h duration with cumulative mode: 2*2 = 4
    let cost = tiered_cost.calculate_cost_with_mode(2.0, &TieredCostCalculationMode::Cumulative);
    assert_eq!(cost, 4.0, "2h with cumulative mode should be 2*2 = 4");

    // For a 7h duration with cumulative mode: 3*2 + 2*4 + 2*5 = 6 + 8 + 10 = 24
    let cost = tiered_cost.calculate_cost_with_mode(7.0, &TieredCostCalculationMode::Cumulative);
    assert_eq!(cost, 24.0, "7h with cumulative mode should be 3*2 + 2*4 + 2*5 = 24");
}

#[test]
fn test_fixed_cost_with_both_modes() {
    let fixed_cost = TieredCost::fixed(3.0).unwrap();

    // Fixed costs should work the same regardless of calculation mode
    let highest_tier_cost = fixed_cost.calculate_cost_with_mode(10.0, &TieredCostCalculationMode::HighestTier);
    let cumulative_cost = fixed_cost.calculate_cost_with_mode(10.0, &TieredCostCalculationMode::Cumulative);

    assert_eq!(highest_tier_cost, 30.0, "Fixed cost with highest tier: 10 * 3 = 30");
    assert_eq!(cumulative_cost, 30.0, "Fixed cost with cumulative: 10 * 3 = 30");
    assert_eq!(highest_tier_cost, cumulative_cost, "Fixed costs should be identical regardless of mode");
}

#[test]
fn test_tiered_costs_struct_with_calculation_modes() {
    let distance_tiers = TieredCost::tiered(vec![
        CostTier { threshold: 0.0, cost: 1.0 },
        CostTier { threshold: 100.0, cost: 2.0 },
    ]).unwrap();
    
    let time_tiers = TieredCost::tiered(vec![
        CostTier { threshold: 0.0, cost: 0.5 },
        CostTier { threshold: 60.0, cost: 1.0 },
    ]).unwrap();

    // Test highest tier mode
    let highest_tier_costs = TieredCosts::new(
        distance_tiers.clone(),
        time_tiers.clone(),
        TieredCostCalculationMode::HighestTier,
    );
    
    // Test cumulative mode
    let cumulative_costs = TieredCosts::new(
        distance_tiers,
        time_tiers,
        TieredCostCalculationMode::Cumulative,
    );

    // Test backward compatibility constructor
    let backward_compatible_costs = TieredCosts::with_highest_tier_mode(
        TieredCost::fixed(1.0).unwrap(),
        TieredCost::fixed(0.5).unwrap(),
    );

    assert_eq!(highest_tier_costs.calculation_mode, TieredCostCalculationMode::HighestTier);
    assert_eq!(cumulative_costs.calculation_mode, TieredCostCalculationMode::Cumulative);
    assert_eq!(backward_compatible_costs.calculation_mode, TieredCostCalculationMode::HighestTier);
}

#[test]
fn test_calculation_mode_edge_cases() {
    let tiered_cost = TieredCost::tiered(vec![
        CostTier { threshold: 0.0, cost: 2.0 },
        CostTier { threshold: 5.0, cost: 4.0 },
    ]).unwrap();

    // Test exact threshold boundaries
    let cost_at_threshold_highest = tiered_cost.calculate_cost_with_mode(5.0, &TieredCostCalculationMode::HighestTier);
    let cost_at_threshold_cumulative = tiered_cost.calculate_cost_with_mode(5.0, &TieredCostCalculationMode::Cumulative);
    
    assert_eq!(cost_at_threshold_highest, 20.0, "At threshold 5.0, highest tier: 5 * 4 = 20");
    assert_eq!(cost_at_threshold_cumulative, 10.0, "At threshold 5.0, cumulative: 5 * 2 = 10");

    // Test zero value
    let cost_zero_highest = tiered_cost.calculate_cost_with_mode(0.0, &TieredCostCalculationMode::HighestTier);
    let cost_zero_cumulative = tiered_cost.calculate_cost_with_mode(0.0, &TieredCostCalculationMode::Cumulative);
    
    assert_eq!(cost_zero_highest, 0.0, "Zero value should result in zero cost for any mode");
    assert_eq!(cost_zero_cumulative, 0.0, "Zero value should result in zero cost for any mode");
}
