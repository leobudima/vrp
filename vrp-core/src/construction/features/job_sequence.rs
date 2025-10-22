//! # Job Sequence Feature
//!
//! A feature to ensure jobs with the same sequence key are executed in strict order
//! with configurable time/shift gaps between them.
//!
//! ## Overview
//!
//! The job sequence feature enables modeling workflows that require strict temporal ordering,
//! such as multi-day installations, phased deliveries, or progressive service tasks.
//!
//! ## Key Capabilities
//!
//! - **Strict Ordering**: Jobs must be executed in numerical order (0, 1, 2, ...)
//! - **Flexible Gap Constraints**: Configure minimum and maximum gaps between consecutive jobs
//! - **Hybrid Validation**:
//!   - **Shift-based**: When jobs are on the same vehicle (counts shifts)
//!   - **Calendar-based**: When jobs are on different vehicles (counts days with tolerance)
//! - **All-or-Nothing Assignment**: All jobs in a sequence must be assigned or none
//! - **Configurable Tolerance**: Adjust calendar-based validation tolerance for real-world flexibility
//!
//! ## Usage Example
//!
//! ```rust
//! use vrp_core::construction::features::{create_job_sequence_feature, create_job_sequence_feature_with_config, JobSequenceConfig};
//! use vrp_core::models::ViolationCode;
//!
//! // Create with default configuration
//! let feature = create_job_sequence_feature("job_sequence", ViolationCode(1)).unwrap();
//!
//! // Or customize configuration
//! let config = JobSequenceConfig {
//!     calendar_tolerance_days: 0.5,  // 12 hours tolerance
//!     penalty_per_missing_job: 200000.0,  // Higher penalty for incomplete sequences
//!     max_reasonable_gap: 180,  // Maximum 6 months between jobs
//! };
//! let feature = create_job_sequence_feature_with_config("job_sequence", ViolationCode(1), config).unwrap();
//! ```
//!
//! ## When to Use Sequence vs Relations
//!
//! - **Use Sequence when**:
//!   - Jobs must be executed in strict numerical order (0 → 1 → 2)
//!   - You need flexible gap constraints between jobs
//!   - Jobs may be assigned to different vehicles on different days
//!   - You want all-or-nothing enforcement (complete sequence or fail)
//!
//! - **Use Relations when**:
//!   - Jobs must be on the same route/vehicle
//!   - You need flexible ordering (any, sequence, or strict)
//!   - You need position constraints (departure, arrival)
//!   - Order doesn't need to be strictly numerical
//!
//! ## Hybrid Validation Strategy
//!
//! The feature uses intelligent validation based on vehicle assignment:
//!
//! ### Same Vehicle (Shift-Based)
//! When consecutive jobs in a sequence are assigned to the same vehicle, the feature
//! counts **shifts** between them. For example, with `days_between_min=1, days_between_max=1`:
//! - Job 0 on Vehicle A, Shift 0
//! - Job 1 on Vehicle A, Shift 1 ✓ (gap = 1 shift)
//! - Job 1 on Vehicle A, Shift 0 ✗ (gap = 0 shifts)
//! - Job 1 on Vehicle A, Shift 2 ✗ (gap = 2 shifts)
//!
//! ### Different Vehicles (Calendar-Based)
//! When consecutive jobs are on different vehicles, the feature uses **calendar days**
//! with a configurable tolerance (default 6 hours):
//! - Job 0 at Day 0, 09:00
//! - Job 1 at Day 1, 10:00 ✓ (gap ≈ 25 hours, within tolerance of 24±6 hours)
//! - Job 1 at Day 2, 09:00 ✗ (gap = 48 hours, outside tolerance)
//!
//! ## Jobs Without Time Windows
//!
//! When jobs in a sequence don't have explicit time windows, the feature uses shift start times
//! as a proxy for scheduled times. This enables gap validation to work correctly in all scenarios:
//!
//! - **Same vehicle**: Uses shift-based counting (counts shifts between jobs)
//! - **Different vehicles**: Uses calendar-based validation with shift start times
//!
//! This approach ensures gap constraints are enforced even when jobs don't have explicit time
//! windows, making the feature flexible for real-world use cases where time windows may not be
//! known in advance.
//!
//! ## Feature Interactions
//!
//! The sequence feature works well with other features:
//!
//! - **same_assignee_key**: Ensures all jobs in sequence go to the same driver across
//!   multiple routes/shifts (useful for technician-specific multi-day tasks)
//! - **vehicle_affinity**: Can bias sequence jobs toward specific vehicles
//! - **skills**: Jobs can still require specific skills
//! - **time_windows**: Timing constraints work alongside sequence constraints
//!
//! ## Configuration Options
//!
//! See [`JobSequenceConfig`] for detailed configuration options:
//! - `calendar_tolerance_days`: Tolerance for calendar-based gap validation
//! - `penalty_per_missing_job`: Objective penalty for incomplete sequences
//! - `max_reasonable_gap`: Maximum allowed gap (sanity check)
//!
//! ## Performance Considerations
//!
//! - State reconstruction is O(routes × jobs_per_route)
//! - Constraint evaluation is O(1) for most checks
//! - Large sequences (50+ jobs) are supported but may impact solver performance
//!
//! ## Error Handling
//!
//! The feature validates input at multiple levels:
//! 1. **API Level** (pragmatic format): Validates sequence completeness, consistent parameters
//! 2. **Core Level**: Validates min ≤ max, reasonable bounds, required dimensions
//! 3. **Runtime**: Prevents duplicate orders, out-of-order assignment, gap violations

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/job_sequence_test.rs"]
mod job_sequence_test;

