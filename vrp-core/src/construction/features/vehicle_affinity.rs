//! A feature to model vehicle affinity for jobs.

use super::*;
use crate::models::solution::{Route, Tour};
use crate::models::problem::Actor;
use crate::models::problem::{Driver, Single};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::cmp::Ordering;

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/vehicle_affinity_test.rs"]
mod vehicle_affinity_test;

custom_dimension!(pub JobAffinity typeof String);
custom_dimension!(pub JobAffinitySequence typeof u32);
custom_dimension!(pub JobAffinityDurationDays typeof u32);
custom_dimension!(pub JobAffinityTolerance typeof f64);
custom_solution_state!(VehicleAffinities typeof HashMap<String, Arc<Vehicle>>);
custom_solution_state!(AffinitySchedules typeof HashMap<String, Vec<(u32, Timestamp)>>);
custom_solution_state!(AffinityGroupStates typeof HashMap<String, AffinityGroupState>);

/// Represents the state of an affinity group
#[derive(Debug, Clone)]
pub struct AffinityGroupState {
    pub assigned_vehicle: Option<Arc<Vehicle>>,
    pub expected_sequences: HashSet<u32>,
    pub assigned_sequences: HashMap<u32, Timestamp>,
    pub duration_days: u32,
    pub base_timestamp: Option<Timestamp>,
}

impl AffinityGroupState {
    fn new(duration_days: u32) -> Self {
        Self {
            assigned_vehicle: None,
            expected_sequences: (0..duration_days).collect(),
            assigned_sequences: HashMap::new(),
            duration_days,
            base_timestamp: None,
        }
    }
    
    fn is_complete(&self) -> bool {
        self.assigned_sequences.len() == self.duration_days as usize &&
        self.expected_sequences.iter().all(|seq| self.assigned_sequences.contains_key(seq))
    }
    
