//! Calculates a total value of the served jobs.

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/total_value_test.rs"]
mod total_value_test;

use super::*;
use crate::models::problem::Actor;
use crate::utils::Either;
use std::cmp::Ordering;

/// Specifies a job value function which takes into account actor and job.
pub type ActorValueFn = Arc<dyn Fn(&Actor, &Job) -> Float + Send + Sync>;
/// Specifies a job value function which takes into account only a job.
pub type SimpleValueFn = Arc<dyn Fn(&Job) -> Float + Send + Sync>;
/// Specifies a job value reader as a variant of two functions.
pub type JobReadValueFn = Either<SimpleValueFn, ActorValueFn>;
/// Specifies a job write value.
pub type JobWriteValueFn = Arc<dyn Fn(Job, Float) -> Job + Send + Sync>;
/// A job value estimation function.
type EstimateValueFn = Arc<dyn Fn(&RouteContext, &Job) -> Float + Send + Sync>;

/// Maximizes a total value of served jobs.
pub fn create_maximize_total_job_value_feature(
    name: &str,
    job_read_value_fn: JobReadValueFn,
    job_write_value_fn: JobWriteValueFn,
    merge_code: ViolationCode,
) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_objective(MaximizeTotalValueObjective {
            estimate_value_fn: Arc::new({
                let job_read_value_fn = job_read_value_fn.clone();
                let sign = -1.;
                move |route_ctx, job| {
                    sign * match &job_read_value_fn {
                        JobReadValueFn::Left(left_fn) => (left_fn)(job),
                        JobReadValueFn::Right(right_fn) => (right_fn)(route_ctx.route().actor.as_ref(), job),
                    }
                }
            }),
        })
        .with_constraint(MaximizeTotalValueConstraint { merge_code, job_read_value_fn, job_write_value_fn })
        .build()
}

struct MaximizeTotalValueObjective {
    estimate_value_fn: EstimateValueFn,
}

impl FeatureObjective for MaximizeTotalValueObjective {
    fn fitness(&self, solution: &InsertionContext) -> Cost {
        solution.solution.routes.iter().fold(0., |acc, route_ctx| {
            route_ctx.route().tour.jobs().fold(acc, |acc, job| acc + (self.estimate_value_fn)(route_ctx, job))
        })
    }

    fn estimate(&self, move_ctx: &MoveContext<'_>) -> Cost {
        match move_ctx {
            MoveContext::Route { route_ctx, job, .. } => (self.estimate_value_fn)(route_ctx, job),
            MoveContext::Activity { .. } => Cost::default(),
        }
    }
}

struct MaximizeTotalValueConstraint {
    merge_code: ViolationCode,
    job_read_value_fn: JobReadValueFn,
    job_write_value_fn: JobWriteValueFn,
}

impl FeatureConstraint for MaximizeTotalValueConstraint {
    fn evaluate(&self, _: &MoveContext<'_>) -> Option<ConstraintViolation> {
        None
    }

    fn merge(&self, source: Job, candidate: Job) -> Result<Job, ViolationCode> {
        match &self.job_read_value_fn {
            JobReadValueFn::Left(left_fn) => {
                let source_value = (left_fn)(&source);
                let candidate_value = (left_fn)(&candidate);
                let new_value = source_value + candidate_value;

                Ok(if compare_floats(new_value, source_value) != Ordering::Equal {
                    (self.job_write_value_fn)(source, new_value)
                } else {
                    source
                })
            }
            JobReadValueFn::Right(_) => Err(self.merge_code),
        }
    }
}
