# Same Assignee Key

## Purpose

The `same_assignee_key` feature ensures that jobs with the same assignee key are assigned to the same vehicle across all routes and shifts. This is useful when specific jobs must be handled by the same resource (e.g., technician, driver) regardless of the day or route.

## Key Differences from Other Features

| Feature | Scope | Ordering | Timing | Use Case |
|---------|-------|----------|--------|----------|
| **group** | Same route only | ❌ | ❌ | Jobs must be in the same tour |
| **affinity** | Cross-route, same vehicle | ❌ (basic) or ✅ (with sequence) | ❌ (basic) or ✅ (with sequence) | Multi-day projects with optional sequential constraints |
| **same_assignee_key** | Cross-route, same vehicle | ❌ | ❌ | Simple vehicle assignment preference |

## Use Cases

- **Technician assignment**: Customer requests specific technician Alice for all their jobs
- **Driver familiarity**: Assign all deliveries for a customer to the same driver
- **Equipment tracking**: Jobs requiring specific equipment stay with the vehicle carrying it
- **Customer preference**: "I want the same person who delivered last time"

## Format

```json
{
  "id": "job1",
  "same_assignee_key": "technician_alice",
  "deliveries": [...]
}
```

### Properties

- `same_assignee_key` (optional, string): The assignee key. Jobs with the same key will be assigned to the same vehicle.

## Example

In this example, we have:
- 3 jobs for technician Alice (spread across 3 days)
- 2 jobs for technician Bob (spread across 2 days)
- 1 regular job without assignee preference

The solver will ensure:
- All Alice jobs go to the same vehicle (across different shifts)
- All Bob jobs go to the same vehicle (across different shifts)
- The regular job can go to any vehicle

```json
{{#include ../../../../../examples/data/pragmatic/basics/same-assignee.basic.problem.json}}
```

### Matrix

```json
{{#include ../../../../../examples/data/pragmatic/basics/same-assignee.basic.matrix.json}}
```

## Expected Behavior

The solver will:
1. ✅ Assign all jobs with `"technician_alice"` to the same vehicle
2. ✅ Assign all jobs with `"technician_bob"` to the same vehicle
3. ✅ Allow Alice and Bob jobs to be on different vehicles
4. ✅ Allow jobs across different shifts/days for the same assignee
5. ✅ Assign the regular job (no key) to any available vehicle

## Comparison with Affinity

### Use `same_assignee_key` when:
- You simply want jobs assigned to the same vehicle
- Order doesn't matter
- No timing constraints needed
- Simpler API preferred

### Use `affinity` (basic mode) when:
- Same as above, but you might need sequence/duration features later

### Use `affinity` (with sequence/duration) when:
- Multi-day projects with consecutive scheduling
- Jobs must happen in specific order
- Time validation between sequential jobs needed
- Project completion tracking required

## Constraints

- Jobs with the same `same_assignee_key` **must** be assigned to the same vehicle
- If a job with a specific key cannot be assigned to the designated vehicle, it will remain unassigned
- Jobs without `same_assignee_key` have no such restrictions
