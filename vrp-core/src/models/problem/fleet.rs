#[cfg(test)]
#[path = "../../../tests/unit/models/problem/fleet_test.rs"]
mod fleet_test;

use crate::models::common::*;
use crate::utils::short_type_name;
use rosomaxa::prelude::Float;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

custom_dimension!(pub VehicleId typeof String);

/// Represents a cost tier with a threshold and associated cost.
#[derive(Clone, Debug)]
pub struct CostTier {
    /// The threshold value above which this tier applies.
    pub threshold: Float,
    /// The cost per unit for this tier.
    pub cost: Float,
}

/// Represents either a fixed cost or a list of tiered costs.
#[derive(Clone, Debug)]
pub enum TieredCost {
    /// Fixed cost per unit.
    Fixed(Float),
    /// List of cost tiers.
    Tiered(Vec<CostTier>),
}

impl TieredCost {
    /// Calculates the cost rate for a given total value using the appropriate tier.
    pub fn calculate_rate(&self, total_value: Float) -> Float {
        match self {
            TieredCost::Fixed(cost) => *cost,
            TieredCost::Tiered(tiers) => {
                // Find the appropriate tier based on the total value
                // Tiers should be sorted by threshold in ascending order
                let applicable_tier = tiers
                    .iter()
                    .rev() // Start from highest threshold
                    .find(|tier| total_value >= tier.threshold);
                
                applicable_tier.map(|tier| tier.cost).unwrap_or(0.0)
            }
        }
    }

    /// Creates a fixed cost.
    pub fn fixed(cost: Float) -> Self {
        TieredCost::Fixed(cost)
    }

    /// Creates a tiered cost from a list of tiers.
    pub fn tiered(mut tiers: Vec<CostTier>) -> Self {
        // Sort tiers by threshold in ascending order
        tiers.sort_by(|a, b| a.threshold.partial_cmp(&b.threshold).unwrap());
        TieredCost::Tiered(tiers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiered_cost_calculation() {
        // Test case matching our validation scenario
        let distance_tiers = vec![
            CostTier { threshold: 0.0, cost: 1.0 },
            CostTier { threshold: 5000.0, cost: 2.0 },
            CostTier { threshold: 10000.0, cost: 3.0 },
        ];
        
        let time_tiers = vec![
            CostTier { threshold: 0.0, cost: 0.5 },
            CostTier { threshold: 600.0, cost: 1.0 },
            CostTier { threshold: 1200.0, cost: 1.5 },
        ];
        
        let distance_tiered_cost = TieredCost::tiered(distance_tiers);
        let time_tiered_cost = TieredCost::tiered(time_tiers);
        
        // Test values from our validation case
        let total_distance = 7976.0;
        let total_time = 798.0;
        
        let distance_rate = distance_tiered_cost.calculate_rate(total_distance);
        let time_rate = time_tiered_cost.calculate_rate(total_time);
        
        // Verify tier selection
        assert_eq!(distance_rate, 2.0, "Distance {} should use tier rate 2.0", total_distance);
        assert_eq!(time_rate, 1.0, "Time {} should use tier rate 1.0", total_time);
        
        // Calculate expected total cost
        let distance_cost = total_distance * distance_rate;
        let time_cost = total_time * time_rate;
        let service_cost = 300.0 * time_rate; // Service uses same rate as time
        let fixed_cost = 100.0;
        
        let expected_total = fixed_cost + distance_cost + time_cost + service_cost;
        
        println!("Tiered cost calculation test:");
        println!("Distance: {} * {} = {}", total_distance, distance_rate, distance_cost);
        println!("Driving time: {} * {} = {}", total_time, time_rate, time_cost);
        println!("Service time: {} * {} = {}", 300.0, time_rate, service_cost);
        println!("Fixed: {}", fixed_cost);
        println!("Expected total: {}", expected_total);
        
        assert_eq!(expected_total, 17150.0, "Expected total cost should be 17150.0");
    }
}

/// Represents operating costs for driver and vehicle.
#[derive(Clone, Debug)]
pub struct Costs {
    /// A fixed cost to use an actor.
    pub fixed: Float,
    /// Cost per distance unit.
    pub per_distance: Float,
    /// Cost per driving time unit.
    pub per_driving_time: Float,
    /// Cost per waiting time unit.
    pub per_waiting_time: Float,
    /// Cost per service time unit.
    pub per_service_time: Float,
}

/// Represents tiered operating costs for driver and vehicle.
/// This is used alongside the regular Costs structure for backward compatibility.
#[derive(Clone, Debug)]
pub struct TieredCosts {
    /// Cost per distance unit - can be tiered.
    pub per_distance: TieredCost,
    /// Cost per driving time unit - can be tiered.
    pub per_driving_time: TieredCost,
}

/// Represents driver detail (reserved for future use).
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct DriverDetail {}

/// Represents a driver, person who drives a [`Vehicle`].
/// Reserved for future usage, e.g., to allow reusing the same vehicle more than once at different times.
pub struct Driver {
    /// Specifies operating costs for a driver.
    pub costs: Costs,

    /// Specifies tiered operating costs for a driver (optional).
    pub tiered_costs: Option<TieredCosts>,

    /// Dimensions that contain extra work requirements.
    pub dimens: Dimensions,

    /// Specifies driver details.
    pub details: Vec<DriverDetail>,
}

impl Driver {
    /// Creates an empty driver definition.
    pub(crate) fn empty() -> Self {
        Self {
            costs: Costs {
                fixed: 0.,
                per_distance: 0.,
                per_driving_time: 0.,
                per_waiting_time: 0.,
                per_service_time: 0.,
            },
            tiered_costs: None,
            dimens: Default::default(),
            details: vec![],
        }
    }
}

/// Specifies a vehicle place.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct VehiclePlace {
    /// Location of a place.
    pub location: Location,

    /// Time interval when vehicle is allowed to be at this place.
    pub time: TimeInterval,
}

/// Represents a vehicle detail (vehicle shift).
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct VehicleDetail {
    /// A place where the vehicle starts.
    pub start: Option<VehiclePlace>,

    /// A place where the vehicle ends.
    pub end: Option<VehiclePlace>,
}

/// Represents a vehicle.
#[derive(Clone, Debug)]
pub struct Vehicle {
    /// A vehicle profile.
    pub profile: Profile,

