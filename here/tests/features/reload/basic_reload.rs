use crate::helpers::*;
use crate::json::problem::*;
use crate::json::solution::*;

parameterized_test! {can_use_vehicle_with_two_tours_and_two_jobs, (jobs, unassigned), {
    can_use_vehicle_with_two_tours_and_two_jobs_impl(jobs, unassigned);
}}

can_use_vehicle_with_two_tours_and_two_jobs! {
    case01: (vec![
                create_delivery_job("job1", vec![1., 0.]),
                create_delivery_job("job2", vec![2., 0.])],
            vec![]),
    case02: (vec![
               create_delivery_job("job1", vec![1., 0.]),
               create_delivery_job("job2", vec![2., 0.]),
               create_delivery_job("job3", vec![3., 0.])
             ],
             vec![
               UnassignedJob {
                    job_id: "job3".to_string(),
                    reasons: vec![UnassignedJobReason {
                        code: 3,
                        description: "does not fit into any vehicle due to capacity".to_string()
                    }]
                }
             ]),
}

fn can_use_vehicle_with_two_tours_and_two_jobs_impl(jobs: Vec<JobVariant>, unassigned: Vec<UnassignedJob>) {
    let problem = Problem {
        id: "my_problem".to_string(),
        plan: Plan { jobs, relations: Option::None },
        fleet: Fleet {
            types: vec![VehicleType {
                id: "my_vehicle".to_string(),
                profile: "car".to_string(),
                costs: create_default_vehicle_costs(),
                shifts: vec![VehicleShift {
                    start: VehiclePlace { time: format_time(0), location: vec![0., 0.].to_loc() },
                    end: Some(VehiclePlace { time: format_time(100).to_string(), location: vec![0., 0.].to_loc() }),
                    breaks: None,
                    reloads: Some(vec![VehicleReload {
                        times: None,
                        location: vec![0., 0.].to_loc(),
                        duration: 2.0,
                        tag: None,
                    }]),
                }],
                capacity: vec![1],
                amount: 1,
                skills: None,
                limits: None,
            }],
            profiles: create_default_profiles(),
        },
        config: None,
    };
    let matrix = create_matrix_from_problem(&problem);

    let solution = solve_with_metaheuristic(problem, vec![matrix]);

    assert_eq!(
        solution,
        Solution {
            problem_id: "my_problem".to_string(),
            statistic: Statistic {
                cost: 26.,
                distance: 6,
                duration: 10,
                times: Timing { driving: 6, serving: 4, waiting: 0, break_time: 0 },
            },
            tours: vec![Tour {
                vehicle_id: "my_vehicle_1".to_string(),
                type_id: "my_vehicle".to_string(),
                stops: vec![
                    create_stop_with_activity(
                        "departure",
                        "departure",
                        (0., 0.),
                        1,
                        ("1970-01-01T00:00:00Z", "1970-01-01T00:00:00Z"),
                    ),
                    create_stop_with_activity(
                        "job1",
                        "delivery",
                        (1., 0.),
                        0,
                        ("1970-01-01T00:00:01Z", "1970-01-01T00:00:02Z"),
                    ),
                    create_stop_with_activity(
                        "reload",
                        "reload",
                        (0., 0.),
                        1,
                        ("1970-01-01T00:00:03Z", "1970-01-01T00:00:05Z"),
                    ),
                    create_stop_with_activity(
                        "job2",
                        "delivery",
                        (2., 0.),
                        0,
                        ("1970-01-01T00:00:07Z", "1970-01-01T00:00:08Z"),
                    ),
                    create_stop_with_activity(
                        "arrival",
                        "arrival",
                        (0., 0.),
                        0,
                        ("1970-01-01T00:00:10Z", "1970-01-01T00:00:10Z"),
                    ),
                ],
                statistic: Statistic {
                    cost: 26.,
                    distance: 6,
                    duration: 10,
                    times: Timing { driving: 6, serving: 4, waiting: 0, break_time: 0 },
                },
            }],
            unassigned,
            extras: Extras { performance: vec![] },
        }
    );
}
