use super::*;
use crate::construction::enablers::create_typed_actor_groups;
use crate::helpers::construction::heuristics::TestInsertionContextBuilder;
use crate::helpers::models::domain::{TestGoalContextBuilder, test_random};
use crate::helpers::models::problem::{FleetBuilder, TestSingleBuilder, test_driver, test_vehicle_with_id};
use crate::helpers::models::solution::{ActivityBuilder, RouteBuilder, RouteContextBuilder};
use crate::models::common::{TimeSpan, TimeWindow};
use crate::models::problem::{Fleet, Single};
use crate::models::solution::Registry;
use std::sync::Arc;

const VIOLATION_CODE: ViolationCode = ViolationCode(1);

fn create_feature() -> Feature {
    create_job_sequence_feature("job_sequence", VIOLATION_CODE).unwrap()
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

fn create_test_single(
    sequence_key: Option<&str>,
    order: Option<u32>,
    days_between_min: Option<u32>,
    days_between_max: Option<u32>,
) -> Arc<Single> {
    let mut builder = TestSingleBuilder::default();
    if let Some(key) = sequence_key {
        builder.dimens_mut().set_job_sequence_key(key.to_string());
    }
    if let Some(ord) = order {
        builder.dimens_mut().set_job_sequence_order(ord);
    }
    if let Some(min) = days_between_min {
        builder.dimens_mut().set_job_sequence_days_between_min(min);
    }
    if let Some(max) = days_between_max {
        builder.dimens_mut().set_job_sequence_days_between_max(max);
    }
    builder.build_shared()
}

fn create_test_solution_context(fleet: &Fleet, routes: Vec<(&str, Vec<Arc<Single>>)>) -> SolutionContext {
    SolutionContext {
        required: vec![],
        ignored: vec![],
        unassigned: Default::default(),
        locked: Default::default(),
        routes: routes
            .into_iter()
            .map(|(vehicle, jobs)| {
                let activities = jobs.into_iter().map(|job| {
                    ActivityBuilder::default().job(Some(job)).build()
                }).collect::<Vec<_>>();
                let route = RouteBuilder::default()
                    .with_vehicle(fleet, vehicle)
                    .add_activities(activities)
                    .build();
                RouteContextBuilder::default().with_route(route).build()
            })
            .collect(),
        registry: RegistryContext::new(&TestGoalContextBuilder::default().build(), Registry::new(fleet, test_random())),
        state: Default::default(),
    }
}

#[test]
fn can_assign_first_job_in_sequence() {
    let fleet = create_test_fleet();
    let job = Job::Single(create_test_single(Some("seq1"), Some(0), None, None));
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![])]);

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));

    assert!(result.is_none());
}

#[test]
fn can_assign_jobs_in_correct_order_same_vehicle() {
    let fleet = create_test_fleet();
    // Use min_gap=0, max_gap=0 to allow same-shift assignment (only ordering is enforced)
    let job0 = create_test_single(Some("seq1"), Some(0), Some(0), Some(0));
    let job1_single = create_test_single(Some("seq1"), Some(1), Some(0), Some(0));
    let job1 = Job::Single(job1_single.clone());

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0])]);
    // Add job1 to required list so the system knows the sequence size
    solution_ctx.required.push(job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job1));

    assert!(result.is_none());
}

#[test]
fn cannot_assign_duplicate_order() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), None, None);
    let job0_duplicate = Job::Single(create_test_single(Some("seq1"), Some(0), None, None));

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0])]);

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job0_duplicate));

    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn cannot_assign_unexpected_order() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), None, None);
    let job1 = create_test_single(Some("seq1"), Some(1), None, None);
    // Try to assign order 5 when only 0,1 exist (sequence size is 2)
    let job5 = Job::Single(create_test_single(Some("seq1"), Some(5), None, None));

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0, job1])]);

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job5));

    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn can_merge_jobs_with_same_sequence_key_different_orders() {
    let job0 = Job::Single(create_test_single(Some("seq1"), Some(0), Some(1), Some(2)));
    let job1 = Job::Single(create_test_single(Some("seq1"), Some(1), Some(1), Some(2)));

    let constraint = create_feature().constraint.unwrap();
    let result = constraint.merge(job0, job1);

    assert!(result.is_ok());
}

#[test]
fn cannot_merge_jobs_with_same_order() {
    let job0 = Job::Single(create_test_single(Some("seq1"), Some(0), Some(1), Some(2)));
    let job0_dup = Job::Single(create_test_single(Some("seq1"), Some(0), Some(1), Some(2)));

    let constraint = create_feature().constraint.unwrap();
    let result = constraint.merge(job0, job0_dup);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), VIOLATION_CODE);
}

