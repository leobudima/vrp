// Comprehensive tests for vehicle affinity feature
// Note: This file is included in vehicle_affinity_test.rs, so no duplicate imports/definitions

fn create_test_single_with_affinity(
    job_id: &str,
    affinity: Option<&str>,
    sequence: Option<u32>,
    duration_days: Option<u32>,
    tolerance: Option<f64>
) -> Arc<Single> {
    let mut builder = TestSingleBuilder::default();
    
    if let Some(affinity) = affinity {
        builder.dimens_mut().set_job_affinity(affinity.to_string());
        if let Some(seq) = sequence {
            builder.dimens_mut().set_job_affinity_sequence(seq);
        }
        if let Some(duration) = duration_days {
            builder.dimens_mut().set_job_affinity_duration_days(duration);
        }
        if let Some(tol) = tolerance {
            builder.dimens_mut().set_job_affinity_tolerance(tol);
        }
    }
    
    builder.id(job_id).build_shared()
}

#[cfg(test)]
mod basic_affinity_tests {
    use super::*;

    #[test]
    fn can_assign_jobs_with_same_affinity_to_same_vehicle_extended() {
        let fleet = create_test_fleet();
        let job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), None, None, None));
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

    #[test]
    fn cannot_assign_jobs_with_same_affinity_to_different_vehicle_extended() {
        let fleet = create_test_fleet();
        let job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), None, None, None));
        let route_ctx = RouteContextBuilder::default()
            .with_route(RouteBuilder::default().with_vehicle(&fleet, "v2").build())
            .build();

        let mut solution_ctx = create_test_solution_context(&fleet, vec![("v1", vec![Some("affinity1")])]);
        
        let feature = create_test_affinity_feature();
        feature.state.as_ref().unwrap().accept_solution_state(&mut solution_ctx);

        let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &job };
        let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);

        assert_eq!(result, ConstraintViolation::fail(VIOLATION_CODE));
    }
}

#[cfg(test)]
mod sequential_affinity_tests {
    use super::*;

    #[test]
    fn validates_input_consistency_for_sequential_jobs() {
        let fleet = create_test_fleet();
        let route_ctx = RouteContextBuilder::default()
            .with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build())
            .build();
        let solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        let feature = create_test_affinity_feature();
        