use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

custom_dimension!(pub JobSequenceKey typeof String);
custom_dimension!(pub JobSequenceOrder typeof u32);
custom_dimension!(pub JobSequenceDaysBetweenMin typeof u32);
custom_dimension!(pub JobSequenceDaysBetweenMax typeof u32);
custom_solution_state!(SequenceGroupStates typeof HashMap<String, SequenceGroupState>);

/// Configuration for job sequence feature
#[derive(Debug, Clone)]
pub struct JobSequenceConfig {
    /// Tolerance for calendar-based gap validation (in days).
    /// Default: 0.25 days (6 hours)
    pub calendar_tolerance_days: f64,
    /// Penalty per missing job in a partial sequence.
    /// Default: 100000.0
    pub penalty_per_missing_job: f64,
    /// Maximum reasonable gap value (sanity check).
    /// Default: 365 days
    pub max_reasonable_gap: u32,
}

impl Default for JobSequenceConfig {
    fn default() -> Self {
        Self {
            calendar_tolerance_days: 0.25, // 6 hours
            penalty_per_missing_job: 100000.0,
            max_reasonable_gap: 365,
        }
    }
}

/// State tracking for a sequence group
#[derive(Debug, Clone)]
pub struct SequenceGroupState {
    /// Expected total size of sequence (e.g., 3 for orders 0,1,2)
    pub expected_size: u32,
    /// Current assignments: order -> assignment details
    pub assignments: HashMap<u32, SequenceJobAssignment>,
    /// Expected orders that must all be assigned
    pub expected_orders: HashSet<u32>,
}

/// Details of a job assignment in a sequence
#[derive(Debug, Clone)]
pub struct SequenceJobAssignment {
    /// Scheduled time (None if job has no time window)
    pub scheduled_time: Option<Timestamp>,
    pub order: u32,
    /// For shift-based validation (when same vehicle)
    pub vehicle: Arc<Vehicle>,
    pub shift_index: usize,
}

impl SequenceGroupState {
    fn new(expected_size: u32) -> Self {
        Self {
            expected_size,
            assignments: HashMap::new(),
            expected_orders: (0..expected_size).collect(),
        }
    }

    fn is_complete(&self) -> bool {
        self.assignments.len() == self.expected_size as usize
            && self.expected_orders.iter().all(|order| self.assignments.contains_key(order))
    }

    fn is_partial(&self) -> bool {
        !self.assignments.is_empty() && !self.is_complete()
    }
}

