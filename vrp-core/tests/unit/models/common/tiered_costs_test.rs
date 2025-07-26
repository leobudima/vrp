use vrp_core::models::common::{CostTier, TieredCost};
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
