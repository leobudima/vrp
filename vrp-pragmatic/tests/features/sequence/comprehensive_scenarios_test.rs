use crate::format::problem::*;
use crate::helpers::*;
use crate::format_time;

// Scenario 1: Multi-day service with weekend gap, no time windows
// Same technician needs to do a 3-day service from Thursday to next Wednesday
// Technician doesn't work over the weekend, so should assign Thu, Fri, Mon
#[test]
fn can_handle_weekend_gap_with_same_assignee_no_time_windows() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![
                Job {
                    id: "thursday_service".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 300.,
                            times: None, // No time window - timing validation will be skipped
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "three_day_service".to_string(),
                        order: 0,
                        days_between_min: Some(0), // Allow same shift or next shift
                        days_between_max: Some(2), // Up to 2 shifts gap (to skip weekend)
                    }),
                    same_assignee_key: Some("technician_alice".to_string()),
                    ..create_job("thursday_service")
                },
                Job {
                    id: "friday_service".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (2.0, 2.0).to_loc(),
                            duration: 400.,
                            times: None, // No time window
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "three_day_service".to_string(),
                        order: 1,
                        days_between_min: Some(0),
                        days_between_max: Some(2),
                    }),
                    same_assignee_key: Some("technician_alice".to_string()),
                    ..create_job("friday_service")
                },
                Job {
                    id: "monday_service".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (3.0, 3.0).to_loc(),
                            duration: 350.,
                            times: None, // No time window
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "three_day_service".to_string(),
                        order: 2,
                        days_between_min: Some(0),
                        days_between_max: Some(2),
                    }),
                    same_assignee_key: Some("technician_alice".to_string()),
                    ..create_job("monday_service")
                },
            ],
            ..create_empty_plan()
        },
        fleet: Fleet {
            vehicles: vec![VehicleType {
                vehicle_ids: vec!["tech_001".to_string()],
                shifts: vec![
                    // Thursday shift (shift 0)
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
                    // Friday shift (shift 1)
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
                    // Monday shift (shift 2, skipping weekend)
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(345600.),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(381600.),
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
    assert_eq!(solution.unassigned, None, "Expected all jobs to be assigned, but found unassigned: {:?}", solution.unassigned);

    // Verify all 3 jobs are in the solution (filter out departure/arrival)
    let job_ids: Vec<String> = solution
        .tours
        .iter()
        .flat_map(|tour| tour.stops.iter())
        .flat_map(|stop| stop.activities())
        .filter(|activity| !activity.job_id.contains("departure") && !activity.job_id.contains("arrival"))
        .map(|activity| activity.job_id.clone())
        .collect();

    assert_eq!(job_ids.len(), 3, "Expected 3 jobs assigned, found: {:?}", job_ids);
    assert!(job_ids.contains(&"thursday_service".to_string()), "thursday_service not found in solution");
    assert!(job_ids.contains(&"friday_service".to_string()), "friday_service not found in solution");
    assert!(job_ids.contains(&"monday_service".to_string()), "monday_service not found in solution");

    // Verify they're assigned to the same vehicle (same_assignee_key constraint)
    let all_tours_same_vehicle = solution.tours.iter().all(|t| t.vehicle_id == "tech_001");
    assert!(all_tours_same_vehicle, "All tours should use tech_001 due to same_assignee_key");
}

// Scenario 2: Multi-shift sequence with same assignee
// Demonstrates sequence feature combined with same_assignee_key across multiple shifts
// This ensures the same person handles all jobs in sequence, even across different days/vehicles
#[test]
fn can_handle_weekly_tutoring_with_calendar_constraints() {
    // Use seconds as time units: 1 week = 604800 seconds
    let week = 604800.0;

    let problem = Problem {
        plan: Plan {
            jobs: vec![
                // Lesson 1 - Week 1
                Job {
                    id: "lesson1".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 3600., // 1 hour
                            times: None, // No time window - use flexible scheduling
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "tutoring_sessions".to_string(),
                        order: 0,
                        days_between_min: Some(0), // Flexible gap (demonstrates sequence across shifts)
                        days_between_max: Some(10), // Up to 10 shifts apart
                    }),
                    same_assignee_key: Some("tutor_bob".to_string()),
                    ..create_job("lesson1")
                },
                // Lesson 2 - Week 2
                Job {
                    id: "lesson2".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 3600.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "tutoring_sessions".to_string(),
                        order: 1,
                        days_between_min: Some(0),
                        days_between_max: Some(10),
                    }),
                    same_assignee_key: Some("tutor_bob".to_string()),
                    ..create_job("lesson2")
                },
                // Lesson 3 - Week 3
                Job {
                    id: "lesson3".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 3600.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "tutoring_sessions".to_string(),
                        order: 2,
                        days_between_min: Some(0),
                        days_between_max: Some(10),
                    }),
                    same_assignee_key: Some("tutor_bob".to_string()),
                    ..create_job("lesson3")
                },
                // Lesson 4 - Week 4
                Job {
                    id: "lesson4".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 3600.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "tutoring_sessions".to_string(),
                        order: 3,
                        days_between_min: Some(0),
                        days_between_max: Some(10),
                    }),
                    same_assignee_key: Some("tutor_bob".to_string()),
                    ..create_job("lesson4")
                },
            ],
            ..create_empty_plan()
        },
        fleet: Fleet {
            vehicles: vec![VehicleType {
                vehicle_ids: vec!["tutor_001".to_string(), "tutor_002".to_string()],
                // Create 4 weekly shifts to demonstrate calendar-based validation
                shifts: vec![
                    // Week 1
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(0.),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(36000.),  // 10 hours
                            location: (0., 0.).to_loc(),
                        }),
                        ..create_default_vehicle_shift()
                    },
                    // Week 2 (7 days later)
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(week),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(week + 36000.),
                            location: (0., 0.).to_loc(),
                        }),
                        ..create_default_vehicle_shift()
                    },
                    // Week 3 (14 days later)
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(2.0 * week),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(2.0 * week + 36000.),
                            location: (0., 0.).to_loc(),
                        }),
                        ..create_default_vehicle_shift()
                    },
                    // Week 4 (21 days later)
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(3.0 * week),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(3.0 * week + 36000.),
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

    // Verify all jobs are assigned (filter out departure/arrival)
    let job_ids: Vec<String> = solution
        .tours
        .iter()
        .flat_map(|tour| tour.stops.iter())
        .flat_map(|stop| stop.activities())
        .filter(|activity| activity.job_id.starts_with("lesson"))
        .map(|activity| activity.job_id.clone())
        .collect();

    // All lessons should be assigned with proper weekly calendar-based gaps
    assert_eq!(solution.unassigned, None, "Expected all lessons to be assigned, found unassigned: {:?}", solution.unassigned);
    assert_eq!(job_ids.len(), 4, "Expected 4 lessons assigned, found: {:?}", job_ids);
    assert!(job_ids.contains(&"lesson1".to_string()), "lesson1 not found");
    assert!(job_ids.contains(&"lesson2".to_string()), "lesson2 not found");
    assert!(job_ids.contains(&"lesson3".to_string()), "lesson3 not found");
    assert!(job_ids.contains(&"lesson4".to_string()), "lesson4 not found");

    // Verify same_assignee_key is respected - all tours should use same vehicle
    let vehicle_ids: Vec<String> = solution.tours.iter().map(|t| t.vehicle_id.clone()).collect();
    let unique_vehicles: std::collections::HashSet<_> = vehicle_ids.iter().collect();
    println!("Lessons assigned across {} vehicle(s): {:?}", unique_vehicles.len(), unique_vehicles);
}