        // Test job with sequence but no duration
        let invalid_job1 = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), None, None));
        let move_ctx1 = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &invalid_job1 };
        let result1 = feature.constraint.as_ref().unwrap().evaluate(&move_ctx1);
        assert_eq!(result1, ConstraintViolation::fail(VIOLATION_CODE));
        
        // Test job with duration but no sequence
        let invalid_job2 = Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), None, Some(3), None));
        let move_ctx2 = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &invalid_job2 };
        let result2 = feature.constraint.as_ref().unwrap().evaluate(&move_ctx2);
        assert_eq!(result2, ConstraintViolation::fail(VIOLATION_CODE));
        
        // Test job with sequence >= duration
        let invalid_job3 = Job::Single(create_test_single_with_affinity("job3", Some("affinity1"), Some(3), Some(3), None));
        let move_ctx3 = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &invalid_job3 };
        let result3 = feature.constraint.as_ref().unwrap().evaluate(&move_ctx3);
        assert_eq!(result3, ConstraintViolation::fail(VIOLATION_CODE));
        
        // Test valid job
        let valid_job = Job::Single(create_test_single_with_affinity("job4", Some("affinity1"), Some(0), Some(3), None));
        let move_ctx4 = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &valid_job };
        let result4 = feature.constraint.as_ref().unwrap().evaluate(&move_ctx4);
        assert!(result4.is_none());
    }

    #[test]
    fn prevents_duplicate_sequence_assignment() {
        let fleet = create_test_fleet();
        let route_ctx = RouteContextBuilder::default()
            .with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build())
            .build();
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up existing affinity group with sequence 0
        let mut group_states = HashMap::new();
        let mut group_state = AffinityGroupState::new(3);
        group_state.assigned_sequences.insert(0, 1000.0);
        group_state.expected_sequences = (0..3).collect();
        group_states.insert("affinity1".to_string(), group_state);
        solution_ctx.state.set_affinity_group_states(group_states);
        
        let feature = create_test_affinity_feature();
        
        // Try to assign another job with same sequence
        let duplicate_job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &duplicate_job };
        let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);
        
        assert_eq!(result, ConstraintViolation::fail(VIOLATION_CODE));
    }

    #[test]
    fn allows_next_sequence_in_group() {
        let fleet = create_test_fleet();
        let route_ctx = RouteContextBuilder::default()
            .with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build())
            .build();
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up existing affinity group with sequence 0
        let mut group_states = HashMap::new();
        let mut group_state = AffinityGroupState::new(3);
        group_state.assigned_sequences.insert(0, 1000.0);
        group_state.expected_sequences = (0..3).collect();
        group_states.insert("affinity1".to_string(), group_state);
        solution_ctx.state.set_affinity_group_states(group_states);
        
        let feature = create_test_affinity_feature();
        
        // Try to assign job with sequence 1
        let next_job = Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), Some(1), Some(3), None));
        let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &next_job };
        let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);
        
        assert!(result.is_none());
    }

    #[test]
    fn rejects_invalid_sequence_for_group() {
        let fleet = create_test_fleet();
        let route_ctx = RouteContextBuilder::default()
            .with_route(RouteBuilder::default().with_vehicle(&fleet, "v1").build())
            .build();
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up existing affinity group for 3-day duration
        let mut group_states = HashMap::new();
        let mut group_state = AffinityGroupState::new(3);
        group_state.assigned_sequences.insert(0, 1000.0);
        group_state.expected_sequences = (0..3).collect();
        group_states.insert("affinity1".to_string(), group_state);
        solution_ctx.state.set_affinity_group_states(group_states);
        
        let feature = create_test_affinity_feature();
        
        // Try to assign job with sequence 5 (invalid for 3-day duration)
        let invalid_job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(5), Some(3), None));
        let move_ctx = MoveContext::Route { solution_ctx: &solution_ctx, route_ctx: &route_ctx, job: &invalid_job };
        let result = feature.constraint.as_ref().unwrap().evaluate(&move_ctx);
        
        assert_eq!(result, ConstraintViolation::fail(VIOLATION_CODE));
    }
}

#[cfg(test)]
mod partial_assignment_prevention_tests {
    use super::*;

    #[test]
    fn notify_failure_clears_partial_affinity_assignments() {
        let fleet = create_test_fleet();
        let feature = create_test_affinity_feature();
        let state = &feature.state.unwrap();
        
        let failed_job = Job::Single(create_test_single_with_affinity("job3", Some("affinity1"), Some(2), Some(3), None));
        
        // Create solution with partial affinity group (2 out of 3 jobs assigned)
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up partial affinity group
        let mut group_states = HashMap::new();
        let mut group_state = AffinityGroupState::new(3);
        group_state.assigned_sequences.insert(0, 1000.0);
        group_state.assigned_sequences.insert(1, 1100.0);
        group_state.expected_sequences = (0..3).collect();
        group_states.insert("affinity1".to_string(), group_state);
        solution_ctx.state.set_affinity_group_states(group_states);
        
        // Set up corresponding vehicle affinities and schedules
        let mut affinities = HashMap::new();
        affinities.insert("affinity1".to_string(), fleet.actors[0].vehicle.clone());
        solution_ctx.state.set_vehicle_affinities(affinities);
        
        let mut schedules = HashMap::new();
        schedules.insert("affinity1".to_string(), vec![(0, 1000.0), (1, 1100.0)]);
        solution_ctx.state.set_affinity_schedules(schedules);
        
        // Notify failure for the third job
        let modified = state.notify_failure(&mut solution_ctx, &[2], &[failed_job]);
        
        // Should return true indicating state was modified
        assert!(modified, "Failure notification should modify state");
        
        // Verify partial assignments were cleared
        let group_states = solution_ctx.state.get_affinity_group_states().unwrap();
        assert!(!group_states.contains_key("affinity1"), "Partial affinity group should be cleared");
        
        let affinities = solution_ctx.state.get_vehicle_affinities().unwrap();
        assert!(!affinities.contains_key("affinity1"), "Vehicle affinity should be cleared");
        
        let schedules = solution_ctx.state.get_affinity_schedules().unwrap();
        assert!(!schedules.contains_key("affinity1"), "Affinity schedule should be cleared");
    }

