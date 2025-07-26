use super::*;
use crate::helpers::models::solution::test_actor_with_profile;
use crate::helpers::models::problem::TestSingleBuilder;

fn create_matrix_data(
    profile: Profile,
    timestamp: Option<Timestamp>,
    duration: (Duration, usize),
    distance: (Distance, usize),
) -> MatrixData {
    MatrixData {
        index: profile.index,
        timestamp,
        durations: vec![duration.0; duration.1],
        distances: vec![distance.0; distance.1],
    }
}

#[test]
fn can_detect_dimensions_mismatch() {
    assert_eq!(
        create_matrix_transport_cost(vec![
            create_matrix_data(Profile::default(), Some(0.), (0., 2), (0., 2)),
            create_matrix_data(Profile::default(), Some(1.), (0., 1), (0., 2)),
        ])
        .err(),
        Some("distance and duration collections have different length".into())
    );
}

#[test]
fn can_return_error_when_mixing_timestamps() {
    let p0 = Profile::default();
    let p1 = Profile::new(1, None);

    assert_eq!(
        TimeAwareMatrixTransportCost::new(
            vec![create_matrix_data(Profile::default(), None, (0., 1), (0., 1))],
            1,
            NoFallback
        )
        .err(),
        Some("time-aware routing requires all matrices to have timestamp".into())
    );

    assert_eq!(
        TimeAwareMatrixTransportCost::new(
            vec![
                create_matrix_data(p0.clone(), Some(0.), (0., 1), (0., 1)),
                create_matrix_data(p0.clone(), None, (0., 1), (0., 1))
            ],
            1,
            NoFallback
        )
        .err(),
        Some("time-aware routing requires all matrices to have timestamp".into())
    );

    assert_eq!(
        TimeAwareMatrixTransportCost::new(
            vec![create_matrix_data(p0.clone(), Some(0.), (0., 1), (0., 1))],
            1,
            NoFallback
        )
        .err(),
        Some("should not use time aware matrix routing with single matrix".into())
    );

    assert_eq!(
        TimeAwareMatrixTransportCost::new(
            vec![
                create_matrix_data(p0.clone(), Some(0.), (1., 1), (1., 1)), //
                create_matrix_data(p0, Some(1.), (1., 1), (1., 1)),         //
                create_matrix_data(p1, Some(0.), (1., 1), (1., 1)),         //
            ],
            1,
            NoFallback
        )
        .err(),
        Some("should not use time aware matrix routing with single matrix".into())
    );
}

#[test]
fn can_interpolate_durations() {
    let route0 = Route { actor: test_actor_with_profile(0), tour: Default::default() };
    let route1 = Route { actor: test_actor_with_profile(1), tour: Default::default() };
    let p0 = route0.actor.vehicle.profile.clone();
    let p1 = route1.actor.vehicle.profile.clone();

    let costs = TimeAwareMatrixTransportCost::new(
        vec![
            create_matrix_data(p0.clone(), Some(0.), (100., 2), (1., 2)),
            create_matrix_data(p0.clone(), Some(10.), (200., 2), (1., 2)),
            create_matrix_data(p1.clone(), Some(0.), (300., 2), (5., 2)),
            create_matrix_data(p1.clone(), Some(10.), (400., 2), (5., 2)),
        ],
        2,
        NoFallback,
    )
    .unwrap();

    for &(timestamp, duration) in &[(0., 100.), (10., 200.), (15., 200.), (3., 130.), (5., 150.), (7., 170.)] {
        assert_eq!(costs.duration(&route0, 0, 1, TravelTime::Departure(timestamp)), duration);
    }

    for &(timestamp, duration) in &[(0., 300.), (10., 400.), (15., 400.), (3., 330.), (5., 350.), (7., 370.)] {
        assert_eq!(costs.duration(&route1, 0, 1, TravelTime::Departure(timestamp)), duration);
    }

    assert_eq!(costs.distance(&route0, 0, 1, TravelTime::Departure(0.)), 1.);
    assert_eq!(costs.distance(&route1, 0, 1, TravelTime::Departure(0.)), 5.);

    assert_eq!(costs.distance_approx(&p0, 0, 1), 1.);
    assert_eq!(costs.distance_approx(&p1, 0, 1), 5.);
}

mod objective {
    use super::*;
    use crate::construction::heuristics::{InsertionContext, MoveContext};
    use crate::helpers::construction::heuristics::TestInsertionContextBuilder;
    use crate::models::{Feature, FeatureBuilder, FeatureObjective, GoalContextBuilder};
    use rosomaxa::prelude::HeuristicObjective;
    use std::cmp::Ordering;

    struct TestObjective {
        index: usize,
    }