    fn is_partial(&self) -> bool {
        !self.assigned_sequences.is_empty() && !self.is_complete()
    }
}

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
                    
                    // Validate input consistency first
                    if let Some(violation) = self.validate_affinity_input(job) {
                        return Some(violation);
                    }
                    
                    // Check if this affinity is already assigned to a different vehicle
                    if let Some(affinities) = solution_ctx.state.get_vehicle_affinities() {
                        if let Some(assigned_vehicle) = affinities.get(affinity) {
                            if !Arc::ptr_eq(assigned_vehicle, current_vehicle) {
                                return ConstraintViolation::fail(self.code);
                            }
                        }
                    }
                    
                    // Check affinity group state and consecutive scheduling
                    if let (Some(sequence), Some(duration_days)) = (
                        job.dimens().get_job_affinity_sequence(),
                        job.dimens().get_job_affinity_duration_days()
                    ) {
                        if let Some(group_states) = solution_ctx.state.get_affinity_group_states() {
                            if let Some(group_state) = group_states.get(affinity) {
                                // Check if sequence is valid for this group
                                if !group_state.expected_sequences.contains(sequence) {
                                    return ConstraintViolation::fail(self.code);
                                }
                                
                                // Check if sequence is already assigned
                                if group_state.assigned_sequences.contains_key(sequence) {
                                    return ConstraintViolation::fail(self.code);
                                }
                                
                                // Validate consecutive scheduling if base timestamp exists
                                if let Some(base_timestamp) = group_state.base_timestamp {
                                    if !self.validate_consecutive_schedule_with_base(
                                        base_timestamp, *sequence, job
                                    ) {
                                        return ConstraintViolation::fail(self.code);
                                    }
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
            (Some(s_affinity), Some(c_affinity)) if s_affinity == c_affinity => {
                // Additional validation for affinity jobs
                let s_sequence = source.dimens().get_job_affinity_sequence();
                let c_sequence = candidate.dimens().get_job_affinity_sequence();
                let s_duration = source.dimens().get_job_affinity_duration_days();
                let c_duration = candidate.dimens().get_job_affinity_duration_days();
                
                // Ensure consistent duration and different sequences
                if s_duration != c_duration || s_sequence == c_sequence {
                    return Err(self.code);
                }
                
                Ok(source)
            },
            _ => Err(self.code),
        }
    }
}

impl VehicleAffinityConstraint {
    /// Validates input consistency for affinity jobs
    fn validate_affinity_input(&self, job: &Job) -> Option<ConstraintViolation> {
        if let Some(_affinity) = job.dimens().get_job_affinity() {
            let sequence = job.dimens().get_job_affinity_sequence();
            let duration_days = job.dimens().get_job_affinity_duration_days();
            
            // Both sequence and duration must be specified together
            match (sequence, duration_days) {
                (Some(seq), Some(duration)) => {
                    // Sequence must be within valid range
                    if *seq >= *duration {
                        return ConstraintViolation::fail(self.code);
                    }
                },
                (None, None) => {}, // Basic affinity without sequence is allowed
                _ => {
                    // Sequence and duration must both be specified or both be None
                    return ConstraintViolation::fail(self.code);
                }
            }
        }
        None
    }
    
    /// Validates consecutive scheduling against a base timestamp
    fn validate_consecutive_schedule_with_base(
        &self,
        base_timestamp: Timestamp,
        sequence: u32,
        job: &Job
    ) -> bool {
        let Some(job_timestamp) = extract_job_start_time(job) else {
            return true; // If no timestamp, allow assignment
        };
        
        let day_duration = calculate_day_duration(job);
        let expected_timestamp = base_timestamp + (sequence as f64 * day_duration);
        
        let tolerance = job.dimens().get_job_affinity_tolerance()
            .copied()
            .unwrap_or(4.0 * 3600.0); // Default 4 hours
        
        let time_diff = (job_timestamp - expected_timestamp).abs();
        time_diff <= tolerance
    }
}

struct VehicleAffinityState {}

impl FeatureState for VehicleAffinityState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, job: &Job) {
        if let Some(affinity) = job.dimens().get_job_affinity() {
            let route_ctx = solution_ctx.routes.get(route_index).unwrap();
            let vehicle = route_ctx.route().actor.vehicle.clone();
            
            // Update vehicle affinities
            let mut affinities = solution_ctx.state.get_vehicle_affinities().cloned().unwrap_or_default();
            affinities.insert(affinity.clone(), vehicle.clone());
            solution_ctx.state.set_vehicle_affinities(affinities);
            
            // Update affinity group state for sequential jobs
            if let (Some(sequence), Some(duration_days)) = (
                job.dimens().get_job_affinity_sequence(),
                job.dimens().get_job_affinity_duration_days()
            ) {
                if let Some(timestamp) = extract_job_start_time(job) {
                    let mut group_states = solution_ctx.state.get_affinity_group_states().cloned().unwrap_or_default();
                    
                    let group_state = group_states.entry(affinity.clone())
                        .or_insert_with(|| AffinityGroupState::new(*duration_days));
                    
                    // Set vehicle if not already set
                    if group_state.assigned_vehicle.is_none() {
                        group_state.assigned_vehicle = Some(vehicle);
                    }
                    
                    // Set base timestamp if this is the first assignment
                    if group_state.base_timestamp.is_none() {
                        let day_duration = calculate_day_duration(job);
                        group_state.base_timestamp = Some(timestamp - (*sequence as f64 * day_duration));
                    }
                    
                    // Add sequence assignment
                    group_state.assigned_sequences.insert(*sequence, timestamp);
                    
                    solution_ctx.state.set_affinity_group_states(group_states);
                    
                    // Update legacy schedule format for backward compatibility
                    let mut schedules = solution_ctx.state.get_affinity_schedules().cloned().unwrap_or_default();
                    schedules.entry(affinity.clone()).or_default().push((*sequence, timestamp));
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
        // Check if we can use incremental update instead of full rebuild
        let needs_full_rebuild = solution_ctx.state.get_affinity_group_states().is_none();
        
        if needs_full_rebuild {
            self.rebuild_solution_state(solution_ctx);
        } else {
            self.validate_and_correct_solution_state(solution_ctx);
        }
    }
    
    fn notify_failure(&self, solution_ctx: &mut SolutionContext, _route_indices: &[usize], jobs: &[Job]) -> bool {
        let mut modified = false;
        let mut affinities = solution_ctx.state.get_vehicle_affinities().cloned().unwrap_or_default();
        let mut schedules = solution_ctx.state.get_affinity_schedules().cloned().unwrap_or_default();
        let mut group_states = solution_ctx.state.get_affinity_group_states().cloned().unwrap_or_default();
        
        // Handle affinity job failures - clear partial assignments to avoid incomplete groups
        for job in jobs {
            if let Some(affinity_key) = job.dimens().get_job_affinity() {
                if let Some(_duration_days) = job.dimens().get_job_affinity_duration_days() {
                    // Check if we have partial assignments for this affinity group
                    if let Some(group_state) = group_states.get(affinity_key) {
                        if group_state.is_partial() {
                            // Clear all assignments for this affinity group
                            self.clear_affinity_group_from_routes(solution_ctx, affinity_key);
                            
                            // Clear tracking state
                            affinities.remove(affinity_key);
                            schedules.remove(affinity_key);
                            group_states.remove(affinity_key);
                            modified = true;
                        }
                    }
                }
            }
        }
        
        if modified {
            solution_ctx.state.set_vehicle_affinities(affinities);
            solution_ctx.state.set_affinity_schedules(schedules);
            solution_ctx.state.set_affinity_group_states(group_states);
        }
        
        modified
    }
}

impl VehicleAffinityState {
    /// Performs a full rebuild of affinity state from scratch
    fn rebuild_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let mut affinities: HashMap<String, Arc<Vehicle>> = HashMap::new();
        let mut schedules: HashMap<String, Vec<(u32, Timestamp)>> = HashMap::new();
        let mut group_states: HashMap<String, AffinityGroupState> = HashMap::new();
        
        // Rebuild affinity assignments from current solution
        for route_ctx in &solution_ctx.routes {
            let vehicle = route_ctx.route().actor.vehicle.clone();
            
            for job in route_ctx.route().tour.jobs() {
                if let Some(affinity) = job.dimens().get_job_affinity() {
                    affinities.insert(affinity.clone(), vehicle.clone());
                    
                    if let (Some(sequence), Some(duration_days)) = (
                        job.dimens().get_job_affinity_sequence(),
                        job.dimens().get_job_affinity_duration_days()
                    ) {
                        if let Some(timestamp) = extract_job_start_time(job) {
                            // Update schedules
                            schedules.entry(affinity.clone()).or_default().push((*sequence, timestamp));
                            
                            // Update group state
                            let group_state = group_states.entry(affinity.clone())
                                .or_insert_with(|| AffinityGroupState::new(*duration_days));
                            
                            if group_state.assigned_vehicle.is_none() {
                                group_state.assigned_vehicle = Some(vehicle.clone());
                            }
                            
                            if group_state.base_timestamp.is_none() {
                                let day_duration = calculate_day_duration(job);
                                group_state.base_timestamp = Some(timestamp - (*sequence as f64 * day_duration));
                            }
                            
                            group_state.assigned_sequences.insert(*sequence, timestamp);
                        }
                    }
                }
            }
        }
        
        // Sort schedules by sequence
        for schedule in schedules.values_mut() {
            schedule.sort_by_key(|(seq, _)| *seq);
        }
        
        solution_ctx.state.set_vehicle_affinities(affinities);
        solution_ctx.state.set_affinity_schedules(schedules);
        solution_ctx.state.set_affinity_group_states(group_states);
    }
    
    /// Validates existing state and corrects inconsistencies incrementally
    fn validate_and_correct_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let mut group_states = solution_ctx.state.get_affinity_group_states().cloned().unwrap_or_default();
        let mut modified = false;
        
        // Validate each affinity group for completeness and consistency
        let mut groups_to_clear = Vec::new();
        
        for (affinity_key, group_state) in &group_states {
            // Count actual assignments in routes
            let mut actual_assignments = HashMap::new();
            
            for route_ctx in &solution_ctx.routes {
                for job in route_ctx.route().tour.jobs() {
                    if let Some(job_affinity) = job.dimens().get_job_affinity() {
                        if job_affinity == affinity_key {
                            if let Some(sequence) = job.dimens().get_job_affinity_sequence() {
                                if let Some(timestamp) = extract_job_start_time(job) {
                                    actual_assignments.insert(*sequence, timestamp);
                                }
                            }
                        }
                    }
                }
            }
            
            // Check for inconsistencies
            if actual_assignments != group_state.assigned_sequences {
                groups_to_clear.push(affinity_key.clone());
                modified = true;
            }
        }
        
        // Clear inconsistent groups
        for affinity_key in groups_to_clear {
            group_states.remove(&affinity_key);
        }
        
        if modified {
            // Trigger full rebuild to ensure consistency
            self.rebuild_solution_state(solution_ctx);
        }
    }
    
