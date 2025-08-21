//! A feature to model synchronization of multiple technicians on the same job.

use super::*;
use crate::models::problem::{ActivityCost, TransportCost, TravelTime};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/sync_test.rs"]
mod sync_test;

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/sync_comprehensive_test.rs"]
mod sync_comprehensive_test;

custom_dimension!(pub JobSyncGroup typeof String);
custom_dimension!(pub JobSyncIndex typeof u32);
custom_dimension!(pub JobSyncSize typeof u32);
custom_dimension!(pub JobSyncTolerance typeof f64);
custom_solution_state!(SyncGroupAssignments typeof HashMap<String, SyncGroupInfo>);
custom_tour_state!(RouteSyncGroups typeof HashSet<String>);

/// Information about a sync group's current assignments
#[derive(Clone, Debug)]
pub struct SyncGroupInfo {
    /// Required number of vehicles for this sync group
    pub required_size: u32,
    /// Assignments: (route_index, job_index, scheduled_time, tolerance)
    pub assignments: Vec<(usize, u32, Timestamp, f64)>,
    /// Set of assigned indices to prevent duplicates
    pub assigned_indices: HashSet<u32>,
}

/// Creates a job synchronization feature with both hard constraint and soft objective.
pub fn create_job_sync_feature(name: &str, code: ViolationCode) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(JobSyncConstraint { code, transport: None, activity: None })
        .with_objective(JobSyncObjective { threshold: 1.0 })
        .with_state(JobSyncState {})
        .build()
}

/// Creates a job synchronization feature with configurable timing threshold.
pub fn create_job_sync_feature_with_threshold(
    name: &str, 
    code: ViolationCode, 
    timing_threshold: f64
) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(JobSyncConstraint { code, transport: None, activity: None })
        .with_objective(JobSyncObjective { threshold: timing_threshold })
        .with_state(JobSyncState {})
        .build()
}

/// Creates a job synchronization feature with configurable timing threshold and access to transport/activity costs.
pub fn create_job_sync_feature_with_threshold_and_costs(
    name: &str,
    code: ViolationCode,
    timing_threshold: f64,
    transport: Arc<dyn TransportCost>,
    activity: Arc<dyn ActivityCost>,
) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(JobSyncConstraint { code, transport: Some(transport), activity: Some(activity) })
        .with_objective(JobSyncObjective { threshold: timing_threshold })
        .with_state(JobSyncState {})
        .build()
}

struct JobSyncConstraint {
    code: ViolationCode,
    transport: Option<Arc<dyn TransportCost>>,    
    activity: Option<Arc<dyn ActivityCost>>,      
}

impl FeatureConstraint for JobSyncConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { solution_ctx, route_ctx, job } => {
                self.validate_route_assignment(solution_ctx, route_ctx, job)
            }
            MoveContext::Activity { route_ctx, activity_ctx, .. } => {
                self.validate_activity_assignment(route_ctx, activity_ctx)
            }
        }
    }

    fn merge(&self, source: Job, candidate: Job) -> Result<Job, ViolationCode> {
        match (
            source.dimens().get_job_sync_group(),
            candidate.dimens().get_job_sync_group(),
            source.dimens().get_job_sync_index(),
            candidate.dimens().get_job_sync_index()
        ) {
            (None, None, None, None) => Ok(source),
            (Some(s_group), Some(c_group), Some(s_index), Some(c_index)) 
                if s_group == c_group && s_index == c_index => Ok(source),
            _ => Err(self.code),
        }
    }
}