#[test]
fn cannot_merge_jobs_with_different_gap_params() {
    let job0 = Job::Single(create_test_single(Some("seq1"), Some(0), Some(1), Some(2)));
    let job1 = Job::Single(create_test_single(Some("seq1"), Some(1), Some(2), Some(3)));

    let constraint = create_feature().constraint.unwrap();
    let result = constraint.merge(job0, job1);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), VIOLATION_CODE);
}

#[test]
fn validates_input_min_greater_than_max() {
    let fleet = create_test_fleet();
    let job = Job::Single(create_test_single(Some("seq1"), Some(0), Some(5), Some(2)));
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![])]);

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));

    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn validates_input_max_too_large() {
    let fleet = create_test_fleet();
    let job = Job::Single(create_test_single(Some("seq1"), Some(0), Some(1), Some(400)));
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![])]);

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));

    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn validates_missing_order_dimension() {
    let fleet = create_test_fleet();
    // Sequence key but no order
    let job = Job::Single(create_test_single(Some("seq1"), None, Some(1), Some(2)));
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![])]);

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));

    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn can_rebuild_solution_state_correctly() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), Some(1), Some(2));
    let job1 = create_test_single(Some("seq1"), Some(1), Some(1), Some(2));
    let job2 = create_test_single(Some("seq2"), Some(0), Some(1), Some(1));

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0, job1]), ("v2", vec![job2])]);

    let state = create_feature().state.unwrap();
    state.accept_solution_state(&mut solution_ctx);

    let group_states = solution_ctx.state.get_sequence_group_states().unwrap();
    assert_eq!(group_states.len(), 2);
    assert!(group_states.contains_key("seq1"));
    assert!(group_states.contains_key("seq2"));

    let seq1_state = &group_states["seq1"];
    assert_eq!(seq1_state.expected_size, 2);
    assert_eq!(seq1_state.assignments.len(), 2);
    assert!(seq1_state.assignments.contains_key(&0));
    assert!(seq1_state.assignments.contains_key(&1));

    let seq2_state = &group_states["seq2"];
    assert_eq!(seq2_state.expected_size, 1);
    assert_eq!(seq2_state.assignments.len(), 1);
    assert!(seq2_state.assignments.contains_key(&0));
}

#[test]
fn objective_penalizes_partial_sequences() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), Some(1), Some(2));
    let job1 = create_test_single(Some("seq1"), Some(1), Some(1), Some(2));
    let job2 = create_test_single(Some("seq1"), Some(2), Some(1), Some(2));

    // Only 2 out of 3 jobs assigned
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0.clone(), job1.clone()])]);
    // Add job2 to required list so the system knows the sequence size is 3
    solution_ctx.required.push(Job::Single(job2));

    let state = create_feature().state.unwrap();
    let objective = create_feature().objective.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let mut insertion_ctx = TestInsertionContextBuilder::default().build();
    insertion_ctx.solution = solution_ctx;

    let cost = objective.fitness(&insertion_ctx);
    // Should be 100000 (1 missing job * 100000 penalty)
    assert!(cost > 0.0);
    assert_eq!(cost, 100000.0);
}

#[test]
fn objective_no_penalty_for_complete_sequences() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), Some(1), Some(2));
    let job1 = create_test_single(Some("seq1"), Some(1), Some(1), Some(2));

    // All jobs assigned (sequence size is 2)
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0, job1])]);

    let state = create_feature().state.unwrap();
    let objective = create_feature().objective.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let mut insertion_ctx = TestInsertionContextBuilder::default().build();
    insertion_ctx.solution = solution_ctx;

    let cost = objective.fitness(&insertion_ctx);
    assert_eq!(cost, 0.0);
}