    #[test]
    fn notify_failure_preserves_complete_affinity_groups() {
        let fleet = create_test_fleet();
        let feature = create_test_affinity_feature();
        let state = &feature.state.unwrap();
        
        let failed_job = Job::Single(create_test_single_with_affinity("job_other", Some("affinity2"), Some(0), Some(2), None));
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up complete affinity group 1
        let mut group_states = HashMap::new();
        let mut complete_group = AffinityGroupState::new(2);
        complete_group.assigned_sequences.insert(0, 1000.0);
        complete_group.assigned_sequences.insert(1, 1100.0);
        complete_group.expected_sequences = (0..2).collect();
        group_states.insert("affinity1".to_string(), complete_group);
        
        // Set up partial affinity group 2
        let mut partial_group = AffinityGroupState::new(2);
        partial_group.assigned_sequences.insert(0, 2000.0);
        partial_group.expected_sequences = (0..2).collect();
        group_states.insert("affinity2".to_string(), partial_group);
        
        solution_ctx.state.set_affinity_group_states(group_states);
        
        // Notify failure for group 2 job
        let modified = state.notify_failure(&mut solution_ctx, &[1], &[failed_job]);
        
        assert!(modified, "Should modify state for partial group cleanup");
        
        // Verify group1 is preserved, group2 is cleared
        let group_states = solution_ctx.state.get_affinity_group_states().unwrap();
        assert!(group_states.contains_key("affinity1"), "Complete group should be preserved");
        assert!(!group_states.contains_key("affinity2"), "Partial group should be cleared");
        
        let group1 = group_states.get("affinity1").unwrap();
        assert!(group1.is_complete(), "Complete group should remain complete");
    }

    #[test]
    fn notify_failure_ignores_non_affinity_jobs() {
        let fleet = create_test_fleet();
        let feature = create_test_affinity_feature();
        let state = &feature.state.unwrap();
        
        let non_affinity_job = Job::Single(create_test_single_with_affinity("regular_job", None, None, None, None));
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up some affinity group
        let mut group_states = HashMap::new();
        let mut group_state = AffinityGroupState::new(3);
        group_state.assigned_sequences.insert(0, 1000.0);
        group_state.expected_sequences = (0..3).collect();
        group_states.insert("affinity1".to_string(), group_state);
        solution_ctx.state.set_affinity_group_states(group_states.clone());
        
        // Notify failure for non-affinity job
        let modified = state.notify_failure(&mut solution_ctx, &[0], &[non_affinity_job]);
        
        // Should return false as no affinity state was modified
        assert!(!modified, "Non-affinity job failure should not modify state");
        
        // Verify affinity state is unchanged
        let updated_states = solution_ctx.state.get_affinity_group_states().unwrap();
        assert_eq!(updated_states.len(), group_states.len(), "Affinity groups should be unchanged");
    }
}

#[cfg(test)]
mod affinity_group_state_tests {
    use super::*;

    #[test]
    fn affinity_group_state_creation() {
        let state = AffinityGroupState::new(3);
        
        assert_eq!(state.duration_days, 3);
        assert_eq!(state.expected_sequences.len(), 3);
        assert!(state.expected_sequences.contains(&0));
        assert!(state.expected_sequences.contains(&1));
        assert!(state.expected_sequences.contains(&2));
        assert!(state.assigned_sequences.is_empty());
        assert!(state.assigned_vehicle.is_none());
        assert!(state.base_timestamp.is_none());
    }

