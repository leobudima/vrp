//! A feature to ensure jobs with the same assignee key are assigned to the same vehicle.
//!
//! This feature allows grouping jobs that should be handled by the same vehicle across
//! multiple routes and days, without requiring specific ordering or timing constraints.
//! It's simpler than the affinity feature and focuses purely on vehicle assignment.

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/same_assignee_test.rs"]
mod same_assignee_test;

use super::*;
use std::collections::HashMap;
use std::sync::Arc;

custom_dimension!(pub JobSameAssigneeKey typeof String);
custom_solution_state!(SameAssigneeAssignments typeof HashMap<String, Arc<Vehicle>>);

/// Creates a same assignee feature as a hard constraint.
///
/// This ensures that all jobs with the same assignee key are assigned to the same vehicle
/// across all routes, regardless of shifts or days.
pub fn create_same_assignee_feature(name: &str, code: ViolationCode) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(SameAssigneeConstraint { code })
        .with_state(SameAssigneeState {})
        .build()
}

struct SameAssigneeConstraint {
    code: ViolationCode,
}

impl FeatureConstraint for SameAssigneeConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { solution_ctx, route_ctx, job } => {
                job.dimens().get_job_same_assignee_key().and_then(|assignee_key| {
                    let current_vehicle = &route_ctx.route().actor.vehicle;

                    // Check if this assignee key is already assigned to a different vehicle
                    if let Some(assignments) = solution_ctx.state.get_same_assignee_assignments() {
                        if let Some(assigned_vehicle) = assignments.get(assignee_key) {
                            if !Arc::ptr_eq(assigned_vehicle, current_vehicle) {
                                return ConstraintViolation::fail(self.code);
                            }
                        }
                    }

                    None
                })
            }
            MoveContext::Activity { .. } => None,
        }
    }

    fn merge(&self, source: Job, candidate: Job) -> Result<Job, ViolationCode> {
        match (source.dimens().get_job_same_assignee_key(), candidate.dimens().get_job_same_assignee_key()) {
            (None, None) => Ok(source),
            (Some(s_key), Some(c_key)) if s_key == c_key => Ok(source),
            _ => Err(self.code),
        }
    }
}

struct SameAssigneeState {}

impl FeatureState for SameAssigneeState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, job: &Job) {
        if let Some(assignee_key) = job.dimens().get_job_same_assignee_key() {
            let route_ctx = solution_ctx.routes.get(route_index).unwrap();
            let vehicle = route_ctx.route().actor.vehicle.clone();

            // Update assignee key to vehicle mapping
            let mut assignments = solution_ctx.state.get_same_assignee_assignments().cloned().unwrap_or_default();
            assignments.insert(assignee_key.clone(), vehicle);
            solution_ctx.state.set_same_assignee_assignments(assignments);
        }
    }

    fn accept_route_state(&self, _: &mut RouteContext) {}

    fn accept_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let mut assignments: HashMap<String, Arc<Vehicle>> = HashMap::new();

        // Rebuild assignments from all routes
        for route_ctx in &solution_ctx.routes {
            let vehicle = route_ctx.route().actor.vehicle.clone();

            for job in route_ctx.route().tour.jobs() {
                if let Some(assignee_key) = job.dimens().get_job_same_assignee_key() {
                    assignments.insert(assignee_key.clone(), vehicle.clone());
                }
            }
        }

        solution_ctx.state.set_same_assignee_assignments(assignments);
    }
}
