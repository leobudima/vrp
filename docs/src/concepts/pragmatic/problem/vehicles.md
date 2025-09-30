# Vehicle types

A vehicle types are defined by `fleet.vehicles` property and their schema has the following properties:

-   **typeId** (required): a vehicle type id

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:100}}
```

-   **vehicleIds** (required): a list of concrete vehicle ids available for usage.

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:101:103}}
```

-   **profile** (required): a vehicle profile which is defined by two properties:
    -   **matrix** (required) : a name of matrix profile
    -   **scale** (optional): duration scale applied to all travelling times (default is 1.0)

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:104:106}}
```

-   **costs** (required): specifies how expensive is vehicle usage. It has the following properties:
    -   **fixed**: a fixed cost per vehicle tour
    -   **time**: a cost per time unit (can be fixed or tiered)
    -   **distance**: a cost per distance unit (can be fixed or tiered)
    -   **calculationMode** (optional): determines how tiered costs are calculated. Options:
        -   `"highestTier"` (default): uses the rate of the highest applicable tier for the entire amount
        -   `"cumulative"`: applies each tier progressively up to its threshold

### Tiered Costs

Both `time` and `distance` costs can be specified as either fixed values or tiered structures. Tiered costs allow different rates based on thresholds, useful for modeling volume discounts or progressive pricing.

**Fixed Cost Example:**

```json
{
	"distance": 0.002,
	"time": 0.003
}
```

**Tiered Cost Example:**

```json
{
	"distance": [
		{ "threshold": 0, "cost": 0.003 },
		{ "threshold": 100, "cost": 0.002 },
		{ "threshold": 200, "cost": 0.001 }
	],
	"time": [
		{ "threshold": 0, "cost": 0.05 },
		{ "threshold": 60, "cost": 0.04 },
		{ "threshold": 120, "cost": 0.03 }
	],
	"calculationMode": "cumulative"
}
```

**Calculation Modes:**

-   **Highest Tier** (default): For a 6-hour route with time tiers `[(0, 0.05), (60, 0.04), (120, 0.03)]`, the cost would be `6 * 60 * 0.03 = 10.8` (using the highest applicable rate).

-   **Cumulative**: For the same route, the cost would be `60 * 0.05 + 60 * 0.04 + 240 * 0.03 = 3 + 2.4 + 7.2 = 12.6` (each tier applied to its portion).

-   **shifts** (required): specify one or more vehicle shift. See detailed description below.

-   **capacity** (required): specifies vehicle capacity symmetric to job demand

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:130:132}}
```

-   **skills** (optional): vehicle skills needed by some jobs

```json
{{#include ../../../../../examples/data/pragmatic/basics/skills.basic.problem.json:131:133}}
```

-   **limits** (optional): vehicle limits. Available options:
    -   **maxDuration** (optional): max tour duration (including travel, service, waiting, and break times)
    -   **maxDistance** (optional): max tour distance
    -   **maxActivityDuration** (optional): max work duration (service time only, excluding travel and waiting)
    -   **tourSize** (optional): max amount of activities in the tour (without departure/arrival). Please note, that
        clustered activities are counted as one in case of vicinity clustering.

An example:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:99:133}}
```

## Shift

Essentially, shift specifies vehicle constraints such as time, start/end locations, etc.:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:112:129}}
```

At least one shift has to be specified. More than one vehicle shift with different times means that this vehicle can be
used more than once. This is useful for multi day scenarios. An example can be found [here](../../../examples/pragmatic/basics/multi-day.md).

Each shift can have the following properties:

-   **start** (required) specifies vehicle start place defined via location, earliest (required) and latest (optional) departure time
-   **end** (optional) specifies vehicle end place defined via location, earliest (reserved) and latest (required) arrival time.
    When omitted, then vehicle ends on last job location