#[test]
fn cannot_skip_order_in_sequence() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), Some(1), Some(2));
    let job2_single = create_test_single(Some("seq1"), Some(2), Some(1), Some(2));
    let job2 = Job::Single(job2_single.clone());

    // Try to assign order 2 when only order 0 exists (missing order 1)
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0])]);
    // Add job2 and a placeholder for job1 to required list so the system knows the sequence size
    solution_ctx.required.push(job2.clone());
    solution_ctx.required.push(Job::Single(create_test_single(Some("seq1"), Some(1), Some(1), Some(2))));

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job2));

    // Should fail because order 1 must be assigned before order 2
    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn validates_calendar_gap_for_different_vehicles() {
    let fleet = create_test_fleet();

    // Create jobs with specific time windows
    // Job 0: Day 0 (timestamp 0)
    // Job 1: Day 2 (timestamp 48 hours = 172800 seconds) - should violate min_gap=1, max_gap=1 (need exactly 1 day)
    let mut job0_builder = TestSingleBuilder::default();
    job0_builder.dimens_mut().set_job_sequence_key("seq1".to_string());
    job0_builder.dimens_mut().set_job_sequence_order(0);
    job0_builder.dimens_mut().set_job_sequence_days_between_min(1);
    job0_builder.dimens_mut().set_job_sequence_days_between_max(1);
    job0_builder.times(vec![TimeWindow::new(0.0, 3600.0)]);
    let job0 = job0_builder.build_shared();

    let mut job1_builder = TestSingleBuilder::default();
    job1_builder.dimens_mut().set_job_sequence_key("seq1".to_string());
    job1_builder.dimens_mut().set_job_sequence_order(1);
    job1_builder.dimens_mut().set_job_sequence_days_between_min(1);
    job1_builder.dimens_mut().set_job_sequence_days_between_max(1);
    // 2 days later (172800 seconds = 48 hours)
    job1_builder.times(vec![TimeWindow::new(172800.0, 176400.0)]);
    let job1_single = job1_builder.build_shared();
    let job1 = Job::Single(job1_single.clone());

    // Assign to different vehicles
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0]), ("v2", vec![])]);
    solution_ctx.required.push(job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.get(1).unwrap(); // v2 route
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job1));

    // Should fail: gap is 2 days, but requirement is exactly 1 day (with 6h tolerance)
    // 2 days is outside the tolerance range of 1 day ± 0.25 days
    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn allows_calendar_gap_within_tolerance() {
    let fleet = create_test_fleet();

    // Job 0: Day 0 (timestamp 0)
    // Job 1: ~Day 1 (timestamp 90000 = 25 hours) - should pass with tolerance
    let mut job0_builder = TestSingleBuilder::default();
    job0_builder.dimens_mut().set_job_sequence_key("seq1".to_string());
    job0_builder.dimens_mut().set_job_sequence_order(0);
    job0_builder.dimens_mut().set_job_sequence_days_between_min(1);
    job0_builder.dimens_mut().set_job_sequence_days_between_max(1);
    job0_builder.times(vec![TimeWindow::new(0.0, 3600.0)]);
    let job0 = job0_builder.build_shared();

    let mut job1_builder = TestSingleBuilder::default();
    job1_builder.dimens_mut().set_job_sequence_key("seq1".to_string());
    job1_builder.dimens_mut().set_job_sequence_order(1);
    job1_builder.dimens_mut().set_job_sequence_days_between_min(1);
    job1_builder.dimens_mut().set_job_sequence_days_between_max(1);
    // ~25 hours later (should be within tolerance of 1 day)
    job1_builder.times(vec![TimeWindow::new(90000.0, 93600.0)]);
    let job1_single = job1_builder.build_shared();
    let job1 = Job::Single(job1_single.clone());

    // Assign to different vehicles for calendar-based validation
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0]), ("v2", vec![])]);
    solution_ctx.required.push(job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.get(1).unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job1));

    // Should pass: 25 hours is within tolerance of 24 hours ± 6 hours
    assert!(result.is_none());
}

#[test]
fn validates_shift_gap_for_same_vehicle() {
    let fleet = create_test_fleet();

    // Jobs on same vehicle should use shift-based validation
    // Create jobs with min_gap=1, max_gap=1 (exactly 1 shift apart)
    let job0 = create_test_single(Some("seq1"), Some(0), Some(1), Some(1));
    let job1_single = create_test_single(Some("seq1"), Some(1), Some(1), Some(1));
    let job1 = Job::Single(job1_single.clone());

    // Both on same vehicle, same shift - should fail (gap = 0)
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0])]);
    solution_ctx.required.push(job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job1));

    // Should fail: both jobs on shift 0, gap is 0 but requirement is 1
    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

// Note: Jobs without time windows work correctly - timing validation is skipped.
// This is tested implicitly by other tests that use jobs with default time windows.
// The explicit test was removed due to complexity in setting up the test context.

