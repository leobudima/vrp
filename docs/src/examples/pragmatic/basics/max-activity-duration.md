# Maximum Activity Duration

This example demonstrates how to use the `maxActivityDuration` limit to control the maximum amount of time vehicles can spend on a route, measured from arrival at the first job to departure from the last job.

## Problem

In this scenario, we have:

-   **Limited Technicians**: Field technicians with 8-hour activity duration limits due to labor regulations.
-   **Unlimited Technician**: Senior technician without activity duration restrictions but at a higher cost.
-   **Service Calls**: Various service calls that need to be efficiently scheduled.

## Use Case

Maximum activity duration limits are important for:

-   **Labor regulations**: Compliance with maximum allowable working hours per shift.
-   **Union agreements**: Contractual limitations on total on-the-job time.
-   **Cost control**: Managing overtime and ensuring efficient use of driver time.
-   **Work-life balance**: Preventing technician burnout by limiting the span of the workday.

## Sample

<details>
<summary>Problem</summary>

```json
{{#include ../../../../examples/data/pragmatic/basics/max-activity-duration.basic.problem.json}}
```

</details>

## Key Features

### Activity Duration vs. Total Duration

`maxActivityDuration` provides a way to limit the duration of work-related activities, separate from the total time the vehicle is on the road.

```json
"limits": {
  "maxDuration": 36000,        // 10 hours total shift time (includes everything)
  "maxActivityDuration": 28800     // 8 hours of activity duration
}
```

-   **maxDuration**: Total time from depot departure to depot return. This includes the initial travel to the first job and the final travel from the last job.
-   **maxActivityDuration**: The duration from arrival at the first job to departure from the last job. This includes all service times, travel between jobs, waiting times, and any breaks taken during the workday. It excludes the initial and final travel segments to and from the depot.

### Vehicle Type Comparison

**Limited Technicians:**

-   Lower fixed cost ($100)
-   8-hour activity duration limit.

**Unlimited Technician:**

-   Higher fixed cost ($120)
-   No activity duration limit.

## Expected Behavior

The solver will:

1.  **Respect Activity Duration Limits**: A limited technician's tour will not exceed 8 hours from the arrival at their first job to the departure from their last.
2.  **Cost Optimization**: Use the cheaper, limited technicians whenever their `maxActivityDuration` is not violated.
3.  **Overflow Handling**: Assign jobs to the more expensive, unlimited technician if the activity duration would exceed the limit for the regular technicians.
4.  **Efficient Routing**: Minimize travel time to fit more work within the allowed duration.

## Benefits

-   **Regulatory Compliance**: Ensures adherence to labor laws regarding the length of the workday.
-   **Cost Control**: Optimizes the use of resources based on their cost and time constraints.
-   **Flexibility**: Allows for different rules for different classes of vehicles or drivers.
-   **Transparency**: Provides a clear distinction between the total shift time and the productive, activity-related time.