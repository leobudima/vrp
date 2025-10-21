use super::*;
use crate::construction::enablers::create_typed_actor_groups;
use crate::helpers::models::domain::{TestGoalContextBuilder, test_random};
use crate::helpers::models::problem::{FleetBuilder, TestSingleBuilder, test_driver, test_vehicle_with_id};
use crate::helpers::models::solution::{ActivityBuilder, RouteBuilder, RouteContextBuilder, RouteStateBuilder};
use crate::models::problem::{Actor, Fleet, Single};
use crate::models::solution::Registry;
use std::collections::HashMap;
use std::sync::Arc;

const VIOLATION_CODE: ViolationCode = ViolationCode(1);

fn create_feature() -> Feature {
    create_same_assignee_feature("same_assignee", VIOLATION_CODE).unwrap()
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

fn create_test_single(assignee_key: Option<&str>) -> Arc<Single> {
    let mut builder = TestSingleBuilder::default();
    if let Some(key) = assignee_key {
        builder.dimens_mut().set_job_same_assignee_key(key.to_string());
    }
    builder.build_shared()
}

fn create_test_solution_context(
    total_jobs: usize,
    fleet: &Fleet,
    routes: Vec<(&str, Vec<Option<&str>>)>,
) -> SolutionContext {
    SolutionContext {
        required: (0..total_jobs).map(|_| Job::Single(create_test_single(None))).collect(),
        ignored: vec![],
        unassigned: Default::default(),
        locked: Default::default(),
        routes: routes
            .into_iter()
            .map(|(vehicle, assignee_keys)| {
                RouteContextBuilder::default()
                    .with_route(
                        RouteBuilder::default()
                            .with_vehicle(fleet, vehicle)
                            .add_activities(assignee_keys.into_iter().map(|key| {
                                ActivityBuilder::with_location(1).job(Some(create_test_single(key))).build()
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

fn get_actor(fleet: &Fleet, vehicle: &str) -> Arc<Actor> {
    fleet.actors.iter().find(|actor| actor.vehicle.dimens.get_vehicle_id().unwrap() == vehicle).unwrap().clone()
}

#[test]
fn can_assign_jobs_with_same_assignee_key_to_same_vehicle() {
    let fleet = create_test_fleet();
    let mut solution_ctx = create_test_solution_context(2, &fleet, vec![("v1", vec![Some("tech_alice")])]);
    let job = Job::Single(create_test_single(Some("tech_alice")));

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    // Rebuild state from existing route
    state.accept_solution_state(&mut solution_ctx);

    // Second job with same assignee key should be accepted
    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));
    assert!(result.is_none());
}

#[test]
fn cannot_assign_jobs_with_same_assignee_key_to_different_vehicles() {
    let fleet = create_test_fleet();
    let mut solution_ctx = create_test_solution_context(
        2,
        &fleet,
        vec![("v1", vec![Some("tech_alice")]), ("v2", vec![])],
    );
    let job = Job::Single(create_test_single(Some("tech_alice")));

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    // Rebuild state from existing routes
    state.accept_solution_state(&mut solution_ctx);

    // Try to assign job with same key to different vehicle - should fail
    let route_ctx2 = solution_ctx.routes.get(1).unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx2, &job));

    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn can_assign_jobs_with_different_assignee_keys_to_different_vehicles() {
    let fleet = create_test_fleet();
    let mut solution_ctx = create_test_solution_context(
        2,
        &fleet,
        vec![("v1", vec![Some("tech_alice")]), ("v2", vec![])],
    );
    let job = Job::Single(create_test_single(Some("tech_bob")));

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    // Rebuild state from existing routes
    state.accept_solution_state(&mut solution_ctx);

    // Assign job with different key to different vehicle - should succeed
    let route_ctx2 = solution_ctx.routes.get(1).unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx2, &job));
    assert!(result.is_none());
}

#[test]
fn can_assign_jobs_without_assignee_key_to_any_vehicle() {
    let fleet = create_test_fleet();
    let mut solution_ctx = create_test_solution_context(2, &fleet, vec![("v1", vec![Some("tech_alice")])]);
    let job = Job::Single(create_test_single(None));

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    // Rebuild state from existing routes
    state.accept_solution_state(&mut solution_ctx);

    // Job without key should be accepted on any vehicle
    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));
    assert!(result.is_none());
}

#[test]
fn can_merge_jobs_with_same_assignee_key() {
    let job1 = Job::Single(create_test_single(Some("tech_alice")));
    let job2 = Job::Single(create_test_single(Some("tech_alice")));

    let constraint = create_feature().constraint.unwrap();
    let result = constraint.merge(job1, job2);

    assert!(result.is_ok());
}

#[test]
fn cannot_merge_jobs_with_different_assignee_keys() {
    let job1 = Job::Single(create_test_single(Some("tech_alice")));
    let job2 = Job::Single(create_test_single(Some("tech_bob")));

    let constraint = create_feature().constraint.unwrap();
    let result = constraint.merge(job1, job2);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), VIOLATION_CODE);
}

#[test]
fn can_rebuild_solution_state_correctly() {
    let fleet = create_test_fleet();
    let mut solution_ctx = create_test_solution_context(
        3,
        &fleet,
        vec![("v1", vec![Some("tech_alice")]), ("v2", vec![Some("tech_bob")])],
    );

    let state = create_feature().state.unwrap();

    // Rebuild the state
    state.accept_solution_state(&mut solution_ctx);

    // Verify assignments are tracked
    let assignments = solution_ctx.state.get_same_assignee_assignments().unwrap();
    assert_eq!(assignments.len(), 2);
    assert!(assignments.contains_key("tech_alice"));
    assert!(assignments.contains_key("tech_bob"));

    // Verify correct vehicles are assigned
    let v1_actor = get_actor(&fleet, "v1");
    let v2_actor = get_actor(&fleet, "v2");
    assert!(Arc::ptr_eq(&assignments["tech_alice"], &v1_actor.vehicle));
    assert!(Arc::ptr_eq(&assignments["tech_bob"], &v2_actor.vehicle));
}

#[test]
fn can_accept_insertion() {
    let fleet = create_test_fleet();
    let mut solution_ctx = create_test_solution_context(2, &fleet, vec![("v1", vec![])]);
    let job = Job::Single(create_test_single(Some("tech_alice")));

    let state = create_feature().state.unwrap();

    // Insert job
    state.accept_insertion(&mut solution_ctx, 0, &job);

    // Verify assignment is tracked
    let assignments = solution_ctx.state.get_same_assignee_assignments().unwrap();
    assert_eq!(assignments.len(), 1);
    assert!(assignments.contains_key("tech_alice"));
}