impl SequenceJobAssignment {
    /// Validates gap from this assignment to next using hybrid approach
    /// Uses shift-based validation for same vehicle, calendar-based for different vehicles
    fn validate_gap_to(
        &self,
        next_vehicle: &Arc<Vehicle>,
        next_shift_index: usize,
        next_time: Timestamp,
        min_gap: u32,
        max_gap: u32,
        tolerance: f64,
    ) -> bool {
        if Arc::ptr_eq(&self.vehicle, next_vehicle) {
            // Same vehicle: use shift-based validation
            let shift_gap = next_shift_index.saturating_sub(self.shift_index);
            return shift_gap >= min_gap as usize && shift_gap <= max_gap as usize;
        }

        // Different vehicles: use calendar-based validation
        // Use self.scheduled_time if available, otherwise use shift start time
        let self_time = self.scheduled_time.unwrap_or(self.vehicle.details[self.shift_index]
            .start.as_ref().and_then(|s| s.time.earliest).unwrap_or(0.0));
        let time_gap_days = (next_time - self_time) / (24.0 * 3600.0);
        time_gap_days >= (min_gap as f64 - tolerance) && time_gap_days <= (max_gap as f64 + tolerance)
    }

    /// Validates gap from previous to this assignment (reverse direction)
    /// Uses shift-based validation for same vehicle, calendar-based for different vehicles
    fn validate_gap_from(
        &self,
        prev_vehicle: &Arc<Vehicle>,
        prev_shift_index: usize,
        prev_time: Timestamp,
        min_gap: u32,
        max_gap: u32,
        tolerance: f64,
    ) -> bool {
        if Arc::ptr_eq(&self.vehicle, prev_vehicle) {
            // Same vehicle: use shift-based validation
            let shift_gap = self.shift_index.saturating_sub(prev_shift_index);
            return shift_gap >= min_gap as usize && shift_gap <= max_gap as usize;
        }

        // Different vehicles: use calendar-based validation
        // Use self.scheduled_time if available, otherwise use shift start time
        let self_time = self.scheduled_time.unwrap_or(self.vehicle.details[self.shift_index]
            .start.as_ref().and_then(|s| s.time.earliest).unwrap_or(0.0));
        let time_gap_days = (self_time - prev_time) / (24.0 * 3600.0);
        time_gap_days >= (min_gap as f64 - tolerance) && time_gap_days <= (max_gap as f64 + tolerance)
    }
}

/// Creates a job sequence feature with both hard constraint and soft objective using default configuration
pub fn create_job_sequence_feature(name: &str, code: ViolationCode) -> Result<Feature, GenericError> {
    create_job_sequence_feature_with_config(name, code, JobSequenceConfig::default())
}

/// Creates a job sequence feature with custom configuration
pub fn create_job_sequence_feature_with_config(
    name: &str,
    code: ViolationCode,
    config: JobSequenceConfig,
) -> Result<Feature, GenericError> {
    let config = Arc::new(config);
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(JobSequenceConstraint { code, config: config.clone() })
        .with_objective(JobSequenceObjective { config: config.clone() })
        .with_state(JobSequenceState { config })
        .build()
}

struct JobSequenceConstraint {
    code: ViolationCode,
    config: Arc<JobSequenceConfig>,
}