-   **breaks** (optional) a list of vehicle breaks. There are two types of breaks:

    -   **required**: this break is guaranteed to be assigned at cost of flexibility. It has the following properties:
        -   `time` (required): a fixed time or time offset interval when the break should happen specified by `earliest` and `latest` properties.
            The break will be assigned not earlier, and not later than the range specified.
        -   `duration` (required): duration of the break
    -   **optional**: although such break is not guaranteed for assignment, it has some advantages over required break:
        -   arbitrary break location is supported
        -   the algorithm has more flexibility for assignment
            It is specified by:
        -   `time` (required): time window or time offset interval after which a break should happen (e.g. between 3 or 4 hours after start).
        -   `places`: list of alternative places defined by `location` (optional), `duration` (required) and `tag` (optional).
            If location of a break is omitted then break is stick to location of a job served before break.
        -   `policy` (optional): a break skip policy. Possible values:
            -   `skip-if-no-intersection`: allows to skip break if actual tour schedule doesn't intersect with vehicle time window (default)
            -   `skip-if-arrival-before-end`: allows to skip break if vehicle arrives before break's time window end.

    Please note that optional break is a soft constraint and can be unassigned in some cases due to other hard constraints, such
    as time windows. You can control its unassignment weight using specific property on `minimize-unassigned` objective.
    See example [here](../../../examples/pragmatic/basics/break.md)

    Additionally, offset time interval requires departure time optimization to be disabled explicitly (see [E1307](../errors/index.md#e1307)).

-   **reloads** (optional) a list of vehicle reloads. A reload is a place where vehicle can load new deliveries and unload
    pickups. It can be used to model multi trip routes.
    Each reload has optional and required fields:
    -   location (required): an actual place where reload activity happens
    -   duration (required): duration of reload activity
    -   times (optional): reload time windows
    -   tag (optional): a tag which will be propagated back within the corresponding reload activity in solution
    -   resourceId (optional): a shared reload resource id. It is used to limit amount of deliveries loaded at this reload.
        See examples [here](../../../examples/pragmatic/basics/reload.md).
-   **recharges** (optional, experimental) specifies recharging stations and max distance limit before recharge should happen.
    See examples [here](../../../examples/pragmatic/basics/recharge.md).

## Activity Duration Limits

The `maxActivityDuration` limit allows you to control the maximum amount of time a vehicle can spend actively working (serving customers),
separate from travel time, waiting time, and breaks. This is useful for modeling:

-   **Labor regulations**: Maximum allowable working hours per shift
-   **Service capacity**: Technician availability for actual work
-   **Productivity targets**: Ensuring vehicles maintain productive time ratios

### Activity Duration vs Total Duration

-   **maxDuration**: Total time from start to end of tour (travel + work + waiting + breaks)
-   **maxActivityDuration**: Only the time spent on actual job service activities

### Example

```json
{
	"typeId": "service_technician",
	"vehicleIds": ["tech_001", "tech_002"],
	"profile": { "matrix": "car" },
	"costs": { "fixed": 100, "distance": 0.002, "time": 0.003 },
	"shifts": [
		{
			"start": { "location": [0.0, 0.0], "earliest": "2023-07-04T09:00:00Z" },
			"end": { "location": [0.0, 0.0], "latest": "2023-07-04T18:00:00Z" }
		}
	],

	"capacity": [100],
	"limits": {
		"maxDuration": 28800, // 8 hours total tour time
		"maxActivityDuration": 21600 // 6 hours actual work time
	}
}
```

In this example:

-   Vehicle can be out for maximum 8 hours total
-   But can only do 6 hours of actual customer service work
-   Remaining 2 hours can be travel, waiting, or breaks

### Use Cases

-   **Field service**: Technicians limited by billable hours regulations
-   **Healthcare**: Medical staff working time limitations
-   **Consulting**: Professional services with specific engagement hour limits
-   **Maintenance**: Crews with physical work hour restrictions

## Related errors

-   [E1300 duplicated vehicle type ids](../errors/index.md#e1300)
-   [E1301 duplicated vehicle ids](../errors/index.md#e1301)
-   [E1302 invalid start or end times in vehicle shift](../errors/index.md#e1302)
-   [E1303 invalid break time windows in vehicle shift](../errors/index.md#e1303)
-   [E1304 invalid reload time windows in vehicle shift](../errors/index.md#e1304)
-   [E1306 time and duration costs are zeros](../errors/index.md#e1306)
-   [E1307 time offset interval for break is used with departure rescheduling](../errors/index.md#e1307)
-   [E1308 invalid vehicle reload resource](../errors/index.md#e1308)
