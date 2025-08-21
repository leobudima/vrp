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
  "tolerance": 300                    // Time tolerance in seconds (5 minutes)
}
```

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
