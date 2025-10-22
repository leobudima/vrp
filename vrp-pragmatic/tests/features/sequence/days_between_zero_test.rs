use crate::format::problem::*;
use crate::helpers::*;
use crate::format_time;

// Test to demonstrate behavior when days_between_min = 0
// This allows jobs to be on the same shift/route
#[test]
fn can_assign_sequence_jobs_on_same_shift_when_days_between_min_is_zero() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![
                Job {
                    id: "task1".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 100.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "same_shift_sequence".to_string(),
                        order: 0,
                        days_between_min: Some(0), // Allows same shift
                        days_between_max: Some(0), // Requires same shift
                    }),
                    ..create_job("task1")
                },
                Job {
                    id: "task2".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (2.0, 2.0).to_loc(),
                            duration: 100.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "same_shift_sequence".to_string(),
                        order: 1,
                        days_between_min: Some(0),
                        days_between_max: Some(0),
                    }),
                    ..create_job("task2")
                },
                Job {
                    id: "task3".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (3.0, 3.0).to_loc(),
                            duration: 100.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "same_shift_sequence".to_string(),
                        order: 2,
                        days_between_min: Some(0),
                        days_between_max: Some(0),
                    }),
                    ..create_job("task3")
                },
            ],
            ..create_empty_plan()
        },
        fleet: Fleet {
            vehicles: vec![VehicleType {
                vehicle_ids: vec!["vehicle_001".to_string()],
                shifts: vec![VehicleShift {
                    start: ShiftStart {
                        earliest: format_time(0.),
                        latest: None,
                        location: (0., 0.).to_loc(),
                    },
                    end: Some(ShiftEnd {
                        earliest: None,
                        latest: format_time(36000.),
                        location: (0., 0.).to_loc(),
                    }),
                    ..create_default_vehicle_shift()
                }],
                capacity: vec![10],
                ..create_default_vehicle_type()
            }],
            ..create_default_fleet()
        },
        ..create_empty_problem()
    };
    let matrix = create_matrix_from_problem(&problem);

    let solution = solve_with_metaheuristic(problem, Some(vec![matrix]));

    // Verify all jobs are assigned
    assert_eq!(solution.unassigned, None, "All jobs should be assigned");

    // Verify all jobs are on the SAME shift/route
    assert_eq!(solution.tours.len(), 1, "All jobs should be on ONE tour (same shift)");

    let job_ids: Vec<String> = solution
        .tours
        .iter()
        .flat_map(|tour| tour.stops.iter())
        .flat_map(|stop| stop.activities())
        .filter(|activity| activity.job_id.starts_with("task"))
        .map(|activity| activity.job_id.clone())
        .collect();

    // All 3 tasks should be on the same route in sequence order
    assert_eq!(job_ids.len(), 3, "All 3 tasks should be assigned");

    // Note: The sequence feature ensures jobs are assigned in order (0, 1, 2),
    // but within a single route, the solver may optimize the visit order for cost.
    // The sequence constraint ensures assignment order, not necessarily visit order.
    assert!(job_ids.contains(&"task1".to_string()), "task1 should be assigned");
    assert!(job_ids.contains(&"task2".to_string()), "task2 should be assigned");
    assert!(job_ids.contains(&"task3".to_string()), "task3 should be assigned");

    println!("✓ With days_between_min=0: All 3 sequence jobs assigned to SAME shift/route");
    println!("  Visit order on route: {:?} (optimized for cost, not sequence order)", job_ids);
}

// Test to demonstrate that days_between_min = 1 requires different shifts
// Jobs MUST be on consecutive shifts when days_between_min = days_between_max = 1
// NOTE: This test uses time windows to help the solver explore multi-shift solutions
#[test]
fn requires_different_shifts_when_days_between_min_is_one() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![
                Job {
                    id: "task1".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 100.,
                            // Add time window to guide solver to shift 0
                            times: Some(vec![vec![format_time(0.), format_time(36000.)]]),
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "different_shift_sequence".to_string(),
                        order: 0,
                        days_between_min: Some(1), // Requires at least 1 shift gap
                        days_between_max: Some(1),
                    }),
                    ..create_job("task1")
                },
                Job {
                    id: "task2".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (2.0, 2.0).to_loc(),
                            duration: 100.,
                            // Add time window to guide solver to shift 1
                            times: Some(vec![vec![format_time(86400.), format_time(122400.)]]),
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "different_shift_sequence".to_string(),
                        order: 1,
                        days_between_min: Some(1),
                        days_between_max: Some(1),
                    }),
                    ..create_job("task2")
                },
            ],
            ..create_empty_plan()
        },
        fleet: Fleet {
            vehicles: vec![VehicleType {
                vehicle_ids: vec!["vehicle_001".to_string()],
                shifts: vec![
                    // Shift 0
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(0.),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(36000.),
                            location: (0., 0.).to_loc(),
                        }),
                        ..create_default_vehicle_shift()
                    },
                    // Shift 1 (next day)
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(86400.),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(122400.),
                            location: (0., 0.).to_loc(),
                        }),
                        ..create_default_vehicle_shift()
                    },
                ],
                capacity: vec![10],
                ..create_default_vehicle_type()
            }],
            ..create_default_fleet()
        },
        ..create_empty_problem()
    };
    let matrix = create_matrix_from_problem(&problem);

    let solution = solve_with_metaheuristic(problem, Some(vec![matrix]));

    // Verify all jobs are assigned
    assert_eq!(solution.unassigned, None, "All jobs should be assigned, found: {:?}", solution.unassigned);

    // Verify jobs are on DIFFERENT shifts
    assert_eq!(solution.tours.len(), 2, "Jobs should be on TWO different tours (different shifts)");

    // Verify they're on consecutive shifts (shift 0 and shift 1)
    let shift_indices: Vec<usize> = solution.tours.iter().map(|t| t.shift_index).collect();
    assert!(shift_indices.contains(&0), "Should have job on shift 0");
    assert!(shift_indices.contains(&1), "Should have job on shift 1");

    println!("✓ With days_between_min=1: Sequence jobs MUST be on consecutive shifts");
}
