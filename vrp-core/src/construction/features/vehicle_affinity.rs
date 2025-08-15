//! A feature to model vehicle affinity for jobs.

use super::*;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/vehicle_affinity_test.rs"]
mod vehicle_affinity_test;

custom_dimension!(pub JobAffinity typeof String);
custom_solution_state!(VehicleAffinities typeof HashMap<String, Arc<Vehicle>>);

/// Creates a vehicle affinity feature as a hard constraint.
pub fn create_vehicle_affinity_feature(name: &str, code: ViolationCode) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(VehicleAffinityConstraint { code })
        .with_state(VehicleAffinityState {})
        .build()
}

struct VehicleAffinityConstraint {
    code: ViolationCode,
}

impl FeatureConstraint for VehicleAffinityConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { solution_ctx, route_ctx, job } => {
                job.dimens().get_job_affinity().and_then(|affinity| {
                    let current_vehicle = &route_ctx.route().actor.vehicle;
                    
                    // Check if this affinity is already assigned to a different vehicle
                    if let Some(affinities) = solution_ctx.state.get_vehicle_affinities() {
                        if let Some(assigned_vehicle) = affinities.get(affinity) {
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
        match (source.dimens().get_job_affinity(), candidate.dimens().get_job_affinity()) {
            (None, None) => Ok(source),
            (Some(s_affinity), Some(c_affinity)) if s_affinity == c_affinity => Ok(source),
            _ => Err(self.code),
        }
    }
}

struct VehicleAffinityState {}

impl FeatureState for VehicleAffinityState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, job: &Job) {
        if let Some(affinity) = job.dimens().get_job_affinity() {
            let route_ctx = solution_ctx.routes.get(route_index).unwrap();
            let vehicle = route_ctx.route().actor.vehicle.clone();
            
            let mut affinities = solution_ctx.state.get_vehicle_affinities().cloned().unwrap_or_default();
            affinities.insert(affinity.clone(), vehicle);
            
            solution_ctx.state.set_vehicle_affinities(affinities);
        }
    }

    fn accept_route_state(&self, _: &mut RouteContext) {}

    fn accept_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let mut affinities: HashMap<String, Arc<Vehicle>> = HashMap::new();
        
        for route_ctx in &solution_ctx.routes {
            let vehicle = route_ctx.route().actor.vehicle.clone();
            
            for job in route_ctx.route().tour.jobs() {
                if let Some(affinity) = job.dimens().get_job_affinity() {
                    affinities.insert(affinity.clone(), vehicle.clone());
                }
            }
        }
        
        solution_ctx.state.set_vehicle_affinities(affinities);
    }
}