    impl FeatureObjective for TestObjective {
        fn fitness(&self, solution: &InsertionContext) -> Cost {
            solution
                .solution
                .state
                .get_value::<(), Vec<Float>>()
                .and_then(|data| data.get(self.index))
                .cloned()
                .unwrap()
        }

        fn estimate(&self, _: &MoveContext<'_>) -> Cost {
            Cost::default()
        }
    }

    fn create_objective_feature(index: usize) -> Feature {
        FeatureBuilder::default()
            .with_name(format!("test_{index}").as_str())
            .with_objective(TestObjective { index })
            .build()
            .unwrap()
    }

    fn create_individual(data: Vec<Float>) -> InsertionContext {
        TestInsertionContextBuilder::default().with_state(|state| state.set_value::<(), _>(data)).build()
    }

    parameterized_test! {can_use_total_order, (data_a, data_b, expected), {
        can_use_total_order_impl(data_a, data_b, expected);
    }}

    can_use_total_order! {
        case01: (vec![0., 1., 2.], vec![0., 1., 2.], Ordering::Equal),
        case02: (vec![1., 1., 2.], vec![0., 1., 2.], Ordering::Greater),
        case03: (vec![0., 1., 2.], vec![1., 1., 2.], Ordering::Less),
        case04: (vec![0., 1., 2.], vec![0., 2., 2.], Ordering::Less),
        case05: (vec![0., 2., 2.], vec![1., 0., 0.], Ordering::Less),
    }

    fn can_use_total_order_impl(data_a: Vec<Float>, data_b: Vec<Float>, expected: Ordering) {
        let features = vec![create_objective_feature(0), create_objective_feature(1), create_objective_feature(2)];
        let goal_ctx = GoalContextBuilder::with_features(&features)
            .expect("cannot create builder")
            .build()
            .expect("cannot build context");

        let a = create_individual(data_a);
        let b = create_individual(data_b);

        let result = goal_ctx.total_order(&a, &b);

        assert_eq!(result, expected);
    }
}

mod tiered_costs {
    use crate::helpers::models::problem::*;
    use crate::models::common::*;
    use crate::models::problem::*;
    use crate::models::solution::{Activity, Route, Tour, Place as SolutionPlace};
    use std::sync::Arc;

    fn create_test_tiered_costs() -> TieredCosts {
        TieredCosts {
            per_distance: TieredCost::tiered(vec![
                CostTier { threshold: 0.0, cost: 1.0 },
                CostTier { threshold: 100.0, cost: 2.0 },
                CostTier { threshold: 200.0, cost: 3.0 },
            ]).unwrap(),
            per_driving_time: TieredCost::tiered(vec![
                CostTier { threshold: 0.0, cost: 0.5 },
                CostTier { threshold: 50.0, cost: 1.0 },
                CostTier { threshold: 100.0, cost: 1.5 },
            ]).unwrap(),
        }
    }

    fn create_test_transport_cost() -> Arc<dyn TransportCost> {
        Arc::new(SimpleTransportCost::new(
            vec![0., 10., 20., 10., 0., 30., 20., 30., 0.], // durations
            vec![0., 100., 200., 100., 0., 300., 200., 300., 0.], // distances
        ).unwrap())
    }

    fn create_test_vehicle_with_tiered_costs() -> Vehicle {
        Vehicle {
            profile: Profile::default(),
            costs: test_costs(),
            tiered_costs: Some(create_test_tiered_costs()),
            dimens: Default::default(),
            details: vec![test_vehicle_detail()],
        }
    }

