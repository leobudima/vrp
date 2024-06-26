use crate::construction::features::skills::create_skills_feature;
use crate::construction::features::{JobSkills, JobSkillsAspects};
use crate::construction::heuristics::MoveContext;
use crate::helpers::construction::heuristics::InsertionContextBuilder;
use crate::helpers::models::problem::{test_driver, FleetBuilder, SingleBuilder, VehicleBuilder};
use crate::helpers::models::solution::{RouteBuilder, RouteContextBuilder};
use crate::models::common::ValueDimension;
use crate::models::problem::{Job, Vehicle};
use crate::models::{ConstraintViolation, ViolationCode};
use hashbrown::HashSet;
use std::iter::FromIterator;

const VIOLATION_CODE: ViolationCode = 1;

#[derive(Clone)]
struct TestJobSkillsAspects;

impl JobSkillsAspects for TestJobSkillsAspects {
    fn get_job_skills<'a>(&self, job: &'a Job) -> Option<&'a JobSkills> {
        job.dimens().get_value("skills")
    }

    fn get_vehicle_skills<'a>(&self, vehicle: &'a Vehicle) -> Option<&'a HashSet<String>> {
        vehicle.dimens.get_value("skills")
    }

    fn get_violation_code(&self) -> ViolationCode {
        VIOLATION_CODE
    }
}

fn create_job_with_skills(all_of: Option<Vec<&str>>, one_of: Option<Vec<&str>>, none_of: Option<Vec<&str>>) -> Job {
    SingleBuilder::default()
        .property(
            "skills",
            JobSkills {
                all_of: all_of.map(|skills| skills.iter().map(|s| s.to_string()).collect()),
                one_of: one_of.map(|skills| skills.iter().map(|s| s.to_string()).collect()),
                none_of: none_of.map(|skills| skills.iter().map(|s| s.to_string()).collect()),
            },
        )
        .build_as_job_ref()
}

fn create_vehicle_with_skills(skills: Option<Vec<&str>>) -> Vehicle {
    let mut builder = VehicleBuilder::default();
    if let Some(skills) = skills {
        builder.property("skills", HashSet::<String>::from_iter(skills.iter().map(|s| s.to_string())));
    }

    builder.id("v1").build()
}

fn failure() -> Option<ConstraintViolation> {
    ConstraintViolation::fail(VIOLATION_CODE)
}

parameterized_test! {can_check_skills, (all_of, one_of, none_of, vehicle_skills, expected), {
    can_check_skills_impl(all_of, one_of, none_of, vehicle_skills, expected);
}}

can_check_skills! {
    case01: (None, None, None, None, None),

    case_all_of_01: (Some(vec!["s1"]), None, None, None, failure()),
    case_all_of_02: (Some(vec![]), None, None, None, None),
    case_all_of_03: (Some(vec!["s1"]), None, None, Some(vec!["s1"]), None),
    case_all_of_04: (Some(vec!["s1"]), None, None, Some(vec!["s2"]), failure()),
    case_all_of_05: (Some(vec!["s1", "s2"]), None, None, Some(vec!["s2"]), failure()),
    case_all_of_06: (Some(vec!["s1"]), None, None, Some(vec!["s1", "s2"]), None),

    case_one_of_01: (None, Some(vec!["s1"]), None, None, failure()),
    case_one_of_02: (None, Some(vec![]), None, None, None),
    case_one_of_03: (None, Some(vec!["s1"]), None, Some(vec!["s1"]), None),
    case_one_of_04: (None, Some(vec!["s1"]), None, Some(vec!["s2"]), failure()),
    case_one_of_05: (None, Some(vec!["s1", "s2"]), None, Some(vec!["s2"]), None),
    case_one_of_06: (None, Some(vec!["s1"]), None, Some(vec!["s1", "s2"]), None),

    case_none_of_01: (None, None, Some(vec!["s1"]), None, None),
    case_none_of_02: (None, None, Some(vec![]), None, None),
    case_none_of_03: (None, None, Some(vec!["s1"]), Some(vec!["s1"]), failure()),
    case_none_of_04: (None, None, Some(vec!["s1"]), Some(vec!["s2"]), None),
    case_none_of_05: (None, None, Some(vec!["s1", "s2"]), Some(vec!["s2"]), failure()),
    case_none_of_06: (None, None, Some(vec!["s1"]), Some(vec!["s1", "s2"]), failure()),

    case_combine_01: (Some(vec!["s1"]), None, Some(vec!["s2"]), Some(vec!["s1", "s2"]), failure()),
    case_combine_02: (None, Some(vec!["s1"]), Some(vec!["s2"]), Some(vec!["s1", "s2"]), failure()),
    case_combine_03: (Some(vec!["s1"]), Some(vec!["s2"]), None, Some(vec!["s1", "s2"]), None),
    case_combine_04: (Some(vec!["s1"]), Some(vec!["s2", "s3"]), None, Some(vec!["s1", "s2"]), None),
    case_combine_05: (Some(vec!["s1", "s2"]), Some(vec!["s3"]), None, Some(vec!["s1", "s2", "s3"]), None),
    case_combine_06: (Some(vec!["s1", "s2"]), Some(vec!["s3"]), None, Some(vec!["s1", "s2"]), failure()),
    case_combine_07: (Some(vec!["s1"]), Some(vec!["s2"]), Some(vec!["s3"]), Some(vec!["s1", "s2", "s3"]), failure()),
}

