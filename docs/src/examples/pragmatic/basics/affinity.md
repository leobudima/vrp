# Job Affinity

This example demonstrates how to use job affinity for multi-day scheduling scenarios where certain jobs must be assigned to the same vehicle across different shifts or days.

## Problem

In this scenario, we have:

-   **Customer Alpha**: 3-day installation project requiring the same technician across all days
-   **Customer Beta**: 2-day maintenance project requiring consistency
-   **Regular jobs**: Standard single-day jobs that can be assigned to any available technician

## Use Case

Job affinity is essential for:

-   **Multi-day installations**: Complex equipment installations requiring multiple consecutive visits
-   **Customer relationships**: Maintaining same technician for customer continuity and relationship building
-   **Progressive services**: Services that build upon previous day's work and require knowledge continuity
-   **Training programs**: Apprentice following same mentor across multiple days

## Sample

<details>
<summary>Problem</summary>

```json
{{#include ../../../../examples/data/pragmatic/basics/affinity.basic.problem.json}}
```

</details>

## Key Features

### Affinity Configuration

Each job in an affinity group has:

```json
"affinity": {
  "key": "customer_alpha_installation",    // Group identifier
  "sequence": 0,                           // Order within group (0, 1, 2...)
  "duration_days": 3                       // Expected project duration
}
```

### Multi-shift Vehicle Setup

Vehicles have multiple shifts to handle multi-day scheduling:

```json
"shifts": [
  {
    "start": {"location": [0.0, 0.0], "earliest": "2024-07-04T08:00:00Z"},
    "end": {"location": [0.0, 0.0], "latest": "2024-07-04T19:00:00Z"}
  },
  {
    "start": {"location": [0.0, 0.0], "earliest": "2024-07-05T08:00:00Z"},
    "end": {"location": [0.0, 0.0], "latest": "2024-07-05T19:00:00Z"}
  },
  {
    "start": {"location": [0.0, 0.0], "earliest": "2024-07-06T08:00:00Z"},
    "end": {"location": [0.0, 0.0], "latest": "2024-07-06T19:00:00Z"}
  }
]
```

## Expected Behavior

The solver will ensure:

1. **Same Vehicle Assignment**: All jobs with `customer_alpha_installation` affinity are assigned to the same vehicle
2. **Sequential Order**: Jobs are scheduled according to their sequence numbers (0, 1, 2...)
3. **Optimal Resource Utilization**: Regular jobs fill remaining capacity while respecting affinity constraints
4. **Flexibility**: Different affinity groups can be assigned to different vehicles

## Benefits

-   **Customer Satisfaction**: Consistent service provider builds trust and familiarity
-   **Efficiency**: Technician familiarity with customer site and requirements reduces setup time
-   **Quality**: Continuous oversight of multi-day projects ensures better outcomes
-   **Accountability**: Clear responsibility assignment for project completion