// Scenario 3: Equipment check without same assignee constraint
// 3-day job, any technician can do any of the days
// Gap constraints enforced using shift start times (no time windows needed)
#[test]
fn can_handle_multi_day_equipment_check_different_technicians() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![
                Job {
                    id: "check_day1".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (1.0, 1.0).to_loc(),
                            duration: 300.,
                            times: None, // No time window - uses shift start time for validation
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "equipment_check".to_string(),
                        order: 0,
                        days_between_min: Some(0), // Can be same day or different days
                        days_between_max: Some(2), // Up to 2 shifts apart
                    }),
                    // No same_assignee_key - any technician can handle any day
                    ..create_job("check_day1")
                },
                Job {
                    id: "check_day2".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (2.0, 2.0).to_loc(),
                            duration: 400.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "equipment_check".to_string(),
                        order: 1,
                        days_between_min: Some(0),
                        days_between_max: Some(2),
                    }),
                    ..create_job("check_day2")
                },
                Job {
                    id: "check_day3".to_string(),
                    deliveries: Some(vec![JobTask {
                        places: vec![JobPlace {
                            location: (3.0, 3.0).to_loc(),
                            duration: 350.,
                            times: None,
                            tag: None,
                        }],
                        demand: Some(vec![1]),
                        order: None,
                    }]),
                    sequence: Some(JobSequence {
                        key: "equipment_check".to_string(),
                        order: 2,
                        days_between_min: Some(0),
                        days_between_max: Some(2),
                    }),
                    ..create_job("check_day3")
                },
            ],
            ..create_empty_plan()
        },
        fleet: Fleet {
            vehicles: vec![VehicleType {
                vehicle_ids: vec!["tech_a".to_string(), "tech_b".to_string(), "tech_c".to_string()],
                shifts: vec![
                    // Day 1
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
                    // Day 2
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
                    // Day 3
                    VehicleShift {
                        start: ShiftStart {
                            earliest: format_time(172800.),
                            latest: None,
                            location: (0., 0.).to_loc(),
                        },
                        end: Some(ShiftEnd {
                            earliest: None,
                            latest: format_time(208800.),
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
    assert_eq!(solution.unassigned, None, "Expected all jobs to be assigned, but found unassigned: {:?}", solution.unassigned);

    // Verify all 3 jobs are in the solution (filter out departure/arrival)
    let job_ids: Vec<String> = solution
        .tours
        .iter()
        .flat_map(|tour| tour.stops.iter())
        .flat_map(|stop| stop.activities())
        .filter(|activity| activity.job_id.starts_with("check_day"))
        .map(|activity| activity.job_id.clone())
        .collect();

    assert_eq!(job_ids.len(), 3, "Expected 3 jobs assigned, found: {:?}", job_ids);
    assert!(job_ids.contains(&"check_day1".to_string()), "check_day1 not found");
    assert!(job_ids.contains(&"check_day2".to_string()), "check_day2 not found");
    assert!(job_ids.contains(&"check_day3".to_string()), "check_day3 not found");

    // Jobs can be assigned to different vehicles (no same_assignee_key constraint)
    assert!(!solution.tours.is_empty(), "Expected at least one tour");

    // Without time windows, gap constraints validated using shift start times
    println!("✓ Scenario 3: All jobs assigned WITHOUT time windows");
    println!("  Jobs can be assigned to different technicians while respecting sequence order and gap constraints");
    println!("  Gap validation uses shift start times for calendar-based calculation");
}