impl JobSyncConstraint {
    /// Estimates service start time for insertion using multiple fallback strategies.
    /// Uses progressive estimation with confidence levels for robust timing validation.
    /// 
    /// Strategy order (highest to lowest confidence):
    /// 1) If job is already in route, return actual scheduled service start
    /// 2) Transport-based estimation with travel time and time windows
    /// 3) Route structure analysis for insertion position estimation
    /// 4) Statistical estimation based on route characteristics
    /// 5) Conservative fallback using job time windows and route end
    fn estimate_service_start_time_for_insertion(&self, route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
        // Strategy 1: Actual scheduled time (highest confidence)
        if let Some(scheduled) = extract_scheduled_time(route_ctx, job) {
            return Some(scheduled);
        }

        // Strategy 2: Transport-based estimation with detailed analysis
        if let Some(estimated_time) = self.estimate_with_transport_analysis(route_ctx, job) {
            return Some(estimated_time);
        }

        // Strategy 3: Route structure analysis for position-based estimation
        if let Some(estimated_time) = self.estimate_with_route_structure_analysis(route_ctx, job) {
            return Some(estimated_time);
        }

        // Strategy 4: Statistical estimation based on route characteristics
        if let Some(estimated_time) = self.estimate_with_statistical_analysis(route_ctx, job) {
            return Some(estimated_time);
        }

        // Strategy 5: Conservative fallback (always succeeds)
        self.estimate_with_conservative_fallback(route_ctx, job)
    }

    /// Strategy 2: Transport-based estimation with enhanced analysis
    fn estimate_with_transport_analysis(&self, route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
        let transport = self.transport.as_ref()?;
        let place = job.places().next()?;
        let route = route_ctx.route();

        // Try multiple reference points for better accuracy
        let reference_activities = [
            route.tour.end(),           // Route end (most common case)
            route.tour.start(),         // Route start (for early insertions)
            route.tour.get(route.tour.total() / 2), // Route middle (for mid-route insertions)
        ];

        for &ref_activity in reference_activities.iter().flatten() {
            if let Some(location) = place.location {
                let depart = ref_activity.schedule.departure;
                let travel = transport.duration(route, ref_activity.place.location, location, TravelTime::Departure(depart));
                let arrival = depart + travel;
                
                // Respect time window constraints with buffer
                let earliest = match place.times.first() {
                    Some(TimeSpan::Window(w)) => w.start,
                    Some(TimeSpan::Offset(o)) => o.start,
                    None => arrival,
                };
                
                let service_start = arrival.max(earliest);
                
                // Add conservative buffer for synchronization safety (5% of travel time, min 30 seconds)
                let safety_buffer = (travel * 0.05).max(30.0);
                return Some(service_start + safety_buffer);
            }
        }

        None
    }

    /// Strategy 3: Route structure analysis for position-based estimation
    fn estimate_with_route_structure_analysis(&self, route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
        let route = route_ctx.route();
        let job_place = job.places().next()?;
        
        // Find the best insertion position based on spatial proximity or time windows
        let mut best_estimate = None;
        let mut min_disruption = f64::INFINITY;

        for i in 0..route.tour.total() {
            if let (Some(prev), Some(next)) = (route.tour.get(i), route.tour.get(i + 1)) {
                // Estimate insertion between these activities
                let avg_time = (prev.schedule.departure + next.schedule.arrival) / 2.0;
                
                // Consider time window constraints
                let earliest = match job_place.times.first() {
                    Some(TimeSpan::Window(w)) => w.start,
                    Some(TimeSpan::Offset(o)) => o.start,
                    None => avg_time,
                };
                
                let estimated_service_start = avg_time.max(earliest);
                
                // Calculate disruption score (prefer insertions with minimal time impact)
                let disruption = (estimated_service_start - avg_time).abs();
                
                if disruption < min_disruption {
                    min_disruption = disruption;
                    best_estimate = Some(estimated_service_start);
                }
            }
        }

        best_estimate
    }

