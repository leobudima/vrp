use super::*;
use crate::helpers::{SIMPLE_MATRIX, SIMPLE_PROBLEM};
use std::io::BufReader;

fn assert_time_windows(actual: &Option<Vec<Vec<String>>>, expected: (&str, &str)) {
    let actual = actual.as_ref().unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual.first().unwrap().len(), 2);
    assert_eq!(actual.first().unwrap().first().unwrap(), expected.0);
    assert_eq!(actual.first().unwrap().last().unwrap(), expected.1);
}

fn assert_location(actual: &Location, expected: (f64, f64)) {
    let (lat, lng) = actual.to_lat_lng();

    assert_eq!(lat, expected.0);
    assert_eq!(lng, expected.1);
}

fn assert_demand(actual: &Option<Vec<i32>>, expected: i32) {
    let actual = actual.as_ref().expect("Empty demand!");
    assert_eq!(actual.len(), 1);
    assert_eq!(*actual.first().unwrap(), expected);
}

#[test]
fn can_deserialize_problem() {
    let problem = deserialize_problem(BufReader::new(SIMPLE_PROBLEM.as_bytes())).ok().unwrap();

    assert_eq!(problem.plan.jobs.len(), 2);
    assert_eq!(problem.fleet.vehicles.len(), 1);
    assert!(problem.plan.relations.is_none());

    // validate jobs
    let job = problem.plan.jobs.first().unwrap();
    assert_eq!(job.id, "single_job");
    assert!(job.pickups.is_none());
    assert!(job.deliveries.is_some());
    assert!(job.skills.is_none());

    let deliveries = job.deliveries.as_ref().unwrap();
    assert_eq!(deliveries.len(), 1);
    let delivery = deliveries.first().unwrap();
    assert_demand(&delivery.demand, 1);
    assert!(delivery.places.first().unwrap().tag.is_none());

    assert_eq!(delivery.places.len(), 1);
    let place = delivery.places.first().unwrap();
    assert_eq!(place.duration, 240.);
    assert_location(&place.location, (52.5622847f64, 13.4023099f64));
    assert_time_windows(&place.times, ("2019-07-04T10:00:00Z", "2019-07-04T16:00:00Z"));

    let job = problem.plan.jobs.last().unwrap();
    assert_eq!(job.id, "multi_job");
    assert!(job.skills.is_none());
    assert_eq!(job.pickups.as_ref().unwrap().len(), 2);
    assert_eq!(job.deliveries.as_ref().unwrap().len(), 1);
}

#[test]
fn can_deserialize_matrix() {
    let matrix = deserialize_matrix(BufReader::new(SIMPLE_MATRIX.as_bytes())).ok().unwrap();

    assert_eq!(matrix.distances.len(), 16);
    assert_eq!(matrix.travel_times.len(), 16);
}

mod tiered_costs {
    use super::*;
    use serde_json::json;

    #[test]
    fn can_deserialize_fixed_cost_format() {
        let json_data = json!({
            "fixed": 100,
            "distance": 1.5,
            "time": 2.0
        });

        let costs: VehicleCosts = serde_json::from_value(json_data).expect("Should deserialize fixed costs");
        
        assert_eq!(costs.fixed, Some(100.0));
        assert_eq!(costs.distance, TieredCost::Fixed(1.5));
        assert_eq!(costs.time, TieredCost::Fixed(2.0));
    }

    #[test]
    fn can_deserialize_tiered_cost_format() {
        let json_data = json!({
            "fixed": 50,
            "distance": [
                {"threshold": 0, "cost": 1.0},
                {"threshold": 100, "cost": 2.0},
                {"threshold": 200, "cost": 3.0}
            ],
            "time": [
                {"threshold": 0, "cost": 0.5},
                {"threshold": 60, "cost": 1.0}
            ]
        });

        let costs: VehicleCosts = serde_json::from_value(json_data).expect("Should deserialize tiered costs");
        
        assert_eq!(costs.fixed, Some(50.0));
        
        match costs.distance {
            TieredCost::Tiered(tiers) => {
                assert_eq!(tiers.len(), 3);
                assert_eq!(tiers[0].threshold, 0.0);
                assert_eq!(tiers[0].cost, 1.0);
                assert_eq!(tiers[2].threshold, 200.0);
                assert_eq!(tiers[2].cost, 3.0);
            }
            _ => panic!("Expected tiered cost for distance"),
        }
        
        match costs.time {
            TieredCost::Tiered(tiers) => {
                assert_eq!(tiers.len(), 2);
                assert_eq!(tiers[1].threshold, 60.0);
                assert_eq!(tiers[1].cost, 1.0);
            }
            _ => panic!("Expected tiered cost for time"),
        }
    }

    #[test]
    fn can_calculate_tiered_cost_rates() {
        let distance_cost = TieredCost::Tiered(vec![
            CostTier { threshold: 0.0, cost: 1.0 },
            CostTier { threshold: 50.0, cost: 1.5 },
            CostTier { threshold: 100.0, cost: 2.0 },
        ]);

        // Test rate calculation for different total values
        assert_eq!(distance_cost.calculate_cost(0.0), 1.0);
        assert_eq!(distance_cost.calculate_cost(25.0), 1.0);
        assert_eq!(distance_cost.calculate_cost(49.9), 1.0);
        assert_eq!(distance_cost.calculate_cost(50.0), 1.5);
        assert_eq!(distance_cost.calculate_cost(75.0), 1.5);
        assert_eq!(distance_cost.calculate_cost(99.9), 1.5);
        assert_eq!(distance_cost.calculate_cost(100.0), 2.0);
        assert_eq!(distance_cost.calculate_cost(150.0), 2.0);
    }

    #[test]
    fn can_serialize_and_deserialize_roundtrip() {
        let original_costs = VehicleCosts {
            fixed: Some(100.0),
            distance: TieredCost::Tiered(vec![
                CostTier { threshold: 0.0, cost: 1.0 },
                CostTier { threshold: 100.0, cost: 2.0 },
            ]),
            time: TieredCost::Fixed(1.5),
        };

        let json_value = serde_json::to_value(&original_costs).expect("Should serialize");
        let deserialized_costs: VehicleCosts = serde_json::from_value(json_value).expect("Should deserialize");
        
        assert_eq!(deserialized_costs.fixed, original_costs.fixed);
        assert_eq!(deserialized_costs.time, original_costs.time);
        
        match (&deserialized_costs.distance, &original_costs.distance) {
            (TieredCost::Tiered(d_tiers), TieredCost::Tiered(o_tiers)) => {
                assert_eq!(d_tiers.len(), o_tiers.len());
                for (d_tier, o_tier) in d_tiers.iter().zip(o_tiers.iter()) {
                    assert_eq!(d_tier.threshold, o_tier.threshold);
                    assert_eq!(d_tier.cost, o_tier.cost);
                }
            }
            _ => panic!("Distance cost types should match"),
        }
    }
}
