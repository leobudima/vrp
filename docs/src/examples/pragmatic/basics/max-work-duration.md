# Maximum Work Duration

This example demonstrates how to use the `maxWorkDuration` limit to control the maximum amount of time vehicles can spend on actual service work, separate from travel time, waiting time, and breaks.

## Problem

In this scenario, we have:

-   **Limited Technicians**: Field technicians with 6-hour daily work limits due to labor regulations
-   **Unlimited Technician**: Senior technician without work duration restrictions but higher cost
-   **Service Calls**: Various duration service calls that need to be efficiently scheduled

## Use Case

Maximum work duration limits are important for:

-   **Labor regulations**: Compliance with maximum allowable working hours per shift
-   **Union agreements**: Contractual limitations on billable hours
-   **Service capacity**: Ensuring technicians maintain productive time ratios
-   **Cost control**: Managing overtime and premium hour costs
-   **Work-life balance**: Preventing technician burnout through reasonable work limits

## Sample

<details>
<summary>Problem</summary>

```json
{{#include ../../../../examples/data/pragmatic/basics/max-work-duration.basic.problem.json}}
```

</details>

## Key Features

### Work Duration vs Total Duration

```json
"limits": {
  "maxDuration": 36000,        // 10 hours total (travel + work + waiting + breaks)
  "maxWorkDuration": 21600     // 6 hours actual customer service work
}
```

-   **maxDuration**: Total time from depot departure to return (includes everything)
-   **maxWorkDuration**: Only time spent on job service activities (excludes travel/waiting)

### Vehicle Type Comparison

**Limited Technicians:**

-   Lower fixed cost ($100)
-   6-hour work limit
-   Can work 10 hours total (4 hours for travel/breaks)

**Unlimited Technician:**

-   Higher fixed cost ($120)
-   No work duration limit
-   Can work up to 10 hours total duration

## Service Time Calculation

In the example, service calls have durations:

-   Service call 1: 3600 seconds (1 hour)
-   Service call 2: 2400 seconds (40 minutes)
-   Service call 3: 1800 seconds (30 minutes)
-   Service call 4: 2700 seconds (45 minutes)
-   Service call 5: 3300 seconds (55 minutes)
-   Service call 6: 1500 seconds (25 minutes)
-   Service call 7: 2100 seconds (35 minutes)
-   Service call 8: 2850 seconds (47.5 minutes)

Total service time: 20,250 seconds ≈ 5.6 hours

## Expected Behavior

The solver will:

1. **Respect Work Limits**: Limited technicians won't exceed 6 hours of actual service time
2. **Cost Optimization**: Use limited technicians when possible due to lower fixed costs
3. **Overflow Handling**: Assign excess work to unlimited technician when work limits are reached
4. **Efficient Routing**: Minimize travel time to maximize productive work within limits

## Possible Solutions

**Scenario 1 - Within Limits:**

-   Limited technicians handle most jobs (staying under 6-hour work limit)
-   Unlimited technician handles overflow or provides more efficient routing

**Scenario 2 - Exceeds Limits:**

-   Some jobs assigned to unlimited technician due to work duration constraints
-   Balance between using cheaper limited resources and maintaining compliance

## Benefits

-   **Regulatory Compliance**: Ensures adherence to labor laws and union agreements
-   **Cost Control**: Optimizes use of lower-cost limited resources when possible
-   **Flexibility**: Provides escape valve through unlimited resources for peak demand
-   **Quality Assurance**: Prevents technician fatigue through reasonable work limits
-   **Transparency**: Clear separation between productive work time and total shift time