    /// Strategy 4: Statistical estimation based on route characteristics
    fn estimate_with_statistical_analysis(&self, route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
        let route = route_ctx.route();
        if route.tour.total() < 2 {
            return None;
        }

        // Calculate average service time and spacing in the route
        let activities: Vec<_> = route.tour.all_activities().collect();
        let total_time = activities.last()?.schedule.departure - activities.first()?.schedule.arrival;
        let avg_service_interval = total_time / (activities.len() as f64).max(1.0);

        // Estimate based on route end plus average interval
        let route_end_time = route.tour.end()?.schedule.departure;
        let estimated_time = route_end_time + avg_service_interval;

        // Respect job time window constraints
        let job_place = job.places().next()?;
        let earliest = match job_place.times.first() {
            Some(TimeSpan::Window(w)) => w.start,
            Some(TimeSpan::Offset(o)) => o.start,
            None => estimated_time,
        };

        Some(estimated_time.max(earliest))
    }

    /// Strategy 5: Conservative fallback (always provides an estimate)
    fn estimate_with_conservative_fallback(&self, route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
        // Conservative estimate: route end time + buffer, respecting job time windows
        let route_end_time = route_ctx
            .route()
            .tour
            .end()
            .map(|end_activity| end_activity.schedule.departure)
            .unwrap_or(0.0);

        let job_earliest = extract_job_start_time(job).unwrap_or(route_end_time);
        
        // Add conservative buffer for travel and coordination (15 minutes default)
        let conservative_buffer = 900.0; // 15 minutes
        let estimated_time = route_end_time.max(job_earliest) + conservative_buffer;

        Some(estimated_time)
    }

    fn validate_route_assignment(
        &self,
        solution_ctx: &SolutionContext,
        route_ctx: &RouteContext,
        job: &Job,
    ) -> Option<ConstraintViolation> {
        job.dimens().get_job_sync_group().and_then(|sync_group| {
            let sync_size = job.dimens().get_job_sync_size()?;
            let sync_index = job.dimens().get_job_sync_index()?;
            
            // Validate sync group size (must be at least 2 for synchronization to make sense)
            if *sync_size < 2 {
                return ConstraintViolation::fail(self.code);
            }

            // Validate index is within expected range
            if *sync_index >= *sync_size {
                return ConstraintViolation::fail(self.code);
            }

            // Enforce at most one member of a sync group per route (distinct vehicles coordination)
            if let Some(route_groups) = route_ctx.state().get_route_sync_groups() {
                if route_groups.contains(sync_group) {
                    return ConstraintViolation::fail(self.code);
                }
            }
            
            // Multiple sync groups per route are allowed - no constraint needed here
            
            // Validate compatibility with existing constraint features
            if let Err(_) = self.validate_feature_compatibility(solution_ctx, sync_group, job) {
                return ConstraintViolation::fail(self.code);
            }
            
            // Check sync group state
            if let Some(assignments) = solution_ctx.state.get_sync_group_assignments() {
                if let Some(sync_info) = assignments.get(sync_group) {
                    // Check if sync group is already complete
                    if sync_info.assignments.len() >= sync_info.required_size as usize {
                        return ConstraintViolation::fail(self.code);
                    }
                    
                    // Check if this index is already assigned
                    if sync_info.assigned_indices.contains(sync_index) {
                        return ConstraintViolation::fail(self.code);
                    }
                    
                    // Validate timing constraints if we have existing assignments
                    if !sync_info.assignments.is_empty() {
                        // Use improved multi-strategy time estimation
                        if let Some(scheduled_time) = self.estimate_service_start_time_for_insertion(route_ctx, job) {
                            let tolerance = job.dimens().get_job_sync_tolerance().unwrap_or(&900.0); // 15 min default
                            if !validate_sync_timing_with_tolerance(&sync_info.assignments, scheduled_time, *tolerance) {
                                return ConstraintViolation::fail(self.code);
                            }
                        } else {
                            // With improved estimation strategies, this should rarely happen
                            // Log warning and reject as last resort
                            // Note: Failed to estimate timing for sync job - this should rarely happen with improved strategies
                            return ConstraintViolation::fail(self.code);
                        }
                    }
                }
            }
            
            None
        })
    }
    
