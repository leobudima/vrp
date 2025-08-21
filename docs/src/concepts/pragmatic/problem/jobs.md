# Job

A job is used to model customer demand, additionally, with different constraints, such as time, skills, etc. A job schema
consists of the following properties:

-   **id** (required): an unique job id
-   **pickups** (optional): a list of pickup tasks
-   **deliveries** (optional): a list of delivery tasks
-   **replacements** (optional): a list of replacement tasks
-   **services** (optional): a list of service tasks
-   **skills** (optional): job skills defined by `allOf`, `oneOf` or `noneOf` conditions:
    ```json
    {{#include ../../../../../examples/data/pragmatic/basics/skills.basic.problem.json:22:29}}
    ```
    These conditions are tested against vehicle's skills.
-   **value** (optional): a value associated with the job. With `maximize-value` objective, it is used to prioritize assignment
    of specific jobs. The difference between value and order (see in `Tasks` below) is that order related logic tries to assign
    jobs with lower order in the beginning of the tour. In contrast, value related logic tries to maximize total solution value
    by prioritizing assignment value scored jobs in any position of a tour.
    See [job priorities](../../../examples/pragmatic/basics/job-priorities.md) example.
-   **group** (optional): a group name. Jobs with the same groups are scheduled in the same tour or left unassigned.
-   **compatibility** (optional): compatibility class. Jobs with different compatibility classes cannot be assigned in
    the same tour. This is useful to avoid mixing cargo, such as hazardous goods and food.
-   **affinity** (optional): affinity information for multi-day scheduling. Jobs with the same affinity are assigned to
    the same vehicle across multiple tours/days. See [affinity section](#affinity) below.
-   **sync** (optional): synchronization information for multi-technician jobs. Jobs with the same sync group require
    multiple vehicles to work together at approximately the same time. See [sync jobs section](#sync-jobs) below.

A job should have at least one task property specified.

## Tasks

A delivery, pickup, replacement and service lists specify multiple job `tasks` and at least one of such tasks has to be
defined. Each task has the following properties:

-   **places** (required): list of possible places from which only one has to be visited
-   **demand** (optional/required): a task demand. It is required for all job types, except service
-   **order** (optional): a job task assignment order which makes preferable to serve some jobs before others in the tour.
    The order property is represented as integer greater than 1, where the lower value means higher priority. By default
    its value is set to maximum.

## Places

Each `place` consists of the following properties:

-   **location** (required): a place location
-   **duration** (required): service (operational) time to serve task here (in seconds)
-   **times** (optional): time windows
-   **tag** (optional): a job place tag which will be returned within job's activity in result solution.

Multiple places on single task can help model variable job location, e.g. visit customer at different location
depending on time of the day.

## Pickup job

Pickup job is a job with `job.pickups` property specified, without `job.deliveries`:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:33:57}}
```

The vehicle picks some `good` at pickup locations, which leads to capacity consumption growth according to `job.pickups.demand`
value, and brings it till the end of the tour (or next reload). Each pickup task has its own properties such as `demand` and `places`.

## Delivery job

Delivery job is a job with `job.deliveries` property specified, without `job.pickups`:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:4:32}}
```

The vehicle picks some `goods` at the start stop, which leads to initial capacity consumption growth, and brings it to
job's locations, where capacity consumption is decreased based on `job.deliveries.demand` values. Each delivery task has
its own properties such as `demand` and `places`.

## Pickup and delivery job

Pickup and delivery job is a job with both `job.pickups` and `job.deliveries` properties specified:

```json
{{#include ../../../../../examples/data/pragmatic/simple.basic.problem.json:58:94}}
```

The vehicle picks some `goods` at one or multiple `job.pickups.location`, which leads to capacity growth, and brings
them to one or many `job.deliveries.location`. The job has the following rules:

-   all pickup/delivery tasks should be done or none of them.
-   assignment order is not defined except all pickups should be assigned before any of deliveries.
-   sum of pickup demand should be equal to sum of delivery demand

A good example of such job is a job with more than two places with variable demand:

```json
{{#include ../../../../../examples/data/pragmatic/basics/multi-job.basic.problem.json:4:55}}
```

This job contains two pickups and one delivery. Interpretation of such job can be "bring two parcels from two different
places to one single customer".

Another example is one pickup and two deliveries:

```json
{{#include ../../../../../examples/data/pragmatic/basics/multi-job.basic.problem.json:56:109}}
```

## Replacement job

A replacement job is a job with `job.replacement` property specified:

```json
{{#include ../../../../../examples/data/pragmatic/basics/multi-job.mixed.problem.json:4:28}}
```

It models an use case when something big has to be replaced at the customer's location. This task requires a new `good`
to be loaded at the beginning of the journey and old replaced one brought to journey's end.

## Service job

A service job is a job with `job.service` property specified:

```json
{{#include ../../../../../examples/data/pragmatic/basics/multi-job.mixed.problem.json:29:54}}
```

This job models some work without demand (e.g. handyman visit).

## Mixing job tasks

You can specify multiple tasks properties to get some mixed job:

```json
{{#include ../../../../../examples/data/pragmatic/basics/multi-job.mixed.problem.json:55:122}}
```