impl FeatureConstraint for JobSequenceConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { solution_ctx, route_ctx, job } => {
                job.dimens().get_job_sequence_key().and_then(|seq_key| {
                    // Validate input consistency FIRST (before checking order exists)
                    if let Some(violation) = self.validate_sequence_input(job) {
                        return Some(violation);
                    }

                    let order = job.dimens().get_job_sequence_order()?;
                    let current_vehicle = &route_ctx.route().actor.vehicle;
                    let current_shift_index = get_shift_index(&route_ctx.route().actor);

                    // Get or create group states
                    let group_states = solution_ctx.state.get_sequence_group_states();

                    // If no state exists for this sequence group yet, we need to check if this is the first job
                    // Only order 0 can start a new sequence (strict ordering)
                    if group_states.is_none() || group_states.and_then(|gs| gs.get(seq_key)).is_none() {
                        if *order != 0 {
                            // Cannot start a sequence with order > 0
                            return ConstraintViolation::fail(self.code);
                        }
                        // Order 0 starting a new sequence is always allowed
                        return None;
                    }

                    let group_state = group_states.unwrap().get(seq_key).unwrap();

                    // Check 1: Order uniqueness
                    if group_state.assignments.contains_key(order) {
                        return ConstraintViolation::fail(self.code);
                    }

                    // Check 2: Order must be in expected set
                    if !group_state.expected_orders.contains(order) {
                        return ConstraintViolation::fail(self.code);
                    }

                    // Check 3: Previous order must be assigned (strict ordering)
                    if *order > 0 && !group_state.assignments.contains_key(&(order - 1)) {
                        return ConstraintViolation::fail(self.code);
                    }

                    // Check 4: Timing constraints
                    // Use shift start time as fallback for jobs without time windows
                    let scheduled_time = get_scheduled_time_for_evaluation(route_ctx, job);
                    let min_gap = job.dimens().get_job_sequence_days_between_min().copied().unwrap_or(1);
                    let max_gap = job.dimens().get_job_sequence_days_between_max().copied().unwrap_or(1);

                    // Validate against previous order
                    if *order > 0 {
                        if let Some(prev) = group_state.assignments.get(&(order - 1)) {
                            if !prev.validate_gap_to(
                                current_vehicle,
                                current_shift_index,
                                scheduled_time,
                                min_gap,
                                max_gap,
                                self.config.calendar_tolerance_days,
                            ) {
                                return ConstraintViolation::fail(self.code);
                            }
                        }
                    }

                    // Validate against next order (if already assigned)
                    if let Some(next) = group_state.assignments.get(&(order + 1)) {
                        if !next.validate_gap_from(
                            current_vehicle,
                            current_shift_index,
                            scheduled_time,
                            min_gap,
                            max_gap,
                            self.config.calendar_tolerance_days,
                        ) {
                            return ConstraintViolation::fail(self.code);
                        }
                    }

                    None
                })
            }
            MoveContext::Activity { .. } => None,
        }
    }

    fn merge(&self, source: Job, candidate: Job) -> Result<Job, ViolationCode> {
        match (source.dimens().get_job_sequence_key(), candidate.dimens().get_job_sequence_key()) {
            (None, None) => Ok(source),
            (Some(s_key), Some(c_key)) if s_key == c_key => {
                // Ensure different orders
                let s_order = source.dimens().get_job_sequence_order();
                let c_order = candidate.dimens().get_job_sequence_order();

                if s_order == c_order {
                    return Err(self.code);
                }

                // Ensure consistent gap parameters
                let s_min = source.dimens().get_job_sequence_days_between_min();
                let c_min = candidate.dimens().get_job_sequence_days_between_min();
                let s_max = source.dimens().get_job_sequence_days_between_max();
                let c_max = candidate.dimens().get_job_sequence_days_between_max();

                if s_min != c_min || s_max != c_max {
                    return Err(self.code);
                }

                Ok(source)
            }
            _ => Err(self.code),
        }
    }
}

impl JobSequenceConstraint {
    fn validate_sequence_input(&self, job: &Job) -> Option<ConstraintViolation> {
        if job.dimens().get_job_sequence_key().is_some() {
            // Order must be specified
            if job.dimens().get_job_sequence_order().is_none() {
                return ConstraintViolation::fail(self.code);
            }

            let min = job.dimens().get_job_sequence_days_between_min().copied().unwrap_or(1);
            let max = job.dimens().get_job_sequence_days_between_max().copied().unwrap_or(1);

            // Validate min <= max
            if min > max {
                return ConstraintViolation::fail(self.code);
            }

            // Validate reasonable bounds
            if max > self.config.max_reasonable_gap {
                // Sanity check based on configuration
                return ConstraintViolation::fail(self.code);
            }
        }
        None
    }
}

struct JobSequenceObjective {
    config: Arc<JobSequenceConfig>,
}

