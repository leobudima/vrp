use super::*;
use crate::construction::enablers::create_typed_actor_groups;
use crate::helpers::models::domain::{TestGoalContextBuilder, test_random};
use crate::helpers::models::problem::{FleetBuilder, TestSingleBuilder, test_driver, test_vehicle_with_id};
use crate::helpers::models::solution::{ActivityBuilder, RouteBuilder, RouteContextBuilder};
use crate::models::problem::{Fleet, Single};
use crate::models::solution::Registry;
use crate::construction::heuristics::RegistryContext;
use std::sync::Arc;

const VIOLATION_CODE: ViolationCode = ViolationCode(1);

fn create_test_affinity_feature() -> Feature {
    create_vehicle_affinity_feature("affinity", VIOLATION_CODE).unwrap()
}

fn create_test_fleet() -> Fleet {
    FleetBuilder::default()
        .add_driver(test_driver())
        .add_vehicle(test_vehicle_with_id("v1"))
        .add_vehicle(test_vehicle_with_id("v2"))
        .with_group_key_fn(Box::new(|actors| {
            Box::new(create_typed_actor_groups(actors, |a| a.vehicle.dimens.get_vehicle_id().cloned().unwrap()))
        }))
        .build()
}

fn create_test_single(affinity: Option<&str>, sequence: Option<u32>) -> Arc<Single> {
    let mut builder = TestSingleBuilder::default();

    if let Some(affinity) = affinity {
        builder.dimens_mut().set_job_affinity(affinity.to_string());
        if let Some(seq) = sequence {
            builder.dimens_mut().set_job_affinity_sequence(seq);
            builder.dimens_mut().set_job_affinity_duration_days(2); // Default duration
        }
    }

    builder.build_shared()
}

fn create_test_solution_context(
    fleet: &Fleet,
    routes: Vec<(&str, Vec<Option<&str>>)>,
) -> SolutionContext {
    SolutionContext {
        required: vec![],
        ignored: vec![],
        unassigned: Default::default(),
        locked: Default::default(),
        routes: routes
            .into_iter()
            .map(|(vehicle_id, affinities)| {
                RouteContextBuilder::default()
                    .with_route(
                        RouteBuilder::default()
                            .with_vehicle(fleet, vehicle_id)
                            .add_activities(affinities.into_iter().map(|affinity| {
                                ActivityBuilder::with_location(1).job(
                                    affinity.map(|a| create_test_single(Some(a), None))
                                ).build()
                            }))
                            .build(),
                    )
                    .build()
            })
            .collect(),
        registry: RegistryContext::new(&TestGoalContextBuilder::default().build(), Registry::new(fleet, test_random())),
        state: Default::default(),
    }
}


#[test]
fn can_assign_jobs_with_same_affinity_to_same_vehicle() {
    let fleet = create_test_fleet();
    // Test basic affinity matching without sequence validation
    let job = Job::Single(create_test_single(Some("affinity1"), None));
    let route_ctx = RouteContextBuilder::default()
        .with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build())
        .build();

    // Create a solution context with existing affinity assignment to v1
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![Some("affinity1")])]);
    
    // Accept the state to update affinity tracking
    let feature = create_test_affinity_feature();
    feature.state.as_ref().unwrap().accept_solution_state(&mut solution_ctx);

    let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &job };

    let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);

    assert!(result.is_none());
}

#[test]
fn cannot_assign_jobs_with_same_affinity_to_different_vehicle() {
    let fleet = create_test_fleet();
    let job = Job::Single(create_test_single(Some("affinity1"), None));
    let route_ctx = RouteContextBuilder::default()
        .with_route(RouteBuilder::default().with_vehicle(&fleet, "v2").build())
        .build();

    // Create a solution context with existing affinity assignment to v1
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![Some("affinity1")])]);
    
    // Accept the state to update affinity tracking
    let feature = create_test_affinity_feature();
    feature.state.as_ref().unwrap().accept_solution_state(&mut solution_ctx);

    let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &job };

    let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);

    assert_eq!(result, ConstraintViolation::fail(VIOLATION_CODE));
}

#[test]
fn can_assign_jobs_with_different_affinity_to_any_vehicle() {
    let fleet = create_test_fleet();
    let job = Job::Single(create_test_single(Some("affinity2"), None));
    let route_ctx = RouteContextBuilder::default()
        .with_route(RouteBuilder::default().with_vehicle(&fleet, "v2").build())
        .build();

    // Create a solution context with existing affinity assignment to v1
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![Some("affinity1")])]);
    
    // Accept the state to update affinity tracking
    let feature = create_test_affinity_feature();
    feature.state.as_ref().unwrap().accept_solution_state(&mut solution_ctx);

    let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &job };

    let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);

    assert!(result.is_none());
}

#[test]
fn can_assign_jobs_without_affinity_to_any_vehicle() {
    let fleet = create_test_fleet();
    let job = Job::Single(create_test_single(None, None));
    let route_ctx = RouteContextBuilder::default()
        .with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build())
        .build();

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![Some("affinity1")])]);
    
    let feature = create_test_affinity_feature();
    feature.state.as_ref().unwrap().accept_solution_state(&mut solution_ctx);

    let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &job };

    let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);

    assert!(result.is_none());
}