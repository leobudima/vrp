use crate::format::problem::*;
use crate::helpers::*;

#[test]
fn can_solve_problem_with_tiered_costs() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![create_delivery_job("job1", (1., 1.)), create_delivery_job("job2", (2., 2.))],
            ..create_empty_plan()
        },
        fleet: Fleet {
            vehicles: vec![VehicleType {
                type_id: "vehicle".to_string(),
                vehicle_ids: vec!["vehicle_1".to_string()],
                profile: create_default_vehicle_profile(),
                costs: VehicleCosts {
                    fixed: Some(100.),
                    distance: TieredCost::Tiered(vec![
                        CostTier { threshold: 0., cost: 1. },
                        CostTier { threshold: 5000., cost: 2. },
                    ]),
                    time: TieredCost::Tiered(vec![
                        CostTier { threshold: 0., cost: 0.5 },
                        CostTier { threshold: 600., cost: 1. },
                    ]),
                    calculation_mode: None,
                },
                shifts: vec![create_default_vehicle_shift()],
                capacity: vec![10],
                ..create_default_vehicle_type()
            }],
            profiles: create_default_matrix_profiles(),
            ..create_default_fleet()
        },
        ..create_empty_problem()
    };

    let solution = solve_with_cheapest_insertion(problem, None);
    
    // Test passes if we can solve a problem with tiered costs without panic
    // The key is that the solver doesn't crash when tiered costs are used
    println!("Solution with tiered costs completed successfully");
    println!("Solution cost: {}", solution.statistic.cost);
    println!("Tours: {}", solution.tours.len());
    
    // Basic sanity checks - the solver should produce some result
    assert!(solution.statistic.cost >= 0.);
    
    // If jobs are assigned, there should be tours
    if solution.unassigned.is_none() || solution.unassigned.as_ref().unwrap().is_empty() {
        assert!(solution.tours.len() > 0);
    }
}