impl FeatureObjective for JobSequenceObjective {
    fn fitness(&self, solution: &InsertionContext) -> Cost {
        if let Some(group_states) = solution.solution.state.get_sequence_group_states() {
            group_states
                .values()
                .filter(|gs| gs.is_partial())
                .map(|gs| {
                    let missing = gs.expected_size - gs.assignments.len() as u32;
                    // High penalty per missing job to enforce all-or-nothing
                    missing as f64 * self.config.penalty_per_missing_job
                })
                .sum()
        } else {
            0.0
        }
    }

    fn estimate(&self, move_ctx: &MoveContext<'_>) -> Cost {
        // Encourage completing sequences
        if let MoveContext::Route { solution_ctx, job, .. } = move_ctx {
            if let Some(seq_key) = job.dimens().get_job_sequence_key() {
                if let Some(group_states) = solution_ctx.state.get_sequence_group_states() {
                    if let Some(gs) = group_states.get(seq_key) {
                        let current_count = gs.assignments.len() as u32;
                        let will_count = current_count + 1;

                        if will_count == gs.expected_size {
                            // Completing sequence: big reward
                            return -(gs.expected_size as f64 * self.config.penalty_per_missing_job);
                        } else if current_count > 0 {
                            // Adding to partial sequence: small reward
                            return -(self.config.penalty_per_missing_job / 10.0);
                        }
                    }
                }
            }
        }
        0.0
    }
}

struct JobSequenceState {
    config: Arc<JobSequenceConfig>,
}

impl FeatureState for JobSequenceState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, job: &Job) {
        if let Some(seq_key) = job.dimens().get_job_sequence_key() {
            if let Some(order) = job.dimens().get_job_sequence_order() {
                let route_ctx = solution_ctx.routes.get(route_index).unwrap();
                let actor = &route_ctx.route().actor;
                let vehicle = actor.vehicle.clone();
                let shift_index = get_shift_index(actor);

                let mut group_states =
                    solution_ctx.state.get_sequence_group_states().cloned().unwrap_or_default();

                let sequence_sizes = detect_sequence_sizes_from_context(solution_ctx);
                let expected_size = sequence_sizes.get(seq_key).copied().unwrap_or(1);

                let group_state =
                    group_states.entry(seq_key.clone()).or_insert_with(|| SequenceGroupState::new(expected_size));

                // Get scheduled time (uses shift start time as fallback for jobs without time windows)
                let scheduled_time = Some(get_scheduled_time_for_evaluation(route_ctx, job));

                // Always track the job
                group_state.assignments.insert(
                    *order,
                    SequenceJobAssignment { scheduled_time, order: *order, vehicle, shift_index },
                );

                solution_ctx.state.set_sequence_group_states(group_states);
            }
        }
    }

    fn accept_route_state(&self, _: &mut RouteContext) {}

    fn accept_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let sequence_sizes = detect_sequence_sizes_from_context(solution_ctx);
        let mut group_states: HashMap<String, SequenceGroupState> = HashMap::new();

        // Collect assignments from routes
        for route_ctx in solution_ctx.routes.iter() {
            let actor = &route_ctx.route().actor;
            let vehicle = actor.vehicle.clone();
            let shift_index = get_shift_index(actor);

            for job in route_ctx.route().tour.jobs() {
                if let Some(seq_key) = job.dimens().get_job_sequence_key() {
                    if let Some(order) = job.dimens().get_job_sequence_order() {
                        let expected_size = sequence_sizes.get(seq_key).copied().unwrap_or(1);

                        let group_state = group_states
                            .entry(seq_key.clone())
                            .or_insert_with(|| SequenceGroupState::new(expected_size));

                        // Get scheduled time (uses shift start time as fallback for jobs without time windows)
                        let scheduled_time = Some(get_scheduled_time_for_evaluation(route_ctx, job));

                        // Always track the job
                        group_state.assignments.insert(
                            *order,
                            SequenceJobAssignment {
                                scheduled_time,
                                order: *order,
                                vehicle: vehicle.clone(),
                                shift_index,
                            },
                        );
                    }
                }
            }
        }

        solution_ctx.state.set_sequence_group_states(group_states);
    }
}