    #[test]
    fn affinity_group_state_completeness_check() {
        let mut state = AffinityGroupState::new(2);
        
        // Initially not complete and not partial
        assert!(!state.is_complete());
        assert!(!state.is_partial());
        
        // Add one job - becomes partial
        state.assigned_sequences.insert(0, 1000.0);
        assert!(!state.is_complete());
        assert!(state.is_partial());
        
        // Add second job - becomes complete
        state.assigned_sequences.insert(1, 1100.0);
        assert!(state.is_complete());
        assert!(!state.is_partial());
    }

    #[test]
    fn affinity_group_state_with_gaps() {
        let mut state = AffinityGroupState::new(3);
        
        // Add non-consecutive sequences
        state.assigned_sequences.insert(0, 1000.0);
        state.assigned_sequences.insert(2, 1200.0);
        
        // Should be partial (not complete due to missing sequence 1)
        assert!(!state.is_complete());
        assert!(state.is_partial());
        
        // Add the missing sequence
        state.assigned_sequences.insert(1, 1100.0);
        
        // Now should be complete
        assert!(state.is_complete());
        assert!(!state.is_partial());
    }
}

#[cfg(test)]
mod state_management_tests {
    use super::*;

    #[test]
    fn accept_insertion_updates_group_state() {
        let fleet = create_test_fleet();
        let feature = create_test_affinity_feature();
        let state = &feature.state.unwrap();
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        let job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), Some(2.0 * 3600.0)));
        
        // Add a route first
        let route = RouteBuilder::default().with_vehicle(&fleet, "v1").build();
        let route_ctx = RouteContextBuilder::default().with_route(route).build();
        solution_ctx.routes.push(route_ctx);
        
        // Accept insertion
        state.accept_insertion(&mut solution_ctx, 0, &job);
        
        // Verify group state was created and updated
        let group_states = solution_ctx.state.get_affinity_group_states().unwrap();
        assert!(group_states.contains_key("affinity1"));
        
        let group_state = group_states.get("affinity1").unwrap();
        assert_eq!(group_state.duration_days, 3);
        assert!(group_state.assigned_sequences.contains_key(&0));
        assert!(group_state.assigned_vehicle.is_some());
        
        // Verify vehicle affinity was set
        let affinities = solution_ctx.state.get_vehicle_affinities().unwrap();
        assert!(affinities.contains_key("affinity1"));
        
        // Verify schedule was updated
        let schedules = solution_ctx.state.get_affinity_schedules().unwrap();
        assert!(schedules.contains_key("affinity1"));
        let schedule = schedules.get("affinity1").unwrap();
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0].0, 0); // sequence
    }

    #[test]
    fn accept_solution_state_rebuilds_from_routes() {
        let fleet = create_test_fleet();
        let feature = create_test_affinity_feature();
        let state = &feature.state.unwrap();
        
        // Create solution with jobs in routes but no state
        let job1 = create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(2), None);
        let job2 = create_test_single_with_affinity("job2", Some("affinity1"), Some(1), Some(2), None);
        
        let route = RouteBuilder::default()
            .with_vehicle(&fleet, "v1")
            .add_activity(ActivityBuilder::with_location(1).job(Some(job1)).build())
            .add_activity(ActivityBuilder::with_location(2).job(Some(job2)).build())
            .build();
        
        let route_ctx = RouteContextBuilder::default().with_route(route).build();
        
        let mut solution_ctx = SolutionContext {
            required: vec![],
            ignored: vec![],
            unassigned: Default::default(),
            locked: Default::default(),
            routes: vec![route_ctx],
            registry: RegistryContext::new(&TestGoalContextBuilder::default().build(), Registry::new(&fleet, test_random())),
            state: Default::default(),
        };
        
        // Accept solution state (should rebuild)
        state.accept_solution_state(&mut solution_ctx);
        
        // Verify state was rebuilt correctly
        let group_states = solution_ctx.state.get_affinity_group_states().unwrap();
        assert!(group_states.contains_key("affinity1"));
        
        let group_state = group_states.get("affinity1").unwrap();
        assert_eq!(group_state.assigned_sequences.len(), 2);
        assert!(group_state.assigned_sequences.contains_key(&0));
        assert!(group_state.assigned_sequences.contains_key(&1));
        assert!(group_state.is_complete());
        
        let affinities = solution_ctx.state.get_vehicle_affinities().unwrap();
        assert!(affinities.contains_key("affinity1"));
        
        let schedules = solution_ctx.state.get_affinity_schedules().unwrap();
        assert!(schedules.contains_key("affinity1"));
        let schedule = schedules.get("affinity1").unwrap();
        assert_eq!(schedule.len(), 2);
    }

    #[test]
    fn incremental_state_validation_detects_inconsistencies() {
        let fleet = create_test_fleet();
        let feature = create_test_affinity_feature();
        let state = &feature.state.unwrap();
        
        let mut solution_ctx = create_test_solution_context(&fleet, vec![]);
        
        // Set up inconsistent state (group state claims assignments but no jobs in routes)
        let mut group_states = HashMap::new();
        let mut group_state = AffinityGroupState::new(2);
        group_state.assigned_sequences.insert(0, 1000.0);
        group_state.assigned_sequences.insert(1, 1100.0);
        group_states.insert("affinity1".to_string(), group_state);
        solution_ctx.state.set_affinity_group_states(group_states);
        
        // Accept solution state (should detect inconsistency and rebuild)
        state.accept_solution_state(&mut solution_ctx);
        
        // Verify inconsistent state was cleared
        let updated_states = solution_ctx.state.get_affinity_group_states().unwrap();
        assert!(!updated_states.contains_key("affinity1") || updated_states.get("affinity1").unwrap().assigned_sequences.is_empty());
    }
}