Similar pickup and delivery job, all these tasks has to be executed or none of them. The order is not specified except
pickups must be scheduled before any delivery, replacement or service.

Hint

Use `tag` property on each job place if you want to use initial solution or checker features.

## Affinity

Affinity is used for multi-day scheduling scenarios where certain jobs must be assigned to the same vehicle across
different shifts or days. This ensures consistency and continuity in service delivery.

### Schema

The affinity property has the following structure:

```json
{
	"affinity": {
		"key": "customer_123",
		"sequence": 0,
		"duration_days": 3
	}
}
```

Where:

-   **key** (required): affinity group identifier. Jobs with the same key will be assigned to the same vehicle.
-   **sequence** (required): order within the affinity group (0, 1, 2...). Lower numbers are scheduled first.
-   **duration_days** (required): expected duration in days for the entire affinity sequence.

### Example

```json
{
	"id": "maintenance_customer_123_day1",
	"deliveries": [
		{
			"places": [{ "location": [1.0, 2.0], "duration": 1800 }],
			"demand": [10]
		}
	],
	"affinity": {
		"key": "customer_123_maintenance",
		"sequence": 0,
		"duration_days": 3
	}
}
```

### Use Cases

-   **Multi-day installations**: Equipment installation requiring multiple consecutive visits
-   **Customer relationships**: Maintaining same technician for customer continuity
-   **Progressive services**: Services that build upon previous day's work
-   **Training programs**: Apprentice following same mentor across multiple days

## Sync Jobs

Sync jobs enable multiple technicians/vehicles to coordinate and work on the same job simultaneously. This is essential
for tasks requiring team coordination, specialized skills, or safety regulations.

### Schema

The sync property has the following structure:

```json
{
	"sync": {
		"key": "emergency_repair_site_A",
		"index": 0,
		"vehicles_required": 3,
		"tolerance": 300
	}
}
```

Where:

-   **key** (required): sync group identifier. Jobs with the same key must be synchronized.
-   **index** (required): unique index within the sync group (0, 1, 2...).
-   **vehicles_required** (required): total number of vehicles needed for this synchronized job (minimum 2).
-   **tolerance** (optional): time tolerance for synchronization in seconds (default: 900 = 15 minutes).

### Requirements

-   All jobs in a sync group must be assigned together or none at all (all-or-none semantics)
-   Each vehicle can have at most one job from any given sync group
-   Multiple different sync groups can be assigned to the same vehicle
-   All sync jobs must start within the specified time tolerance

### Example

Emergency repair requiring 3 specialized technicians:

```json
[
	{
		"id": "emergency_repair_electrical",
		"services": [
			{
				"places": [{ "location": [10.0, 20.0], "duration": 7200 }],
				"demand": [0]
			}
		],
		"skills": { "allOf": ["electrical"] },
		"sync": {
			"key": "emergency_site_alpha",
			"index": 0,
			"vehicles_required": 3,
			"tolerance": 300
		}
	},
	{
		"id": "emergency_repair_mechanical",
		"services": [
			{
				"places": [{ "location": [10.0, 20.0], "duration": 7200 }],
				"demand": [0]
			}
		],
		"skills": { "allOf": ["mechanical"] },
		"sync": {
			"key": "emergency_site_alpha",
			"index": 1,
			"vehicles_required": 3,
			"tolerance": 300
		}
	},
	{
		"id": "emergency_repair_safety",
		"services": [
			{
				"places": [{ "location": [10.0, 20.0], "duration": 7200 }],
				"demand": [0]
			}
		],
		"skills": { "allOf": ["safety"] },
		"sync": {
			"key": "emergency_site_alpha",
			"index": 2,
			"vehicles_required": 3,
			"tolerance": 300
		}
	}
]
```

### Use Cases

-   **Emergency repairs**: Multiple specialists working on critical infrastructure
-   **Complex installations**: Jobs requiring different skill sets working together
-   **Safety-critical operations**: Operations requiring multiple personnel for safety compliance
-   **Heavy equipment operations**: Tasks requiring multiple operators/coordinators
-   **Team-based services**: Services where team coordination improves efficiency

### Sync vs Other Features

-   **Sync vs Groups**: Groups ensure jobs are on same route; sync ensures jobs happen simultaneously
-   **Sync vs Relations**: Relations control order; sync controls timing coordination
-   **Sync vs Skills**: Skills ensure capability; sync ensures cooperation

## Related errors

-   [E1100 duplicated job ids](../errors/index.md#e1100)
-   [E1101 invalid job task demand](../errors/index.md#e1101)
-   [E1102 invalid pickup and delivery demand](../errors/index.md#e1102)
-   [E1103 invalid time windows in jobs](../errors/index.md#e1103)
-   [E1104 reserved job id is used](../errors/index.md#e1104)
-   [E1105 empty job](../errors/index.md#e1105)
-   [E1106 job has negative duration](../errors/index.md#e1106)
-   [E1107 job has negative demand](../errors/index.md#e1107)
-   [E1110 invalid sync groups](../errors/index.md#e1110)

## Examples

Please refer to [basic job usage examples](../../../examples/pragmatic/basics/job-types.md) to see how to specify problem with
different job types.
