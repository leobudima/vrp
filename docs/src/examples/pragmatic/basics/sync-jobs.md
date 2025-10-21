# Synchronized Jobs

This example demonstrates how to use synchronized jobs for scenarios where multiple technicians/vehicles need to coordinate and work on the same job simultaneously.

## Problem

In this scenario, we have:

-   **Emergency Repair**: Critical infrastructure repair requiring 3 specialists (electrical, mechanical, safety) working together
-   **Complex Installation**: HVAC and plumbing work requiring 2 specialists coordinating their efforts
-   **Regular Maintenance**: Standard single-technician jobs that can be scheduled independently

## Use Case

Synchronized jobs are essential for:

-   **Emergency repairs**: Multiple specialists working on critical infrastructure failures
-   **Complex installations**: Jobs requiring different skill sets working together for efficiency and safety
-   **Safety-critical operations**: Operations requiring multiple personnel for safety compliance
-   **Heavy equipment operations**: Tasks requiring multiple operators/coordinators
-   **Team-based services**: Services where team coordination significantly improves efficiency

## Sample

<details>
<summary>Problem</summary>

```json
{{#include ../../../../examples/data/pragmatic/basics/sync-jobs.basic.problem.json}}
```

</details>

## Key Features

### Sync Configuration

Each job in a sync group has:

```json
"sync": {
  "key": "emergency_site_alpha",      // Sync group identifier
  "index": 0,                         // Unique index within group (0, 1, 2...)
  "vehicles_required": 3,             // Total vehicles needed for this job
  "tolerance": 300                    // Time tolerance in seconds (optional, defaults to 900 = 15 minutes)
}
```

**Properties:**

-   `key` (required): String identifier for the sync group. All jobs with the same key must be synchronized.
-   `index` (required): Unique integer index within the group (0, 1, 2, ..., n-1). Must form a complete sequence.
-   `vehicles_required` (required): Total number of vehicles needed. Must be at least 2.
-   `tolerance` (optional): Maximum time difference in seconds between synchronized job start times. Defaults to 900 seconds (15 minutes).

### Specialist Vehicle Setup

Different vehicle types with specific skills:

```json
{
	"typeId": "electrical_specialist",
	"skills": ["electrical", "general"]
	// ... other vehicle properties
}
```

### Skills Integration

Jobs require specific skills while being part of sync groups:

```json
{
	"id": "emergency_repair_electrical",
	"skills": { "allOf": ["electrical"] },
	"sync": {
		"key": "emergency_site_alpha",
		"index": 0,
		"vehicles_required": 3,
		"tolerance": 300
	}
}
```

## Sync Constraints

The solver enforces these rules:

1. **All-or-None**: All jobs in a sync group must be assigned together, or none at all
2. **One per Route**: Each vehicle can have at most one job from any given sync group
3. **Multiple Groups**: A vehicle can participate in multiple different sync groups
4. **Time Tolerance**: All sync jobs must start within the specified time tolerance

## Validation Rules

When defining synchronized jobs, the following validation rules apply:

### Group Completeness

-   **Exact Cardinality**: Each sync group must have exactly `vehicles_required` jobs defined. If you specify `vehicles_required: 3`, you must define exactly 3 jobs with that sync key.
-   **Sequential Indices**: Job indices must form a complete sequence starting from 0. For a group requiring 3 vehicles, indices must be 0, 1, and 2 (no gaps, no duplicates).
-   **Minimum Size**: At least 2 vehicles are required for synchronization (`vehicles_required >= 2`).

### Job Consistency

All jobs within the same sync group must have identical:

-   **Locations**: All jobs must be at the same location
-   **Time Windows**: All jobs must have the same time window constraints
-   **Service Durations**: All jobs must have the same duration
-   **Demands**: All jobs must have identical demand values

**Exception**: Jobs can have **different skills** to allow for complementary specialists (e.g., electrical + mechanical + safety).

### Feature Compatibility

Jobs in the same sync group must be compatible with other constraint features:

-   **Job Groups**: If any job has a `group` property, all sync jobs in that group must have the same `group` value
-   **Affinity**: If any job has an `affinity` property, all sync jobs must have the same affinity key and attributes
-   **Compatibility**: If any job has a `compatibility` property, all sync jobs must have the same compatibility value

## Timing Details

### Service Start Synchronization

Synchronization is based on **service start time**, not arrival time:

-   **Service Start** = max(arrival_time, time_window_earliest)
-   Vehicles may arrive at slightly different times but must start working within the tolerance window
-   This ensures that all technicians begin their coordinated work simultaneously

### Tolerance Calculation

-   The effective tolerance is the **minimum** of all tolerance values in the sync group
-   If jobs specify different tolerances (not recommended), the strictest tolerance applies
-   Example: If job A has `tolerance: 600` and job B has `tolerance: 300`, the effective tolerance is 300 seconds

## Expected Behavior

For the emergency repair (3-vehicle sync):

-   Electrical, mechanical, and safety specialists arrive within 5 minutes of each other
-   All three jobs are assigned to different vehicles
-   If any job cannot be assigned, none of the three are assigned

For the complex installation (2-vehicle sync):

-   Plumbing and HVAC specialists coordinate with 10-minute tolerance
-   Both jobs assigned to different vehicles or both remain unassigned

## Benefits

-   **Safety Compliance**: Ensures required personnel are present for safety-critical operations
-   **Efficiency**: Coordinated teams complete complex tasks faster than sequential work
-   **Quality**: Multiple specialists can address different aspects simultaneously
-   **Risk Management**: Critical operations have appropriate backup and oversight
-   **Customer Satisfaction**: Faster completion times for complex installations

## Troubleshooting

### Common Validation Errors

**Error E1110: Invalid sync groups**

This error occurs when sync group validation fails. Common causes:

1. **Incorrect cardinality**: Number of jobs doesn't match `vehicles_required`
    - ✗ Wrong: `vehicles_required: 3` but only 2 jobs defined with that sync key
    - ✓ Correct: Define exactly 3 jobs with indices 0, 1, 2

2. **Invalid indices**: Indices are not sequential or have gaps
    - ✗ Wrong: Indices 0, 1, 3 (missing index 2)
    - ✗ Wrong: Indices 0, 0, 1 (duplicate index 0)
    - ✓ Correct: Indices 0, 1, 2

3. **Inconsistent task definitions**: Jobs have different locations, times, or demands
    - ✗ Wrong: Jobs at different locations `[10.0, 10.0]` vs `[11.0, 11.0]`
    - ✓ Correct: All jobs at the same location `[10.0, 10.0]`

4. **Incompatible attributes**: Jobs have conflicting group/compatibility/affinity values
    - ✗ Wrong: Job A has `group: "morning"`, Job B has `group: "afternoon"`
    - ✓ Correct: All jobs either have no group or share the same group

### Partial Assignment Recovery

If a sync group cannot be fully assigned:

-   **Partial assignments are cleared**: If some (but not all) jobs in a sync group are assigned and the solver cannot complete the group, all partial assignments are removed
-   **All-or-none guarantee**: This ensures the integrity of the sync constraint
-   **Optimization behavior**: The solver uses penalties to discourage partial assignments and rewards complete sync groups

### Performance Considerations

-   **State Management**: The solver maintains both solution-level and route-level sync state for efficient validation
-   **Incremental Updates**: State updates are optimized to minimize cloning and unnecessary operations
-   **Time Estimation**: Multiple fallback strategies are used for robust timing validation during insertion