fn can_check_skills_impl(
    all_of: Option<Vec<&str>>,
    one_of: Option<Vec<&str>>,
    none_of: Option<Vec<&str>>,
    vehicle_skills: Option<Vec<&str>>,
    expected: Option<ConstraintViolation>,
) {
    let fleet = FleetBuilder::default()
        .add_driver(test_driver())
        .add_vehicle(create_vehicle_with_skills(vehicle_skills))
        .build();
    let route_ctx =
        RouteContextBuilder::default().with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build()).build();

    let constraint = create_skills_feature("skills", TestJobSkillsAspects).unwrap().constraint.unwrap();

    let actual = constraint.evaluate(&MoveContext::route(
        &InsertionContextBuilder::default().build().solution,
        &route_ctx,
        &create_job_with_skills(all_of, one_of, none_of),
    ));

    assert_eq!(actual, expected)
}

parameterized_test! {can_merge_skills, (source, candidate, expected), {
    can_merge_skills_impl(source, candidate, expected);
}}

can_merge_skills! {
    case_01: (create_job_with_skills(None, None, None), create_job_with_skills(None, None, None), Ok(())),

    case_02: (create_job_with_skills(Some(vec!["skill"]), None, None), create_job_with_skills(None, None, None), Ok(())),
    case_03: (create_job_with_skills(None, Some(vec!["skill"]), None), create_job_with_skills(None, None, None), Ok(())),
    case_04: (create_job_with_skills(None, None, Some(vec!["skill"])), create_job_with_skills(None, None, None), Ok(())),

    case_05: (create_job_with_skills(None, None, None), create_job_with_skills(Some(vec!["skill"]), None, None), Err(1)),
    case_06: (create_job_with_skills(None, None, None), create_job_with_skills(None, Some(vec!["skill"]), None), Err(1)),
    case_07: (create_job_with_skills(None, None, None), create_job_with_skills(None, None, Some(vec!["skill"])), Err(1)),

    case_08: (create_job_with_skills(Some(vec!["skill"]), None, None), create_job_with_skills(Some(vec!["skill"]), None, None), Ok(())),
    case_09: (create_job_with_skills(Some(vec!["skill"]), None, None), create_job_with_skills(None, Some(vec!["skill"]), None), Err(1)),
    case_10: (create_job_with_skills(Some(vec!["skill1", "skill2"]), None, None), create_job_with_skills(Some(vec!["skill1"]), None, None), Ok(())),
    case_11: (create_job_with_skills(Some(vec!["skill1"]), None, None), create_job_with_skills(Some(vec!["skill1", "skill2"]), None, None), Err(1)),
}

fn can_merge_skills_impl(source: Job, candidate: Job, expected: Result<(), i32>) {
    let constraint = create_skills_feature("skills", TestJobSkillsAspects).unwrap().constraint.unwrap();

    let result = constraint.merge(source, candidate).map(|_| ());

    assert_eq!(result, expected);
}

#[test]
fn can_create_empty_skills_as_none() {
    let skills = JobSkills::new(Some(vec![]), Some(vec![]), Some(vec![]));

    assert!(skills.all_of.is_none());
    assert!(skills.one_of.is_none());
    assert!(skills.none_of.is_none());
}
