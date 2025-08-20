//! A feature to model vehicle affinity for jobs.

use super::*;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/vehicle_affinity_test.rs"]
mod vehicle_affinity_test;

custom_dimension!(pub JobAffinity typeof String);
custom_dimension!(pub JobAffinitySequence typeof u32);
custom_dimension!(pub JobAffinityDurationDays typeof u32);
custom_solution_state!(VehicleAffinities typeof HashMap<String, Arc<Vehicle>>);
custom_solution_state!(AffinitySchedules typeof HashMap<String, Vec<(u32, Timestamp)>>);

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
                    
                    // Check consecutive day scheduling if sequence and duration are specified
                    if let (Some(sequence), Some(duration_days)) = (
                        job.dimens().get_job_affinity_sequence(),
                        job.dimens().get_job_affinity_duration_days()
                    ) {
                        if let Some(schedules) = solution_ctx.state.get_affinity_schedules() {
                            if let Some(existing_schedule) = schedules.get(affinity) {
                                // Validate consecutive scheduling
                                if !validate_consecutive_schedule(existing_schedule, *sequence, *duration_days, job) {
                                    return ConstraintViolation::fail(self.code);
                                }
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
            
            // Update schedule tracking for consecutive validation
            if let Some(sequence) = job.dimens().get_job_affinity_sequence() {
                if let Some(timestamp) = extract_job_start_time(job) {
                    let mut schedules = solution_ctx.state.get_affinity_schedules().cloned().unwrap_or_default();
                    schedules.entry(affinity.clone()).or_default().push((*sequence, timestamp));
                    
                    // Keep schedule sorted
                    if let Some(schedule) = schedules.get_mut(affinity) {
                        schedule.sort_by_key(|(seq, _)| *seq);
                    }
                    
                    solution_ctx.state.set_affinity_schedules(schedules);
                }
            }
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
        
        // Update affinity schedules for consecutive day validation
        let mut schedules: HashMap<String, Vec<(u32, Timestamp)>> = HashMap::new();
        
        for route_ctx in &solution_ctx.routes {
            for job in route_ctx.route().tour.jobs() {
                if let (Some(affinity), Some(sequence)) = (
                    job.dimens().get_job_affinity(),
                    job.dimens().get_job_affinity_sequence()
                ) {
                    // Extract timestamp from job's first place
                    if let Some(timestamp) = extract_job_start_time(job) {
                        schedules.entry(affinity.clone()).or_default().push((*sequence, timestamp));
                    }
                }
            }
        }
        
        // Sort schedules by sequence for validation
        for schedule in schedules.values_mut() {
            schedule.sort_by_key(|(seq, _)| *seq);
        }
        
        solution_ctx.state.set_affinity_schedules(schedules);
    }
}

/// Validates that a new job can be scheduled consecutively with existing jobs in the affinity group.
fn validate_consecutive_schedule(
    existing_schedule: &[(u32, Timestamp)],
    new_sequence: u32,
    _duration_days: u32,
    job: &Job,
) -> bool {
    // Extract expected timestamp for this job
    let Some(new_timestamp) = extract_job_start_time(job) else {
        return true; // If no timestamp, allow assignment
    };
    
    // Calculate expected day based on sequence (assuming 24-hour days)
    let day_duration = 24.0 * 3600.0; // seconds in a day
    
    // Find the base timestamp (sequence 0)
    let base_timestamp = if let Some((_, first_timestamp)) = existing_schedule.first() {
        // Calculate base from first scheduled job
        let first_sequence = existing_schedule[0].0;
        first_timestamp - (first_sequence as f64 * day_duration)
    } else {
        // If no existing jobs, use current job as base
        new_timestamp - (new_sequence as f64 * day_duration)
    };
    
    let expected_timestamp = base_timestamp + (new_sequence as f64 * day_duration);
    
    // Allow some tolerance for time windows (e.g., ±4 hours)
    let tolerance = 4.0 * 3600.0;
    let time_diff = (new_timestamp - expected_timestamp).abs();
    
    // Validate consecutive scheduling
    time_diff <= tolerance
}

/// Extracts the start time from a job's first place.
fn extract_job_start_time(job: &Job) -> Option<Timestamp> {
    job.places().next().and_then(|place| {
        place.times.first().map(|time_span| match time_span {
            TimeSpan::Window(window) => window.start,
            TimeSpan::Offset(offset) => offset.start,
        })
    })
}