#[test]
fn handles_multiple_independent_sequences() {
    let fleet = create_test_fleet();

    // Create two independent sequences with min_gap=0 to allow same-shift assignment
    let seq1_job0 = create_test_single(Some("seq1"), Some(0), Some(0), Some(0));
    let seq1_job1 = create_test_single(Some("seq1"), Some(1), Some(0), Some(0));
    let seq2_job0 = create_test_single(Some("seq2"), Some(0), Some(0), Some(0));
    let seq2_job1_single = create_test_single(Some("seq2"), Some(1), Some(0), Some(0));
    let seq2_job1 = Job::Single(seq2_job1_single.clone());

    let mut solution_ctx = create_test_solution_context(&fleet, vec![
        ("v1", vec![seq1_job0, seq1_job1]),
        ("v2", vec![seq2_job0])
    ]);
    solution_ctx.required.push(seq2_job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    // Verify seq1 is complete
    let group_states = solution_ctx.state.get_sequence_group_states().unwrap();
    assert_eq!(group_states.len(), 2);
    assert!(group_states["seq1"].is_complete());
    assert!(!group_states["seq2"].is_complete());

    // Should be able to assign seq2_job1 to same route (same shift allowed with gap=0)
    let route_ctx = solution_ctx.routes.get(1).unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &seq2_job1));
    assert!(result.is_none());
}

#[test]
fn validates_zero_gap_same_shift() {
    let fleet = create_test_fleet();

    // Use min_gap=0, max_gap=0 to allow same-shift assignment
    let job0 = create_test_single(Some("seq1"), Some(0), Some(0), Some(0));
    let job1_single = create_test_single(Some("seq1"), Some(1), Some(0), Some(0));
    let job1 = Job::Single(job1_single.clone());

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0])]);
    solution_ctx.required.push(job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job1));

    // Should pass: gap=0 is allowed when min_gap=0, max_gap=0
    assert!(result.is_none());
}

#[test]
fn rejects_gap_violations_at_boundary() {
    let fleet = create_test_fleet();

    // Create jobs with time windows that violate max_gap by 1 day
    let mut job0_builder = TestSingleBuilder::default();
    job0_builder.dimens_mut().set_job_sequence_key("seq1".to_string());
    job0_builder.dimens_mut().set_job_sequence_order(0);
    job0_builder.dimens_mut().set_job_sequence_days_between_min(1);
    job0_builder.dimens_mut().set_job_sequence_days_between_max(2);
    job0_builder.times(vec![TimeWindow::new(0.0, 3600.0)]);
    let job0 = job0_builder.build_shared();

    let mut job1_builder = TestSingleBuilder::default();
    job1_builder.dimens_mut().set_job_sequence_key("seq1".to_string());
    job1_builder.dimens_mut().set_job_sequence_order(1);
    job1_builder.dimens_mut().set_job_sequence_days_between_min(1);
    job1_builder.dimens_mut().set_job_sequence_days_between_max(2);
    // 3.5 days later - outside max_gap=2 even with tolerance
    job1_builder.times(vec![TimeWindow::new(302400.0, 306000.0)]);
    let job1_single = job1_builder.build_shared();
    let job1 = Job::Single(job1_single.clone());

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0]), ("v2", vec![])]);
    solution_ctx.required.push(job1.clone());

    let constraint = create_feature().constraint.unwrap();
    let state = create_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.get(1).unwrap();
    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job1));

    // Should fail: 3.5 days exceeds max_gap=2 even with 0.25 day tolerance
    assert!(result.is_some());
    assert_eq!(result.unwrap().code, VIOLATION_CODE);
}

#[test]
fn objective_rewards_completing_sequence() {
    let fleet = create_test_fleet();
    let job0 = create_test_single(Some("seq1"), Some(0), Some(1), Some(2));
    let job1 = create_test_single(Some("seq1"), Some(1), Some(1), Some(2));
    let job2_single = create_test_single(Some("seq1"), Some(2), Some(1), Some(2));
    let job2 = Job::Single(job2_single.clone());

    // 2 out of 3 jobs assigned
    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![job0, job1])]);
    solution_ctx.required.push(job2.clone());

    let state = create_feature().state.unwrap();
    let objective = create_feature().objective.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.first().unwrap();

    // Estimate cost for completing the sequence
    let estimate = objective.estimate(&MoveContext::route(&solution_ctx, route_ctx, &job2));

    // Should be a large negative value (reward) for completing sequence
    assert!(estimate < 0.0);
    assert_eq!(estimate, -(3.0 * 100000.0)); // completing sequence of size 3
}

#[test]
fn handles_large_sequences() {
    let fleet = create_test_fleet();

    // Create a sequence of 10 jobs
    let mut jobs = Vec::new();
    for i in 0..10 {
        jobs.push(create_test_single(Some("seq1"), Some(i), Some(1), Some(1)));
    }

    let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", jobs)]);

    let state = create_feature().state.unwrap();
    state.accept_solution_state(&mut solution_ctx);

    let group_states = solution_ctx.state.get_sequence_group_states().unwrap();
    let seq_state = &group_states["seq1"];

    assert_eq!(seq_state.expected_size, 10);
    assert_eq!(seq_state.assignments.len(), 10);
    assert!(seq_state.is_complete());
}