/// Detects expected sequence sizes by scanning all jobs in the solution context
fn detect_sequence_sizes_from_context(solution_ctx: &SolutionContext) -> HashMap<String, u32> {
    let mut sizes = HashMap::new();

    // Scan all jobs (required + ignored + in routes)
    let all_jobs = solution_ctx
        .required
        .iter()
        .chain(solution_ctx.ignored.iter())
        .chain(solution_ctx.routes.iter().flat_map(|rc| rc.route().tour.jobs()));

    for job in all_jobs {
        if let Some(seq_key) = job.dimens().get_job_sequence_key() {
            if let Some(order) = job.dimens().get_job_sequence_order() {
                let max_order = sizes.entry(seq_key.clone()).or_insert(0);
                *max_order = (*max_order).max(*order);
            }
        }
    }

    // Convert max_order to size (0-indexed, so add 1)
    sizes.into_iter().map(|(k, max)| (k, max + 1)).collect()
}

/// Extracts the shift index for an actor by finding which vehicle detail matches
///
/// Uses multiple strategies to robustly identify the shift:
/// 1. Exact match on start time (within 1 second tolerance)
/// 2. Fallback to first detail if no match found
///
/// Note: The tolerance of 1 second handles floating-point precision issues
/// while being strict enough to distinguish between different shifts.
fn get_shift_index(actor: &Actor) -> usize {
    let detail_start_time = actor.detail.time.start;
    let detail_end_time = actor.detail.time.end;

    // Use a small epsilon for floating-point comparison to handle precision issues
    const TIME_EPSILON: f64 = 1.0; // 1 second tolerance

    actor.vehicle.details
        .iter()
        .position(|detail| {
            // Match based on start time
            let start_matches = detail.start.as_ref()
                .and_then(|s| s.time.earliest)
                .map(|t| (t - detail_start_time).abs() < TIME_EPSILON)
                .unwrap_or(false);

            // Additional validation: check end time if available for extra robustness
            let end_matches = if start_matches {
                detail.end.as_ref()
                    .and_then(|e| e.time.latest)
                    .map(|t| (t - detail_end_time).abs() < TIME_EPSILON)
                    .unwrap_or(true) // If no end time specified, accept the match
            } else {
                false
            };

            start_matches && end_matches
        })
        .unwrap_or_else(|| {
            // Fallback: if no exact match found, return 0 (first shift)
            // This ensures we always have a valid shift index
            #[cfg(debug_assertions)]
            {
                eprintln!(
                    "WARNING: Failed to match shift index for actor with start time {} and end time {}. \
                     Falling back to shift index 0. This may indicate a configuration issue.",
                    detail_start_time, detail_end_time
                );
            }
            0
        })
}

fn extract_job_start_time(job: &Job) -> Option<Timestamp> {
    job.places().next().and_then(|place| {
        place.times.first().map(|time_span| match time_span {
            TimeSpan::Window(window) => window.start,
            TimeSpan::Offset(offset) => offset.start,
        })
    })
}

/// Extracts the actual scheduled departure time for a job from the route's activities.
/// This captures the real schedule even when jobs don't have explicit time windows.
///
/// Returns the departure time of the first activity associated with the job,
/// or None if the job hasn't been scheduled in the route yet.
fn extract_scheduled_time_from_route(route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
    route_ctx.route().tour.job_activities(job).next().map(|activity| activity.schedule.departure)
}

/// Extracts the shift start time from an actor's detail.
/// This is used as a proxy for scheduled time when jobs don't have explicit time windows.
fn extract_shift_start_time(actor: &Actor) -> Timestamp {
    actor.detail.time.start
}

/// Gets the scheduled time for a job during constraint evaluation.
/// Priority order:
/// 1. Explicit time window from job definition
/// 2. Actual scheduled time from route (if job already inserted)
/// 3. Shift start time as fallback (for jobs without time windows)
fn get_scheduled_time_for_evaluation(route_ctx: &RouteContext, job: &Job) -> Timestamp {
    extract_job_start_time(job)
        .or_else(|| extract_scheduled_time_from_route(route_ctx, job))
        .unwrap_or_else(|| extract_shift_start_time(&route_ctx.route().actor))
}