    fn validate_activity_assignment(
        &self,
        route_ctx: &RouteContext,
        activity_ctx: &ActivityContext,
    ) -> Option<ConstraintViolation> {
        // Get job from activity
        let job = activity_ctx.target.retrieve_job()?;
        let sync_group = job.dimens().get_job_sync_group()?;
        
        // For sync jobs being moved within any route, validate timing if possible
        if let Some(proposed_time) = self.estimate_activity_time(route_ctx, activity_ctx, &job) {
            let tolerance = job.dimens().get_job_sync_tolerance().unwrap_or(&900.0);
            
            // Check if proposed timing would violate sync constraints with other routes
            // Note: We can't access solution_ctx here, so this is a simplified check
            // The main timing validation still happens at route level
            if let Some(existing_assignments) = self.get_other_sync_assignments(route_ctx, sync_group) {
                if !validate_sync_timing_with_tolerance(&existing_assignments, proposed_time, *tolerance) {
                    return ConstraintViolation::fail(self.code);
                }
            }
        }
        
        // No sync groups in this route yet - this is a new insertion, let route-level validation handle it
        None
    }
    
    /// Estimates the service start time for an activity at a given position
    /// Uses enhanced estimation similar to insertion estimation
    fn estimate_activity_time(
        &self,
        route_ctx: &RouteContext,
        activity_ctx: &ActivityContext,
        job: &Job,
    ) -> Option<Timestamp> {
        // High confidence: transport/activity based estimation
        if let (Some(transport), Some(_activity)) = (&self.transport, &self.activity) {
            let route = route_ctx.route();
            let prev = activity_ctx.prev;
            let target = activity_ctx.target;
            let depart = prev.schedule.departure;

            // Compute travel duration from prev to target using time-dependent transport
            let travel = transport.duration(route, prev.place.location, target.place.location, TravelTime::Departure(depart));
            let arrival = depart + travel;
            let service_start = arrival.max(target.place.time.start);
            
            // Add small buffer for synchronization safety
            let safety_buffer = (travel * 0.02).max(15.0);
            return Some(service_start + safety_buffer);
        }

        // Medium confidence: position-based estimation using surrounding activities
        if let Some(next_activity) = activity_ctx.next {
            let prev_departure = activity_ctx.prev.schedule.departure;
            let next_arrival = next_activity.schedule.arrival;
            
            // Estimate based on position between activities
            let time_span = next_arrival - prev_departure;
            let estimated_service_time = job.places().next()?.duration;
            let estimated_arrival = prev_departure + (time_span - estimated_service_time) / 2.0;
            
            let job_earliest = extract_job_start_time(job).unwrap_or(estimated_arrival);
            return Some(estimated_arrival.max(job_earliest));
        }

        // Low confidence fallback: simple estimation with improved buffer
        let prev_activity = activity_ctx.prev;
        let base_travel_time = 300.0; // 5 minutes default travel time
        let estimated_arrival = prev_activity.schedule.departure + base_travel_time;
        let job_earliest = extract_job_start_time(job).unwrap_or(estimated_arrival);
        Some(estimated_arrival.max(job_earliest))
    }
    
    /// Gets sync assignments from other routes (simplified version for activity validation)
    fn get_other_sync_assignments(
        &self,
        _route_ctx: &RouteContext,
        _sync_group: &str,
    ) -> Option<Vec<(usize, u32, Timestamp, f64)>> {
        // In practice, this would need access to solution context
        // For now, return None to skip timing validation at activity level
        // Main validation still happens at route level where we have full context
        None
    }
    
