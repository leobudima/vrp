# Tiered costs

This example demonstrates how to use tiered cost structures for vehicles, which allow different cost rates based on usage thresholds.

## Problem

<details>
<summary>Click to expand</summary>

```json
{{#include ../../../../../examples/data/pragmatic/basics/tiered-costs.problem.json}}
```

</details>

## Key Features

This example showcases:

### Vehicle Types with Different Calculation Modes

**Standard Vehicle (Highest Tier Mode):**
- Uses `"calculationMode": "highestTier"` (default behavior)
- Distance tiers: 0.003 (0-100km), 0.002 (100-200km), 0.001 (200km+)
- Time tiers: 0.05 (0-1h), 0.04 (1-2h), 0.03 (2h+)
- For a 150km, 2.5h route: cost = 150 × 0.002 + 2.5 × 3600 × 0.03 = 0.3 + 270 = 270.3

**Premium Vehicle (Cumulative Mode):**
- Uses `"calculationMode": "cumulative"` (progressive calculation)
- Distance tiers: 0.0025 (0-150km), 0.0015 (150-300km), 0.001 (300km+)
- Time tiers: 0.04 (0-1h), 0.03 (1-2h), 0.025 (2h+)
- For a 200km, 2.5h route: 
  - Distance: 150 × 0.0025 + 50 × 0.0015 = 0.375 + 0.075 = 0.45
  - Time: 3600 × 0.04 + 3600 × 0.03 + 1800 × 0.025 = 144 + 108 + 45 = 297
  - Total: 0.45 + 297 = 297.45

### Tiered Cost Structure

Each tier is defined by:
- **threshold**: The minimum value (distance in km or time in seconds) for this tier to apply
- **cost**: The rate per unit for this tier

### Calculation Modes

- **highestTier** (default): Uses the rate of the highest applicable tier for the entire amount
- **cumulative**: Applies each tier progressively, calculating costs for each tier segment

## Use Cases

Tiered costs are useful for modeling:

1. **Volume Discounts**: Lower rates for longer distances/times
2. **Progressive Pricing**: Different rates for different usage levels  
3. **Fuel Efficiency**: Better rates at optimal usage ranges
4. **Driver Compensation**: Different hourly rates based on total hours
5. **Maintenance Costs**: Varying costs based on vehicle usage intensity

## Benefits

- **Realistic Cost Modeling**: Better reflects real-world pricing structures
- **Flexible Pricing**: Support for both simple volume discounts and complex progressive rates
- **Backward Compatibility**: Existing fixed-cost configurations continue to work
- **Mixed Fleets**: Different vehicles can use different calculation modes within the same problem

## Configuration Guidelines

1. **Always include a tier with threshold 0** - this is the base rate
2. **Tiers are automatically sorted** by threshold in ascending order
3. **Choose calculation mode** based on your business model:
   - Use `"highestTier"` for simple volume discount structures
   - Use `"cumulative"` for progressive rate structures
4. **Test with realistic data** to ensure the cost structure produces expected results