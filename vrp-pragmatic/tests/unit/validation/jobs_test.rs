use super::*;
use crate::helpers::*;

fn assert_result(code: &str, action: &str, result: Option<FormatError>) {
    assert_eq!(result.clone().map(|err| err.code), Some(code.to_string()));
    assert!(result.map_or("".to_string(), |err| err.action).contains(action));
}

parameterized_test! {can_detect_reserved_ids, (job_id, expected), {
    can_detect_reserved_ids_impl(job_id.to_string(), expected);
}}

can_detect_reserved_ids! {
    case01: ("job1", None),
    case02: ("departure", Some("departure")),
    case03: ("arrival", Some("arrival")),
    case04: ("break", Some("break")),
    case05: ("reload", Some("reload")),
}

fn can_detect_reserved_ids_impl(job_id: String, expected: Option<&str>) {
    let problem = Problem {
        plan: Plan { jobs: vec![create_delivery_job(job_id.as_str(), (1., 0.))], ..create_empty_plan() },
        fleet: create_default_fleet(),
        ..create_empty_problem()
    };

    let result = check_e1104_no_reserved_ids(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();

    if let Some(action) = expected {
        assert_result("E1104", action, result);
    } else {
        assert!(result.is_none());
    }
}

#[test]
fn can_detect_empty_job() {
    let problem = Problem {
        plan: Plan { jobs: vec![Job { deliveries: Some(vec![]), ..create_job("job1") }], ..create_empty_plan() },
        ..create_empty_problem()
    };

    let result = check_e1105_empty_jobs(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();

    assert_result("E1105", "job1", result);
}

#[test]
fn can_detect_negative_duration() {
    let problem = Problem {
        plan: Plan { jobs: vec![create_delivery_job_with_duration("job1", (1., 0.), -10.)], ..create_empty_plan() },
        ..create_empty_problem()
    };

    let result =
        check_e1106_negative_duration(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();

    assert_result("E1106", "job1", result);
}

#[test]
fn can_detect_negative_demand() {
    let problem = Problem {
        plan: Plan {
            jobs: vec![create_delivery_job_with_demand("job1", (1., 0.), vec![0, -1])],
            ..create_empty_plan()
        },
        ..create_empty_problem()
    };

    let result = check_e1107_negative_demand(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();

    assert_result("E1107", "job1", result);
}

// --- Sync groups validation tests (E1110) ---

#[test]
fn sync_validation_fails_on_wrong_cardinality() {
    // vehicles_required = 2, only one clone present
    let mut job1 = create_delivery_job("job1", (1., 0.));
    job1.sync = Some(JobSync { key: "g1".into(), index: 0, vehicles_required: 2, tolerance: None });

    let problem = Problem { plan: Plan { jobs: vec![job1], ..create_empty_plan() }, ..create_empty_problem() };
    let res = super::check_sync_groups_consistency(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();
    assert!(res.is_some());
    assert_eq!(res.unwrap().code, "E1110");
}

#[test]
fn sync_validation_fails_on_index_gaps_or_duplicates() {
    // required=3, indices {0,2}
    let mut job1 = create_delivery_job("job1", (1., 0.));
    job1.sync = Some(JobSync { key: "g1".into(), index: 0, vehicles_required: 3, tolerance: None });
    let mut job2 = create_delivery_job("job2", (1., 0.));
    job2.sync = Some(JobSync { key: "g1".into(), index: 2, vehicles_required: 3, tolerance: None });

    let problem = Problem { plan: Plan { jobs: vec![job1, job2], ..create_empty_plan() }, ..create_empty_problem() };
    let res = super::check_sync_groups_consistency(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();
    assert!(res.is_some());
    assert_eq!(res.unwrap().code, "E1110");
}

#[test]
fn sync_validation_fails_on_inconsistent_tasks() {
    // Two clones, different durations
    let mut job1 = create_delivery_job_with_duration("job1", (1., 0.), 10.);
    job1.sync = Some(JobSync { key: "g1".into(), index: 0, vehicles_required: 2, tolerance: None });
    let mut job2 = create_delivery_job_with_duration("job2", (1., 0.), 20.);
    job2.sync = Some(JobSync { key: "g1".into(), index: 1, vehicles_required: 2, tolerance: None });

    let problem = Problem { plan: Plan { jobs: vec![job1, job2], ..create_empty_plan() }, ..create_empty_problem() };
    let res = super::check_sync_groups_consistency(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();
    assert!(res.is_some());
    assert_eq!(res.unwrap().code, "E1110");
}

#[test]
fn sync_validation_fails_on_inconsistent_group_or_affinity_or_compatibility() {
    // Two clones, different group
    let mut job1 = create_delivery_job("job1", (1., 0.));
    job1.group = Some("A".into());
    job1.sync = Some(JobSync { key: "g1".into(), index: 0, vehicles_required: 2, tolerance: None });

    let mut job2 = create_delivery_job("job2", (1., 0.));
    job2.group = Some("B".into());
    job2.sync = Some(JobSync { key: "g1".into(), index: 1, vehicles_required: 2, tolerance: None });

    let problem = Problem { plan: Plan { jobs: vec![job1, job2], ..create_empty_plan() }, ..create_empty_problem() };
    let res = super::check_sync_groups_consistency(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();
    assert!(res.is_some());
    assert_eq!(res.unwrap().code, "E1110");
}

#[test]
fn sync_validation_allows_different_skills() {
    // Same tasks/attrs, different skills → OK
    let mut job1 = create_delivery_job("job1", (1., 0.));
    job1.sync = Some(JobSync { key: "g1".into(), index: 0, vehicles_required: 2, tolerance: None });
    job1.skills = Some(JobSkills { all_of: Some(vec!["A".into()]), one_of: None, none_of: None });

    let mut job2 = create_delivery_job("job2", (1., 0.));
    job2.sync = Some(JobSync { key: "g1".into(), index: 1, vehicles_required: 2, tolerance: None });
    job2.skills = Some(JobSkills { all_of: Some(vec!["B".into()]), one_of: None, none_of: None });

    let problem = Problem { plan: Plan { jobs: vec![job1, job2], ..create_empty_plan() }, ..create_empty_problem() };
    let res = super::check_sync_groups_consistency(&ValidationContext::new(&problem, None, &CoordIndex::new(&problem))).err();
    assert!(res.is_none());
}