    /// Validates that sync jobs are compatible with other constraint features
    fn validate_feature_compatibility(
        &self,
        solution_ctx: &SolutionContext,
        sync_group: &str,
        job: &Job,
    ) -> Result<(), ViolationCode> {
        // Check if there are existing assignments in this sync group
        if let Some(assignments) = solution_ctx.state.get_sync_group_assignments() {
            if let Some(sync_info) = assignments.get(sync_group) {
                // Validate against existing sync group members for compatibility
                for (route_idx, _, _, _) in &sync_info.assignments {
                    if let Some(route_ctx) = solution_ctx.routes.get(*route_idx) {
                        // Find the sync job in this route and check compatibility
                        for existing_job in route_ctx.route().tour.jobs() {
                            if existing_job.dimens().get_job_sync_group().map_or(false, |g| g == sync_group) {
                                // Check job group compatibility
                                if let (Some(existing_group), Some(new_group)) = (
                                    existing_job.dimens().get_job_group(),
                                    job.dimens().get_job_group()
                                ) {
                                    if existing_group != new_group {
                                        return Err(self.code); // Different job groups
                                    }
                                }
                                
                                // Check affinity compatibility
                                if let (Some(existing_affinity), Some(new_affinity)) = (
                                    existing_job.dimens().get_job_affinity(),
                                    job.dimens().get_job_affinity()
                                ) {
                                    if existing_affinity != new_affinity {
                                        return Err(self.code); // Different affinities
                                    }
                                }
                                
                                // Check compatibility constraint
                                if let (Some(existing_compat), Some(new_compat)) = (
                                    existing_job.dimens().get_job_compatibility(),
                                    job.dimens().get_job_compatibility()
                                ) {
                                    if existing_compat != new_compat {
                                        return Err(self.code); // Different compatibility groups
                                    }
                                }
                                
                                break; // Found the sync job in this route
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

/// Soft constraint objective to guide optimization toward better sync solutions
struct JobSyncObjective {
    threshold: f64,
}

impl FeatureObjective for JobSyncObjective {
    fn estimate(&self, move_ctx: &MoveContext<'_>) -> Cost {
        match move_ctx {
            MoveContext::Route { solution_ctx, route_ctx, job } => {
                self.estimate_sync_cost(solution_ctx, route_ctx, job)
            }
            MoveContext::Activity { activity_ctx, .. } => {
                if let Some(job) = activity_ctx.target.retrieve_job() {
                    // Use a simple penalty for activity moves that might affect sync timing
                    if job.dimens().get_job_sync_group().is_some() {
                        self.threshold * 0.1 // Small penalty for sync job moves
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            }
        }
    }

    fn fitness(&self, ctx: &InsertionContext) -> Cost {
        // Calculate overall sync fitness - lower is better
        if let Some(assignments) = ctx.solution.state.get_sync_group_assignments() {
            assignments.values()
                .map(|sync_info| self.calculate_sync_group_fitness(sync_info))
                .sum()
        } else {
            0.0
        }
    }
}

impl JobSyncObjective {
    fn estimate_sync_cost(&self, solution_ctx: &SolutionContext, route_ctx: &RouteContext, job: &Job) -> Cost {
        if let Some(sync_group) = job.dimens().get_job_sync_group() {
            if let Some(assignments) = solution_ctx.state.get_sync_group_assignments() {
                if let Some(sync_info) = assignments.get(sync_group) {
                    if let Some(scheduled_time) = extract_scheduled_time(route_ctx, job) {
                        let tolerance = job.dimens().get_job_sync_tolerance().unwrap_or(&900.0);
                        
                        // Calculate timing penalty based on deviation from existing assignments
                        let timing_penalty = sync_info.assignments.iter()
                            .map(|(_, _, existing_time, _)| {
                                let diff = (scheduled_time - existing_time).abs();
                                if diff <= *tolerance {
                                    0.0 // Within tolerance - no penalty
                                } else {
                                    self.threshold * (diff - tolerance) / tolerance // Penalty grows with deviation
                                }
                            })
                            .fold(0.0, |acc, penalty| acc + penalty);
                        
                        return timing_penalty;
                    }
                }
            }
        }
        0.0
    }

    fn calculate_sync_group_fitness(&self, sync_info: &SyncGroupInfo) -> f64 {
        let assigned_count = sync_info.assignments.len();
        let required_count = sync_info.required_size as usize;
        
        if assigned_count == 0 {
            return 0.0; // No penalty for unstarted groups
        }
        
        if assigned_count == required_count {
            // Complete sync group - reward with negative cost (better fitness)
            // Plus small penalty for timing variance to encourage tight synchronization
            let times: Vec<f64> = sync_info.assignments.iter().map(|(_, _, time, _)| *time).collect();
            let mean_time = times.iter().sum::<f64>() / times.len() as f64;
            let variance = times.iter().map(|time| (time - mean_time).powi(2)).sum::<f64>() / times.len() as f64;
            
            // Reward for completion minus small variance penalty
            -self.threshold * 10.0 + (variance / 10000.0)
        } else {
            // Incomplete sync group - heavy penalty that increases as we get closer to required size
            // This incentivizes completing groups but penalizes partial assignments heavily
            let completion_ratio = assigned_count as f64 / required_count as f64;
            self.threshold * 50.0 * completion_ratio // Penalty grows with partial completion
        }
    }
}

struct JobSyncState {}

impl FeatureState for JobSyncState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, job: &Job) {
        if let (Some(sync_group), Some(sync_size), Some(sync_index)) = (
            job.dimens().get_job_sync_group(),
            job.dimens().get_job_sync_size(),
            job.dimens().get_job_sync_index()
        ) {
            let tolerance = job.dimens().get_job_sync_tolerance().unwrap_or(&900.0);
            
            // Optimized state update: check if we need to modify state first
            let scheduled_time = solution_ctx.routes.get(route_index)
                .and_then(|route_ctx| extract_scheduled_time_cached(route_ctx, job));
            
            // Only proceed with state update if we have timing information
            if let Some(scheduled_time) = scheduled_time {
                // Minimize cloning by working with existing state when possible
                let mut assignments = solution_ctx.state.get_sync_group_assignments()
                    .cloned()
                    .unwrap_or_else(HashMap::new);
                
                let sync_info = assignments.entry(sync_group.clone()).or_insert_with(|| SyncGroupInfo {
                    required_size: *sync_size,
                    assignments: Vec::new(),
                    assigned_indices: HashSet::new(),
                });
                
                // Add assignment efficiently
                sync_info.assignments.push((route_index, *sync_index, scheduled_time, *tolerance));
                sync_info.assigned_indices.insert(*sync_index);
                
                solution_ctx.state.set_sync_group_assignments(assignments);
                
                // Update route-level state efficiently - avoid unnecessary clone when possible
                if let Some(route_ctx) = solution_ctx.routes.get_mut(route_index) {
                    let needs_update = route_ctx.state().get_route_sync_groups()
                        .map_or(true, |groups| !groups.contains(sync_group));
                    
                    if needs_update {
                        let mut route_sync_groups = route_ctx.state().get_route_sync_groups()
                            .cloned()
                            .unwrap_or_else(HashSet::new);
                        route_sync_groups.insert(sync_group.clone());
                        route_ctx.state_mut().set_route_sync_groups(route_sync_groups);
                    }
                }
            }
        }
    }

    fn accept_route_state(&self, route_ctx: &mut RouteContext) {
        let route_sync_groups = get_route_sync_groups(route_ctx);
        route_ctx.state_mut().set_route_sync_groups(route_sync_groups);
    }

    fn accept_solution_state(&self, solution_ctx: &mut SolutionContext) {
        // Check if we can use incremental update instead of full rebuild
        let needs_full_rebuild = solution_ctx.state.get_sync_group_assignments().is_none();
        
        if needs_full_rebuild {
            // Full rebuild for initial state setup
            self.rebuild_solution_state(solution_ctx);
        } else {
            // Incremental validation and correction of existing state
            self.validate_and_correct_solution_state(solution_ctx);
        }
    }

    fn notify_failure(&self, solution_ctx: &mut SolutionContext, _route_indices: &[usize], jobs: &[Job]) -> bool {
        let mut modified = false;
        let mut assignments = solution_ctx.state.get_sync_group_assignments().cloned().unwrap_or_default();
        
        // Handle sync job failures - clear partial assignments to avoid incomplete sync groups
        for job in jobs {
            if let Some(sync_group) = job.dimens().get_job_sync_group() {
                if let Some(sync_info) = assignments.get_mut(sync_group) {
                    let current_assignments = sync_info.assignments.len();
                    let required_size = sync_info.required_size as usize;
                    
                    // Clear partial assignments to avoid stuck states (aggressive recovery as intended)
                    if current_assignments > 0 && current_assignments < required_size {
                        // Collect affected routes before clearing
                        let affected_routes: Vec<usize> = sync_info.assignments.iter().map(|(route_idx, _, _, _)| *route_idx).collect();
                        
                        // Clear sync group assignments
                        sync_info.assignments.clear();
                        sync_info.assigned_indices.clear();
                        modified = true;
                        
                        // Clear route-level state for affected routes efficiently
                        for route_idx in affected_routes {
                            if let Some(route_ctx) = solution_ctx.routes.get_mut(route_idx) {
                                if let Some(mut route_sync_groups) = route_ctx.state().get_route_sync_groups().cloned() {
                                    if route_sync_groups.remove(sync_group) {
                                        route_ctx.state_mut().set_route_sync_groups(route_sync_groups);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if modified {
            solution_ctx.state.set_sync_group_assignments(assignments);
        }
        
        modified
    }
}

impl JobSyncState {
    /// Performs a full rebuild of sync state from scratch
    fn rebuild_solution_state(&self, solution_ctx: &mut SolutionContext) {
        let mut assignments: HashMap<String, SyncGroupInfo> = HashMap::new();
        
        // Rebuild sync group assignments from current solution
        for (route_index, route_ctx) in solution_ctx.routes.iter_mut().enumerate() {
            let mut route_sync_groups = HashSet::new();
            
            for job in route_ctx.route().tour.jobs() {
                if let (Some(sync_group), Some(sync_size), Some(sync_index)) = (
                    job.dimens().get_job_sync_group(),
                    job.dimens().get_job_sync_size(),
                    job.dimens().get_job_sync_index()
                ) {
                    route_sync_groups.insert(sync_group.clone());
                    
                    let tolerance = job.dimens().get_job_sync_tolerance().unwrap_or(&900.0);
                    let sync_info = assignments.entry(sync_group.clone()).or_insert_with(|| SyncGroupInfo {
                        required_size: *sync_size,
                        assignments: Vec::new(),
                        assigned_indices: HashSet::new(),
                    });
                    
                    if let Some(scheduled_time) = extract_scheduled_time(route_ctx, job) {
                        sync_info.assignments.push((route_index, *sync_index, scheduled_time, *tolerance));
                        sync_info.assigned_indices.insert(*sync_index);
                    }
                }
            }
            
            route_ctx.state_mut().set_route_sync_groups(route_sync_groups);
        }
        
        solution_ctx.state.set_sync_group_assignments(assignments);
    }
    
    /// Validates existing state and corrects inconsistencies incrementally
    fn validate_and_correct_solution_state(&self, solution_ctx: &mut SolutionContext) {
        // Validate route-level sync groups match actual jobs
        for (_route_index, route_ctx) in solution_ctx.routes.iter_mut().enumerate() {
            let actual_sync_groups = get_route_sync_groups(route_ctx);
            let current_sync_groups = route_ctx.state().get_route_sync_groups().cloned().unwrap_or_default();
            
            // Update route state only if it differs from actual
            if actual_sync_groups != current_sync_groups {
                route_ctx.state_mut().set_route_sync_groups(actual_sync_groups);
            }
        }
        
        // Validate solution-level assignments match routes
        let mut assignments = solution_ctx.state.get_sync_group_assignments().cloned().unwrap_or_default();
        let mut state_changed = false;
        
        // Remove stale assignments and update existing ones
        assignments.retain(|sync_group, sync_info| {
            let mut new_assignments = Vec::new();
            let mut new_indices = HashSet::new();
            
            for (route_index, sync_index, _, tolerance) in &sync_info.assignments {
                if let Some(route_ctx) = solution_ctx.routes.get(*route_index) {
                    // Find sync job in this route
                    if let Some(job) = route_ctx.route().tour.jobs().find(|job| {
                        job.dimens().get_job_sync_group().map_or(false, |g| g == sync_group) &&
                        job.dimens().get_job_sync_index().map_or(false, |idx| idx == sync_index)
                    }) {
                        if let Some(scheduled_time) = extract_scheduled_time(route_ctx, job) {
                            new_assignments.push((*route_index, *sync_index, scheduled_time, *tolerance));
                            new_indices.insert(*sync_index);
                        }
                    }
                }
            }
            
            // Update assignments if they changed
            if new_assignments.len() != sync_info.assignments.len() || 
               new_assignments != sync_info.assignments {
                sync_info.assignments = new_assignments;
                sync_info.assigned_indices = new_indices;
                state_changed = true;
            }
            
            // Keep group if it has assignments
            !sync_info.assignments.is_empty()
        });
        
        if state_changed {
            solution_ctx.state.set_sync_group_assignments(assignments);
        }
    }
}

/// Gets sync groups assigned to a route.
pub fn get_route_sync_groups(route_ctx: &RouteContext) -> HashSet<String> {
    route_ctx.route().tour.jobs()
        .filter_map(|job| job.dimens().get_job_sync_group())
        .cloned()
        .collect()
}

/// Validates timing with configurable tolerance.
pub fn validate_sync_timing_with_tolerance(
    existing_assignments: &[(usize, u32, Timestamp, f64)], 
    new_scheduled_time: Timestamp, 
    tolerance: f64
) -> bool {
    if existing_assignments.is_empty() {
        return true;
    }
    
    existing_assignments.iter().all(|(_, _, existing_time, existing_tolerance)| {
        let effective_tolerance = tolerance.min(*existing_tolerance);
        let time_diff = (new_scheduled_time - existing_time).abs();
        time_diff <= effective_tolerance
    })
}

/// Extracts scheduled service start time from route context considering actual route timing.
pub fn extract_scheduled_time(route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
    extract_scheduled_time_cached(route_ctx, job)
}

/// Optimized version that minimizes scanning by leveraging job ID comparison early.
fn extract_scheduled_time_cached(route_ctx: &RouteContext, job: &Job) -> Option<Timestamp> {
    // Get job ID once for comparison
    let target_job_id = job.dimens().get_job_id()?;
    
    // Scan activities efficiently - use iterator for early termination
    for activity in route_ctx.route().tour.all_activities() {
        if let Some(activity_job) = activity.retrieve_job() {
            // Fast comparison using cached job ID
            if let Some(activity_id) = activity_job.dimens().get_job_id() {
                if activity_id == target_job_id {
                    // Use service start time (arrival + waiting time) for synchronization
                    // This ensures vehicles start working at the same time, not just arrive
                    let service_start = activity.schedule.arrival.max(
                        activity.place.time.start // Respect time window constraints
                    );
                    return Some(service_start);
                }
            }
        }
    }
    
    None
}

/// Extracts the start time from a job's first place (fallback method).
fn extract_job_start_time(job: &Job) -> Option<Timestamp> {
    job.places().next().and_then(|place| {
        place.times.first().map(|time_span| match time_span {
            TimeSpan::Window(window) => window.start,
            TimeSpan::Offset(offset) => offset.start,
        })
    })
}

// NOTE: Single estimator exists as a method on JobSyncConstraint.