    /// Specifies operating costs for vehicle.
    pub costs: Costs,

    /// Specifies tiered operating costs for vehicle (optional).
    pub tiered_costs: Option<TieredCosts>,

    /// Dimensions that contain extra work requirements.
    pub dimens: Dimensions,

    /// Specifies vehicle details.
    pub details: Vec<VehicleDetail>,
}

/// Represents an actor detail: exact start/end location and operating time.
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ActorDetail {
    /// A place where actor's vehicle starts.
    pub start: Option<VehiclePlace>,

    /// A place where the actor's vehicle ends.
    pub end: Option<VehiclePlace>,

    /// Time window when an actor allowed working.
    pub time: TimeWindow,
}

/// Represents an actor: abstraction over vehicle and driver.
pub struct Actor {
    /// A vehicle associated within an actor.
    pub vehicle: Arc<Vehicle>,

    /// A driver associated within an actor.
    pub driver: Arc<Driver>,

    /// Specifies actor detail.
    pub detail: ActorDetail,
}

impl Debug for Actor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(short_type_name::<Self>())
            .field("vehicle", &self.vehicle.dimens.get_vehicle_id().map(|id| id.as_str()).unwrap_or("undef"))
            .finish_non_exhaustive()
    }
}

/// Represents available resources to serve jobs.
pub struct Fleet {
    /// All fleet drivers.
    pub drivers: Vec<Arc<Driver>>,

    /// All fleet vehicles.
    pub vehicles: Vec<Arc<Vehicle>>,

    /// All fleet profiles.
    pub profiles: Vec<Profile>,

    /// All fleet actors.
    pub actors: Vec<Arc<Actor>>,

    /// A grouped actors.
    pub groups: HashMap<usize, HashSet<Arc<Actor>>>,
}

impl Fleet {
    /// Creates a new instance of `Fleet`.
    pub fn new<R: Fn(&Actor) -> usize + Send + Sync>(
        drivers: Vec<Arc<Driver>>,
        vehicles: Vec<Arc<Vehicle>>,
        group_key: impl Fn(&[Arc<Actor>]) -> R,
    ) -> Fleet {
        // TODO we should also consider multiple drivers to support smart vehicle-driver assignment.
        assert_eq!(drivers.len(), 1);
        assert!(!vehicles.is_empty());

        let profiles: HashMap<usize, Profile> = vehicles.iter().map(|v| (v.profile.index, v.profile.clone())).collect();
        let mut profiles = profiles.into_iter().collect::<Vec<_>>();
        profiles.sort_by(|(a, _), (b, _)| a.cmp(b));
        let (_, profiles): (Vec<_>, Vec<_>) = profiles.into_iter().unzip();

        let actors = vehicles
            .iter()
            .flat_map(|vehicle| {
                vehicle.details.iter().map(|detail| {
                    Arc::new(Actor {
                        vehicle: vehicle.clone(),
                        driver: drivers.first().unwrap().clone(),
                        detail: ActorDetail {
                            start: detail.start.clone(),
                            end: detail.end.clone(),
                            time: TimeWindow {
                                start: detail.start.as_ref().and_then(|s| s.time.earliest).unwrap_or(0.),
                                end: detail.end.as_ref().and_then(|e| e.time.latest).unwrap_or(Float::MAX),
                            },
                        },
                    })
                })
            })
            .collect::<Vec<_>>();

        let group_key = (group_key)(&actors);
        let groups: HashMap<_, HashSet<_>> = actors.iter().cloned().fold(HashMap::new(), |mut acc, actor| {
            acc.entry((group_key)(&actor)).or_default().insert(actor.clone());
            acc
        });

        Fleet { drivers, vehicles, profiles, actors, groups }
    }
}

impl Debug for Fleet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(short_type_name::<Self>())
            .field("vehicles", &self.vehicles.len())
            .field("drivers", &self.drivers.len())
            .field("profiles", &self.profiles.len())
            .field("actors", &self.actors.len())
            .field("groups", &self.groups.len())
            .finish()
    }
}

impl PartialEq<Actor> for Actor {
    fn eq(&self, other: &Actor) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Actor {}

impl Hash for Actor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let address = self as *const Actor;
        address.hash(state);
    }
}