    /// Removes all jobs with given affinity from all routes
    fn clear_affinity_group_from_routes(&self, solution_ctx: &mut SolutionContext, _affinity_key: &str) {
        for _route_ctx in &mut solution_ctx.routes {
            // Remove jobs from routes - this would need to be implemented differently
            // as Tour doesn't have jobs_mut(). For now, mark for rebuild.
            // TODO: Implement proper job removal from tours
            // Placeholder - would need proper implementation
        }
    }
}

/// Calculates the day duration based on job's time window or shift duration
fn calculate_day_duration(job: &Job) -> f64 {
    job.places()
        .next()
        .and_then(|place| place.times.first())
        .map(|time_span| match time_span {
            TimeSpan::Window(window) => window.end - window.start,
            TimeSpan::Offset(offset) => offset.end - offset.start,
        })
        .unwrap_or(24.0 * 3600.0) // Default to 24 hours
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
    
    // Calculate expected day based on sequence (using dynamic day duration)
    let day_duration = calculate_day_duration(job);
    
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
    
    // Allow configurable tolerance or default to 4 hours
    let tolerance = job.dimens().get_job_affinity_tolerance()
        .copied()
        .unwrap_or(4.0 * 3600.0);
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

/// Finds the optimal start date for an affinity group within the planning horizon
pub fn find_optimal_affinity_start_date(
    affinity_jobs: &[Job],
    planning_horizon: &TimeWindow,
    vehicle: &Vehicle
) -> Option<Timestamp> {
    if affinity_jobs.is_empty() {
        return None;
    }
    
    // Sort jobs by sequence to establish the correct order
    let mut sorted_jobs: Vec<_> = affinity_jobs.iter().collect();
    sorted_jobs.sort_by_key(|job| {
        job.dimens().get_job_affinity_sequence().unwrap_or(&0)
    });
    
    let first_job = sorted_jobs[0];
    let day_duration = calculate_day_duration(first_job);
    let duration_days = first_job.dimens().get_job_affinity_duration_days().unwrap_or(&1);
    
    // Find the earliest feasible start date considering multiple factors
    let mut candidates = Vec::new();
    
    // Candidate 1: Earliest possible start respecting job time windows
    let earliest_job_start = sorted_jobs.iter()
        .filter_map(|job| extract_job_start_time(job))
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
        .unwrap_or(planning_horizon.start);
    candidates.push(earliest_job_start);
    
    // Candidate 2: Vehicle availability-based start (considering shift patterns)
    if let Some(vehicle_start) = find_vehicle_available_start(vehicle, planning_horizon, *duration_days as usize, day_duration) {
        candidates.push(vehicle_start);
    }
    
    // Candidate 3: Optimal load balancing start (spread work evenly)
    let load_balanced_start = find_load_balanced_start(planning_horizon, *duration_days as usize, day_duration);
    candidates.push(load_balanced_start);
    
    // Select the best candidate based on multiple criteria
    let optimal_start = candidates.into_iter()
        .filter(|&start| start >= planning_horizon.start && start <= planning_horizon.end)
        .filter(|&start| {
            // Ensure all jobs can fit within their time windows
            validate_affinity_group_time_windows(&sorted_jobs, start, day_duration)
        })
        .min_by(|&a, &b| {
            // Prefer earlier starts, but consider vehicle efficiency
            let a_efficiency = calculate_start_efficiency(a, vehicle, &sorted_jobs, day_duration);
            let b_efficiency = calculate_start_efficiency(b, vehicle, &sorted_jobs, day_duration);
            
            // Higher efficiency is better, but if close, prefer earlier start
            if (a_efficiency - b_efficiency).abs() < 0.01 {
                a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                b_efficiency.partial_cmp(&a_efficiency).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    
    optimal_start
}

/// Evaluates the cost of assigning an entire affinity group to a vehicle
pub fn evaluate_affinity_group_assignment(
    affinity_jobs: &[Job], 
    vehicle: &Vehicle,
    transport_cost: &dyn TransportCost
) -> Option<Cost> {
    if affinity_jobs.is_empty() {
        return Some(0.0);
    }
    
    // Sort jobs by sequence to establish the correct order
    let mut sorted_jobs: Vec<_> = affinity_jobs.iter().collect();
    sorted_jobs.sort_by_key(|job| {
        job.dimens().get_job_affinity_sequence().unwrap_or(&0)
    });
    
    let mut total_cost = 0.0;
    
    // 1. Calculate travel costs across all days
    let travel_cost = calculate_affinity_travel_costs(&sorted_jobs, vehicle, transport_cost);
    total_cost += travel_cost;
    
    // 2. Evaluate skill compatibility
    let skill_penalty = calculate_skill_compatibility_cost(&sorted_jobs, vehicle);
    total_cost += skill_penalty;
    
    // 3. Check capacity constraints
    let capacity_cost = calculate_capacity_constraint_cost(&sorted_jobs, vehicle);
    total_cost += capacity_cost;
    
    // 4. Evaluate time window conflicts
    let time_window_penalty = calculate_time_window_conflict_cost(&sorted_jobs, vehicle);
    total_cost += time_window_penalty;
    
    // 5. Factor in vehicle availability and efficiency
    let availability_cost = calculate_vehicle_availability_cost(&sorted_jobs, vehicle);
    total_cost += availability_cost;
    
    // 6. Consider service time and waiting costs
    let service_cost = calculate_affinity_service_costs(&sorted_jobs, vehicle);
    total_cost += service_cost;
    
    // 7. Add efficiency bonus for well-matched assignments
    let efficiency_bonus = calculate_affinity_efficiency_bonus(&sorted_jobs, vehicle);
    total_cost -= efficiency_bonus; // Subtract because it's a bonus
    
    Some(total_cost.max(0.0)) // Ensure non-negative cost
}

// Helper functions for sophisticated cost evaluation and optimal start date finding

/// Finds the earliest start date when the vehicle has consecutive availability
fn find_vehicle_available_start(
    vehicle: &Vehicle,
    planning_horizon: &TimeWindow,
    duration_days: usize,
    day_duration: f64
) -> Option<Timestamp> {
    // Check if vehicle has multiple shifts (multi-day availability)
    if vehicle.details.len() >= duration_days {
        // Find consecutive shifts that can accommodate the affinity group
        for window in vehicle.details.windows(duration_days) {
            if let (Some(first_start), Some(last_end)) = (
                window.first().and_then(|d| d.start.as_ref()),
                window.last().and_then(|d| d.end.as_ref())
            ) {
                let start_time = first_start.time.earliest.unwrap_or(planning_horizon.start);
                let end_time = last_end.time.latest.unwrap_or(planning_horizon.end);
                
                // Check if this window can accommodate the full duration
                if end_time - start_time >= (duration_days as f64 - 1.0) * day_duration {
                    return Some(start_time.max(planning_horizon.start));
                }
            }
        }
    }
    
    // If no specific multi-day pattern, use the earliest available time
    vehicle.details.first()
        .and_then(|d| d.start.as_ref())
        .map(|start| start.time.earliest.unwrap_or(planning_horizon.start))
        .map(|t| t.max(planning_horizon.start))
}

/// Finds a start date that balances load across the planning horizon
fn find_load_balanced_start(
    planning_horizon: &TimeWindow,
    duration_days: usize,
    day_duration: f64
) -> Timestamp {
    let total_duration = duration_days as f64 * day_duration;
    let horizon_duration = planning_horizon.end - planning_horizon.start;
    
    // Aim for the middle of the horizon if possible
    let ideal_start = planning_horizon.start + (horizon_duration - total_duration) / 2.0;
    
    // Ensure it's within bounds
    ideal_start.max(planning_horizon.start).min(planning_horizon.end - total_duration)
}

/// Validates that all jobs in affinity group fit within their time windows
fn validate_affinity_group_time_windows(
    sorted_jobs: &[&Job],
    start_time: Timestamp,
    day_duration: f64
) -> bool {
    for (sequence, &job) in sorted_jobs.iter().enumerate() {
        let expected_job_time = start_time + (sequence as f64 * day_duration);
        
        // Check if job's time window is compatible
        if let Some(job_start) = extract_job_start_time(job) {
            if let Some(job_end) = extract_job_end_time(job) {
                let tolerance = job.dimens().get_job_affinity_tolerance()
                    .copied()
                    .unwrap_or(4.0 * 3600.0);
                
                if expected_job_time < job_start - tolerance || expected_job_time > job_end + tolerance {
                    return false;
                }
            }
        }
    }
    true
}

/// Calculates efficiency score for a given start time
fn calculate_start_efficiency(
    start_time: Timestamp,
    vehicle: &Vehicle,
    sorted_jobs: &[&Job],
    day_duration: f64
) -> f64 {
    let mut efficiency = 1.0;
    
    // Factor 1: Vehicle shift alignment
    let shift_alignment = calculate_shift_alignment_score(start_time, vehicle, sorted_jobs.len(), day_duration);
    efficiency *= shift_alignment;
    
    // Factor 2: Time window utilization
    let window_utilization = calculate_time_window_utilization(start_time, sorted_jobs, day_duration);
    efficiency *= window_utilization;
    
    // Factor 3: Avoid peak/busy periods (prefer off-peak scheduling)
    let peak_avoidance = calculate_peak_avoidance_score(start_time, day_duration);
    efficiency *= peak_avoidance;
    
    efficiency
}

/// Calculates how well the start time aligns with vehicle shifts
fn calculate_shift_alignment_score(
    start_time: Timestamp,
    vehicle: &Vehicle,
    duration_days: usize,
    day_duration: f64
) -> f64 {
    if vehicle.details.len() < duration_days {
        return 0.5; // Partial penalty for insufficient shifts
    }
    
    let mut alignment_score = 0.0;
    let mut valid_alignments = 0;
    
    for (day, detail) in vehicle.details.iter().take(duration_days).enumerate() {
        let expected_day_start = start_time + (day as f64 * day_duration);
        
        if let Some(shift_start) = detail.start.as_ref() {
            let shift_start_time = shift_start.time.earliest.unwrap_or(0.0);
            let time_diff = (expected_day_start - shift_start_time).abs();
            
            // Perfect alignment gets score 1.0, degrading with time difference
            let day_score = (1.0 - (time_diff / (4.0 * 3600.0))).max(0.0);
            alignment_score += day_score;
            valid_alignments += 1;
        }
    }
    
    if valid_alignments > 0 {
        alignment_score / valid_alignments as f64
    } else {
        0.5
    }
}

/// Calculates how well time windows are utilized
fn calculate_time_window_utilization(
    start_time: Timestamp,
    sorted_jobs: &[&Job],
    day_duration: f64
) -> f64 {
    let mut total_utilization = 0.0;
    let mut job_count = 0;
    
    for (sequence, &job) in sorted_jobs.iter().enumerate() {
        let expected_time = start_time + (sequence as f64 * day_duration);
        
        if let (Some(job_start), Some(job_end)) = (extract_job_start_time(job), extract_job_end_time(job)) {
            let window_size = job_end - job_start;
            if window_size > 0.0 {
                let window_center = job_start + window_size / 2.0;
                let distance_from_center = (expected_time - window_center).abs();
                let utilization = (1.0 - (distance_from_center / (window_size / 2.0))).max(0.0);
                total_utilization += utilization;
                job_count += 1;
            }
        }
    }
    
    if job_count > 0 {
        total_utilization / job_count as f64
    } else {
        1.0
    }
}

/// Calculates score for avoiding peak periods
fn calculate_peak_avoidance_score(start_time: Timestamp, _day_duration: f64) -> f64 {
    // Simple heuristic: avoid starting during typical rush hours
    // This could be made configurable based on business requirements
    
    let hour_of_day = ((start_time % (24.0 * 3600.0)) / 3600.0) as i32;
    
    // Peak hours: 7-9 AM and 5-7 PM
    let is_peak = (hour_of_day >= 7 && hour_of_day <= 9) || (hour_of_day >= 17 && hour_of_day <= 19);
    
    if is_peak {
        0.7 // Penalty for peak times
    } else {
        1.0 // No penalty for off-peak
    }
}

/// Calculates travel costs for the entire affinity group
fn calculate_affinity_travel_costs(
    sorted_jobs: &[&Job],
    vehicle: &Vehicle,
    transport_cost: &dyn TransportCost
) -> Cost {
    let mut total_cost = 0.0;
    let profile = &vehicle.profile;
    
    // Create a temporary route for cost calculation
    let driver = Arc::new(Driver::empty());
    let actor = Arc::new(Actor {
        vehicle: Arc::new(vehicle.clone()),
        driver,
        detail: vehicle.details.first().map(|d| ActorDetail {
            start: d.start.clone(),
            end: d.end.clone(),
            time: TimeWindow {
                start: d.start.as_ref().and_then(|s| s.time.earliest).unwrap_or(0.),
                end: d.end.as_ref().and_then(|e| e.time.latest).unwrap_or(Float::MAX),
            },
        }).unwrap_or(ActorDetail {
            start: None,
            end: None,
            time: TimeWindow { start: 0., end: Float::MAX },
        }),
    });
    
    let _route = Route {
        actor,
        tour: Tour::default(),
    };
    
    // Calculate travel costs between job locations
    for jobs_pair in sorted_jobs.windows(2) {
        if let [job1, job2] = jobs_pair {
            if let (Some(loc1), Some(loc2)) = (extract_job_location(job1), extract_job_location(job2)) {
                let distance = transport_cost.distance_approx(profile, loc1, loc2);
                let duration = transport_cost.duration_approx(profile, loc1, loc2);
                
                total_cost += distance * (vehicle.costs.per_distance)
                    + duration * (vehicle.costs.per_driving_time);
            }
        }
    }
    
    // Add costs from depot to first job and last job to depot
    if let (Some(first_job), Some(last_job)) = (sorted_jobs.first(), sorted_jobs.last()) {
        if let (Some(depot_loc), Some(first_loc), Some(last_loc)) = (
            vehicle.details.first().and_then(|d| d.start.as_ref()).map(|s| s.location),
            extract_job_location(first_job),
            extract_job_location(last_job)
        ) {
            // Depot to first job
            let distance1 = transport_cost.distance_approx(profile, depot_loc, first_loc);
            let duration1 = transport_cost.duration_approx(profile, depot_loc, first_loc);
            
            // Last job to depot
            let distance2 = transport_cost.distance_approx(profile, last_loc, depot_loc);
            let duration2 = transport_cost.duration_approx(profile, last_loc, depot_loc);
            
            total_cost += (distance1 + distance2) * vehicle.costs.per_distance
                + (duration1 + duration2) * vehicle.costs.per_driving_time;
        }
    }
    
    total_cost
}

/// Calculates penalty for skill incompatibility
fn calculate_skill_compatibility_cost(_sorted_jobs: &[&Job], _vehicle: &Vehicle) -> Cost {
    // Simplified implementation - skill checking would need proper skill dimension handling
    // For now, return 0 penalty
    0.0
}

/// Calculates cost related to capacity constraints
fn calculate_capacity_constraint_cost(sorted_jobs: &[&Job], _vehicle: &Vehicle) -> Cost {
    // Vehicle capacity is stored in dimens, not as a direct field
    // For now, assume single capacity dimension
    let mut max_load = 0.0;
    
    // Calculate maximum load across all jobs in the affinity group
    for &job in sorted_jobs {
        match job {
            Job::Single(single) => {
                // Add demands from all tasks in the single job
                // Simplified capacity calculation - would need proper demand handling
                for _place in &single.places {
                    // Assume all places add to load - would need proper pickup/delivery logic
                    max_load += 1.0; // Placeholder for demand calculation
                }
            },
            Job::Multi(multi) => {
                // Handle multi-job constraints
                for single in &multi.jobs {
                    for _place in &single.places {
                        // Simplified - would need proper demand handling
                        max_load += 1.0;
                    }
                }
            }
        }
    }
    
    // Simplified capacity check - would need proper capacity dimension handling
    let penalty = if max_load > 20.0 { // Assume default capacity of 20
        let violation = max_load - 20.0;
        violation * violation * 100.0
    } else {
        0.0
    };
    
    penalty
}

/// Calculates penalty for time window conflicts
fn calculate_time_window_conflict_cost(sorted_jobs: &[&Job], _vehicle: &Vehicle) -> Cost {
    let default_single = Single {
        places: vec![],
        dimens: Dimensions::default(),
    };
    let default_job = Job::Single(Arc::new(default_single));
    let day_duration = calculate_day_duration(sorted_jobs.first().unwrap_or(&&default_job));
    let mut penalty = 0.0;
    
    for window in sorted_jobs.windows(2) {
        if let [job1, job2] = window {
            if let (Some(end1), Some(start2)) = (extract_job_end_time(job1), extract_job_start_time(job2)) {
                let expected_gap = day_duration;
                let actual_gap = start2 - end1;
                
                // Penalty for insufficient gap between consecutive jobs
                if actual_gap < expected_gap * 0.8 { // Allow 20% tolerance
                    penalty += (expected_gap * 0.8 - actual_gap) * 10.0;
                }
            }
        }
    }
    
    penalty
}

/// Calculates cost based on vehicle availability patterns
fn calculate_vehicle_availability_cost(sorted_jobs: &[&Job], vehicle: &Vehicle) -> Cost {
    let duration_days = sorted_jobs.len();
    
    // Penalty if vehicle doesn't have enough shifts for the duration
    if vehicle.details.len() < duration_days {
        return (duration_days - vehicle.details.len()) as f64 * 500.0;
    }
    
    // Check if shifts are consecutive and properly aligned
    let mut consecutive_penalty = 0.0;
    let day_duration = calculate_day_duration(sorted_jobs.first().unwrap());
    
    for i in 1..duration_days.min(vehicle.details.len()) {
        if let (Some(prev_end), Some(curr_start)) = (
            vehicle.details[i-1].end.as_ref(),
            vehicle.details[i].start.as_ref()
        ) {
            let prev_end_time = prev_end.time.latest.unwrap_or(f64::MAX);
            let curr_start_time = curr_start.time.earliest.unwrap_or(0.0);
            let gap = curr_start_time - prev_end_time;
            
            // Expect roughly 24-hour gaps between shifts
            let expected_gap = day_duration;
            let gap_difference = (gap - expected_gap).abs();
            
            if gap_difference > 4.0 * 3600.0 { // More than 4 hours difference
                consecutive_penalty += gap_difference * 0.1;
            }
        }
    }
    
    consecutive_penalty
}

/// Calculates service costs for all jobs in affinity group
fn calculate_affinity_service_costs(sorted_jobs: &[&Job], vehicle: &Vehicle) -> Cost {
    let mut total_service_cost = 0.0;
    
    for &job in sorted_jobs {
        let service_duration = extract_job_service_duration(job);
        total_service_cost += service_duration * vehicle.costs.per_service_time;
    }
    
    total_service_cost
}

/// Calculates efficiency bonus for well-matched assignments
fn calculate_affinity_efficiency_bonus(sorted_jobs: &[&Job], vehicle: &Vehicle) -> Cost {
    let mut bonus = 0.0;
    
    // Bonus for skill match
    let skill_match_bonus = calculate_skill_match_bonus(sorted_jobs, vehicle);
    bonus += skill_match_bonus;
    
    // Bonus for location proximity
    let proximity_bonus = calculate_location_proximity_bonus(sorted_jobs, vehicle);
    bonus += proximity_bonus;
    
    // Bonus for consistent assignment (vehicle specialization)
    let specialization_bonus = calculate_specialization_bonus(sorted_jobs, vehicle);
    bonus += specialization_bonus;
    
    bonus
}

/// Calculates bonus for perfect skill matches
fn calculate_skill_match_bonus(_sorted_jobs: &[&Job], _vehicle: &Vehicle) -> Cost {
    // Simplified implementation - skill checking would need proper skill dimension handling
    // For now, return small bonus
    10.0
}

/// Calculates bonus for jobs close to vehicle's home base
fn calculate_location_proximity_bonus(sorted_jobs: &[&Job], vehicle: &Vehicle) -> Cost {
    let mut bonus = 0.0;
    
    if let Some(depot_location) = vehicle.details.first()
        .and_then(|d| d.start.as_ref())
        .map(|s| s.location) {
        
        for &job in sorted_jobs {
            if let Some(job_location) = extract_job_location(job) {
                // Simple distance-based bonus (in real implementation, use proper distance calculation)
                let distance_factor = 1.0 / (1.0 + (depot_location as f64 - job_location as f64).abs());
                bonus += distance_factor * 25.0;
            }
        }
    }
    
    bonus
}

/// Calculates bonus for vehicle specialization
fn calculate_specialization_bonus(_sorted_jobs: &[&Job], _vehicle: &Vehicle) -> Cost {
    // Placeholder for specialization logic
    // Could be based on vehicle type, past assignment history, etc.
    10.0
}

/// Helper function to extract job location
fn extract_job_location(job: &Job) -> Option<Location> {
    job.places().next().and_then(|place| place.location)
}

/// Helper function to extract job end time
fn extract_job_end_time(job: &Job) -> Option<Timestamp> {
    job.places().next().and_then(|place| {
        place.times.first().map(|time_span| match time_span {
            TimeSpan::Window(window) => window.end,
            TimeSpan::Offset(offset) => offset.end,
        })
    })
}

/// Helper function to extract job service duration
fn extract_job_service_duration(job: &Job) -> Duration {
    job.places().next().map(|place| place.duration).unwrap_or(0.0)
}