#[cfg(test)]
mod merge_function_tests {
    use super::*;

    #[test]
    fn merge_allows_same_affinity_with_different_sequences() {
        let feature = create_test_affinity_feature();
        let constraint = &feature.constraint.unwrap();
        
        let job1 = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        let job2 = Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), Some(1), Some(3), None));
        
        let result = constraint.merge(job1, job2);
        assert!(result.is_ok());
    }

    #[test]
    fn merge_rejects_same_affinity_with_same_sequence() {
        let feature = create_test_affinity_feature();
        let constraint = &feature.constraint.unwrap();
        
        let job1 = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        let job2 = Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), Some(0), Some(3), None));
        
        let result = constraint.merge(job1, job2);
        assert_eq!(result, Err(VIOLATION_CODE));
    }

    #[test]
    fn merge_rejects_same_affinity_with_different_duration() {
        let feature = create_test_affinity_feature();
        let constraint = &feature.constraint.unwrap();
        
        let job1 = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        let job2 = Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), Some(1), Some(2), None));
        
        let result = constraint.merge(job1, job2);
        assert_eq!(result, Err(VIOLATION_CODE));
    }

    #[test]
    fn merge_rejects_different_affinities() {
        let feature = create_test_affinity_feature();
        let constraint = &feature.constraint.unwrap();
        
        let job1 = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        let job2 = Job::Single(create_test_single_with_affinity("job2", Some("affinity2"), Some(0), Some(2), None));
        
        let result = constraint.merge(job1, job2);
        assert_eq!(result, Err(VIOLATION_CODE));
    }

    #[test]
    fn merge_allows_no_affinity() {
        let feature = create_test_affinity_feature();
        let constraint = &feature.constraint.unwrap();
        
        let job1 = Job::Single(create_test_single_with_affinity("job1", None, None, None, None));
        let job2 = Job::Single(create_test_single_with_affinity("job2", None, None, None, None));
        
        let result = constraint.merge(job1, job2);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod helper_function_tests {
    use super::*;

    #[test]
    fn calculate_day_duration_extracts_from_time_window() {
        let job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        
        let duration = calculate_day_duration(&job);
        
        // Test jobs may have different time windows, so just check it's positive
        assert!(duration > 0.0);
    }

    #[test]
    fn extract_job_start_time_works() {
        let job = Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None));
        
        let start_time = extract_job_start_time(&job);
        
        // Test jobs typically have some default time window
        assert!(start_time.is_some());
    }

    #[test]
    fn affinity_group_state_methods() {
        let mut state = AffinityGroupState::new(3);
        
        // Test initial state
        assert!(!state.is_complete());
        assert!(!state.is_partial());
        
        // Add partial assignments
        state.assigned_sequences.insert(0, 1000.0);
        assert!(!state.is_complete());
        assert!(state.is_partial());
        
        state.assigned_sequences.insert(1, 1100.0);
        assert!(!state.is_complete());
        assert!(state.is_partial());
        
        // Complete the group
        state.assigned_sequences.insert(2, 1200.0);
        assert!(state.is_complete());
        assert!(!state.is_partial());
    }
}