    #[test]
    fn test_tiered_cost_tier_selection() {
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
    fn test_coordinated_cost_calculator_shares_route_totals() {
        let transport_cost = create_test_transport_cost();
        let calculator = CoordinatedCostCalculator::new(transport_cost.clone());
        
        // Create a test route with activities
        let vehicle = Arc::new(create_test_vehicle_with_tiered_costs());
        let driver = Arc::new(test_driver());
        let actor = Arc::new(Actor {
            vehicle: vehicle.clone(),
            driver: driver.clone(),
            detail: ActorDetail {
                start: Some(VehiclePlace { location: 0, time: TimeInterval { earliest: Some(0.), latest: None } }),
                end: Some(VehiclePlace { location: 0, time: TimeInterval { earliest: None, latest: Some(1000.) } }),
                time: TimeWindow { start: 0., end: 1000. },
            },
        });
        
        let mut tour = Tour::new(&actor);
        
        // Add activities at different locations - use helper to create proper activities
        let job1 = TestSingleBuilder::default().build_shared();
        let job2 = TestSingleBuilder::default().build_shared();
        
        let activity1 = Activity {
            place: SolutionPlace { idx: 0, location: 1, duration: 10., time: TimeWindow::new(0., 1000.) },
            schedule: crate::models::common::Schedule { arrival: 10., departure: 20. },
            job: Some(job1),
            commute: None,
        };
        
        let activity2 = Activity {
            place: SolutionPlace { idx: 1, location: 2, duration: 20., time: TimeWindow::new(0., 1000.) },
            schedule: crate::models::common::Schedule { arrival: 50., departure: 70. },
            job: Some(job2),
            commute: None,
        };
        
        tour.insert_at(activity1, 1);
        tour.insert_at(activity2, 2);
        
        let route = Route { actor, tour };
        
        // Both transport and activity costs should use the same route totals
        let route_totals_1 = calculator.get_route_totals(&route);
        let route_totals_2 = calculator.calculate_route_totals(&route);
        
        assert_eq!(route_totals_1, route_totals_2);
        
        // Verify route totals are calculated correctly
        // Route: 0 -> 1 -> 2 -> 0, so distances: 100 + 300 + 200 = 600, durations: 10 + 30 + 20 = 60
        assert_eq!(route_totals_1.0, 600.0); // total distance
        assert_eq!(route_totals_1.1, 60.0);  // total duration
    }

    #[test]
    fn test_transport_cost_with_tiered_costs() {
        let transport_cost = create_test_transport_cost();
        let calculator = CoordinatedCostCalculator::new(transport_cost.clone());
        
        let vehicle = Arc::new(create_test_vehicle_with_tiered_costs());
        let driver = Arc::new(test_driver());
        let actor = Arc::new(Actor {
            vehicle: vehicle.clone(),
            driver: driver.clone(),
            detail: ActorDetail {
                start: Some(VehiclePlace { location: 0, time: TimeInterval { earliest: Some(0.), latest: None } }),
                end: Some(VehiclePlace { location: 0, time: TimeInterval { earliest: None, latest: Some(1000.) } }),
                time: TimeWindow { start: 0., end: 1000. },
            },
        });
        
        let mut tour = Tour::new(&actor);
        let job = TestSingleBuilder::default().build_shared();
        tour.insert_at(Activity {
            place: SolutionPlace { idx: 0, location: 1, duration: 10., time: TimeWindow::new(0., 1000.) },
            schedule: crate::models::common::Schedule { arrival: 10., departure: 20. },
            job: Some(job),
            commute: None,
        }, 1);
        
        let route = Route { actor, tour };
        
        // Calculate transport cost between locations 0 and 1
        let cost = TransportCost::cost(&calculator, &route, 0, 1, TravelTime::Departure(0.));
        
        // Route totals: distance=100, duration=10 (for single segment route 0->1)
        // Distance tier: 100 -> rate 2.0, so distance cost = 100 * 2.0 = 200
        // Duration tier: 10 -> rate 0.5, so duration cost = 10 * 0.5 = 5
        // Expected total: 200 + 5 = 205
        assert_eq!(cost, 205.0);
    }

    #[test]
    fn test_activity_cost_with_tiered_costs() {
        let transport_cost = create_test_transport_cost();
        let calculator = CoordinatedCostCalculator::new(transport_cost.clone());
        
        let vehicle = Arc::new(create_test_vehicle_with_tiered_costs());
        let driver = Arc::new(test_driver());
        let actor = Arc::new(Actor {
            vehicle: vehicle.clone(),
            driver: driver.clone(),
            detail: ActorDetail {
                start: Some(VehiclePlace { location: 0, time: TimeInterval { earliest: Some(0.), latest: None } }),
                end: Some(VehiclePlace { location: 0, time: TimeInterval { earliest: None, latest: Some(1000.) } }),
                time: TimeWindow { start: 0., end: 1000. },
            },
        });
        
        let mut tour = Tour::new(&actor);
        let job = TestSingleBuilder::default().build_shared();
        let activity = Activity {
            place: SolutionPlace { idx: 0, location: 1, duration: 30., time: TimeWindow::new(0., 1000.) },
            schedule: crate::models::common::Schedule { arrival: 10., departure: 40. },
            job: Some(job),
            commute: None,
        };
        tour.insert_at(activity, 1);
        
        let route = Route { actor, tour };
        
        // Calculate activity cost (no waiting time, just service time)
        let activity_ref = route.tour.get(1).unwrap();
        let cost = ActivityCost::cost(&calculator, &route, activity_ref, 10.); // arrival = 10, start = 0, no waiting
        
        // Route totals: distance=100, duration=10 (for single segment route 0->1)  
        // Duration tier: 10 -> rate 0.5
        // Service time cost = 30 * 0.5 = 15
        // Waiting time cost = 0 * 0.5 = 0
        // Expected total: 15 + 0 = 15
        assert_eq!(cost, 15.0);
    }
}
