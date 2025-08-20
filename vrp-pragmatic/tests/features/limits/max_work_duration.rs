use crate::format::problem::*;
use crate::helpers::*;
use vrp_core::prelude::Float;

fn create_vehicle_type_with_max_work_duration_limit(max_work_duration: Float) -> VehicleType {
    VehicleType {
        limits: Some(VehicleLimits { 
            max_distance: None, 
            max_duration: None, 
            max_work_duration: Some(max_work_duration), 
            tour_size: None 
        }),
        ..create_default_vehicle_type()
    }
}

#[test]
fn can_limit_one_job_by_max_work_duration() {
    let problem = Problem {
        plan: Plan { jobs: vec![create_delivery_job_with_duration("job1", (100., 0.), 15.)], ..create_empty_plan() },
        fleet: Fleet { vehicles: vec![create_vehicle_type_with_max_work_duration_limit(9.)], ..create_default_fleet() },
        ..create_empty_problem()
    };
    let matrix = Matrix {
        profile: Some("car".to_owned()),
        timestamp: None,
        travel_times: vec![1, 100, 100, 1],
        distances: vec![1, 1, 1, 1],
        error_codes: None,
    };

    let solution = solve_with_metaheuristic(problem, Some(vec![matrix]));

    assert_eq!(solution.unassigned.iter().len(), 1);
    assert_eq!(solution.unassigned.as_ref().unwrap()[0].reasons[0].code, "WORK_DURATION_CONSTRAINT");
}

#[test]
fn can_serve_job_when_work_duration_within_limit() {
    let problem = Problem {
        plan: Plan { jobs: vec![create_delivery_job("job1", (100., 0.))], ..create_empty_plan() },
        fleet: Fleet { vehicles: vec![create_vehicle_type_with_max_work_duration_limit(11.)], ..create_default_fleet() },
        ..create_empty_problem()
    };
    let matrix = Matrix {
        profile: Some("car".to_owned()),
        timestamp: None,
        travel_times: vec![1, 100, 100, 1],
        distances: vec![1, 1, 1, 1],
        error_codes: None,
    };

    let solution = solve_with_metaheuristic(problem, Some(vec![matrix]));

    assert!(solution.unassigned.is_none());
    assert!(!solution.tours.is_empty());
}

#[test]
fn can_skip_jobs_when_work_duration_exceeds_limit() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![
                create_delivery_job_with_duration("job1", (1., 0.), 5.),
                create_delivery_job_with_duration("job2", (2., 0.), 5.),
                create_delivery_job_with_duration("job3", (3., 0.), 5.),
            ],
            ..create_empty_plan()
        },
        fleet: Fleet { vehicles: vec![create_vehicle_type_with_max_work_duration_limit(15.)], ..create_default_fleet() },
        ..create_empty_problem()
    };
    let matrix = create_matrix_from_problem(&problem);

    let solution = solve_with_metaheuristic(problem, Some(vec![matrix]));

    // Should be able to serve 2 jobs (total work duration ~11), but not all 3 (~17 > 15)
    assert!(solution.tours.len() == 1);
    assert!(solution.tours[0].stops.len() <= 4); // start + 2 jobs + end = 4 stops max
    assert!(solution.unassigned.is_some());
}

#[test]
fn work_duration_limit_does_not_include_depot_travel() {
    // Test that work duration only includes time from first job arrival to last job departure
    // Travel from depot to first job and from last job to depot should be excluded
    let problem = Problem {
        plan: Plan { 
            jobs: vec![create_delivery_job_with_duration("job1", (10., 0.), 10.)],
            ..create_empty_plan() 
        },
        fleet: Fleet { 
            vehicles: vec![create_vehicle_type_with_max_work_duration_limit(10.)], 
            ..create_default_fleet() 
        },
        ..create_empty_problem()
    };
    let matrix = Matrix {
        profile: Some("car".to_owned()),
        timestamp: None,
        // High travel time from depot (0,0) to job (10,0) and back, but work duration should only be job duration
        travel_times: vec![1, 50, 50, 1],
        distances: vec![1, 1, 1, 1],
        error_codes: None,
    };

    let solution = solve_with_metaheuristic(problem, Some(vec![matrix]));

    // Should succeed because work duration (10) equals limit (10), even though total duration is much higher
    assert!(solution.unassigned.is_none());
    assert!(!solution.tours.is_empty());
}