#[cfg(test)]
mod sophisticated_logic_tests {
    use super::*;

    #[test]
    fn evaluate_affinity_group_assignment_returns_cost() {
        let fleet = create_test_fleet();
        let vehicle = &fleet.vehicles[0];
        
        let jobs = vec![
            Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None)),
            Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), Some(1), Some(3), None)),
            Job::Single(create_test_single_with_affinity("job3", Some("affinity1"), Some(2), Some(3), None)),
        ];
        
        // Create a simple transport cost implementation for testing
        use crate::models::problem::SimpleTransportCost;
        let durations = vec![10.0; 100]; // 10x10 matrix with 10.0 duration
        let distances = vec![5.0; 100]; // 10x10 matrix with 5.0 distance
        let transport = Arc::new(SimpleTransportCost::new(durations, distances).unwrap());
        
        let cost = evaluate_affinity_group_assignment(&jobs, vehicle, transport.as_ref());
        
        assert!(cost.is_some());
        assert!(cost.unwrap() >= 0.0);
    }

    #[test]
    fn find_optimal_affinity_start_date_returns_valid_time() {
        let fleet = create_test_fleet();
        let vehicle = &fleet.vehicles[0];
        
        let jobs = vec![
            Job::Single(create_test_single_with_affinity("job1", Some("affinity1"), Some(0), Some(3), None)),
            Job::Single(create_test_single_with_affinity("job2", Some("affinity1"), Some(1), Some(3), None)),
            Job::Single(create_test_single_with_affinity("job3", Some("affinity1"), Some(2), Some(3), None)),
        ];
        
        let planning_horizon = TimeWindow { start: 0.0, end: 7.0 * 24.0 * 3600.0 }; // 7 days
        
        let start_date = find_optimal_affinity_start_date(&jobs, &planning_horizon, vehicle);
        
        assert!(start_date.is_some());
        let start = start_date.unwrap();
        assert!(start >= planning_horizon.start);
        assert!(start <= planning_horizon.end);
    }

    #[test]
    fn empty_jobs_return_appropriate_defaults() {
        let fleet = create_test_fleet();
        let vehicle = &fleet.vehicles[0];
        
        let empty_jobs: Vec<Job> = vec![];
        let planning_horizon = TimeWindow { start: 0.0, end: 7.0 * 24.0 * 3600.0 };
        
        // Test find_optimal_affinity_start_date with empty jobs
        let start_date = find_optimal_affinity_start_date(&empty_jobs, &planning_horizon, vehicle);
        assert!(start_date.is_none());
        
        // Test evaluate_affinity_group_assignment with empty jobs
        use crate::models::problem::SimpleTransportCost;
        let durations = vec![10.0; 100];
        let distances = vec![5.0; 100];
        let transport = Arc::new(SimpleTransportCost::new(durations, distances).unwrap());
        let cost = evaluate_affinity_group_assignment(&empty_jobs, vehicle, transport.as_ref());
        assert_eq!(cost, Some(0.0));
    }
}