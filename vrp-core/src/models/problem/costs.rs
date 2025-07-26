#[cfg(test)]
#[path = "../../../tests/unit/models/problem/costs_test.rs"]
mod costs_test;

use crate::models::common::*;
use crate::models::solution::{Activity, Route};
use rosomaxa::prelude::{Float, GenericError, GenericResult};
use rosomaxa::utils::CollectGroupBy;
use std::collections::HashMap;
use std::sync::Arc;

/// Specifies a travel time type.
#[derive(Copy, Clone)]
pub enum TravelTime {
    /// Arrival time type.
    Arrival(Timestamp),
    /// Departure time type
    Departure(Timestamp),
}

/// Provides the way to get cost information for specific activities done by specific actor.
pub trait ActivityCost: Send + Sync {
    /// Returns cost to perform activity.
    fn cost(&self, route: &Route, activity: &Activity, arrival: Timestamp) -> Cost {
        self.cost_with_route_totals(route, activity, arrival, None)
    }

    /// Returns cost to perform activity with optional pre-calculated route totals.
    /// If route_totals is None, will calculate them internally.
    fn cost_with_route_totals(
        &self, 
        route: &Route, 
        activity: &Activity, 
        arrival: Timestamp,
        route_totals: Option<(Distance, Duration)>
    ) -> Cost {
        let actor = route.actor.as_ref();

        let waiting = if activity.place.time.start > arrival { activity.place.time.start - arrival } else { 0. };
        let service = activity.place.duration;

        // Check if tiered costs are available for time-based calculations
        let (driver_service_rate, vehicle_service_rate, driver_waiting_rate, vehicle_waiting_rate) = 
            if actor.driver.tiered_costs.is_some() || actor.vehicle.tiered_costs.is_some() {
                // Use provided route totals or calculate them
                let (_, total_duration) = route_totals.unwrap_or_else(|| self.calculate_route_totals(route));

                let driver_service_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.driver.costs.per_service_time);
                    
                let vehicle_service_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.vehicle.costs.per_service_time);
                    
                let driver_waiting_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.driver.costs.per_waiting_time);
                    
                let vehicle_waiting_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.vehicle.costs.per_waiting_time);
                    
                (driver_service_rate, vehicle_service_rate, driver_waiting_rate, vehicle_waiting_rate)
            } else {
                // Use fixed costs
                (actor.driver.costs.per_service_time, actor.vehicle.costs.per_service_time,
                 actor.driver.costs.per_waiting_time, actor.vehicle.costs.per_waiting_time)
            };

        waiting * (driver_waiting_rate + vehicle_waiting_rate)
            + service * (driver_service_rate + vehicle_service_rate)
    }

    /// Calculates route totals for tiered cost evaluation.
    /// Default implementation should be overridden by implementations that have access to transport data.
    fn calculate_route_totals(&self, _route: &Route) -> (Distance, Duration) {
        // Default implementation returns zeros - this should be overridden
        (0.0, 0.0)
    }


    /// Estimates departure time for activity and actor at given arrival time.
    fn estimate_departure(&self, route: &Route, activity: &Activity, arrival: Timestamp) -> Timestamp;

    /// Estimates arrival time for activity and actor at given departure time.
    fn estimate_arrival(&self, route: &Route, activity: &Activity, departure: Timestamp) -> Timestamp;
}

/// An actor independent activity costs.
#[derive(Default)]
pub struct SimpleActivityCost {}

impl ActivityCost for SimpleActivityCost {
    fn estimate_departure(&self, _: &Route, activity: &Activity, arrival: Timestamp) -> Timestamp {
        arrival.max(activity.place.time.start) + activity.place.duration
    }

    fn estimate_arrival(&self, _: &Route, activity: &Activity, departure: Timestamp) -> Timestamp {
        activity.place.time.end.min(departure - activity.place.duration)
    }
}

/// A coordinated cost calculator that implements both ActivityCost and TransportCost traits
/// and shares route totals calculation between them for consistent tiered cost evaluation.
/// Includes caching to avoid recalculating route totals repeatedly.
pub struct CoordinatedCostCalculator {
    transport_cost: Arc<dyn TransportCost>,
    activity_cost: Arc<dyn ActivityCost>,
    // Cache for route totals: (route_hash, (distance, duration))
    route_cache: std::sync::Mutex<std::collections::HashMap<u64, (Distance, Duration)>>,
}

impl CoordinatedCostCalculator {
    /// Creates a new coordinated cost calculator.
    pub fn new(transport_cost: Arc<dyn TransportCost>) -> Self {
        Self {
            transport_cost,
            activity_cost: Arc::new(SimpleActivityCost::default()),
            route_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Creates a new coordinated cost calculator with custom activity cost.
    pub fn with_activity_cost(transport_cost: Arc<dyn TransportCost>, activity_cost: Arc<dyn ActivityCost>) -> Self {
        Self {
            transport_cost,
            activity_cost,
            route_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Calculates a hash for the route to use as a cache key.
    /// This is a simple hash based on the route's activity locations and actor info.
    fn calculate_route_hash(&self, route: &Route) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        
        // Hash the actor information
        route.actor.vehicle.profile.index.hash(&mut hasher);
        route.actor.vehicle.profile.scale.to_bits().hash(&mut hasher);
        
        // Hash the sequence of locations in the route
        for activity in route.tour.all_activities() {
            (activity.place.location as u64).hash(&mut hasher);
        }
        
        hasher.finish()
    }

    /// Gets the cached route totals or calculates them if not cached.
    fn get_or_calculate_route_totals(&self, route: &Route) -> (Distance, Duration) {
        let route_hash = self.calculate_route_hash(route);
        
        // Try to get from cache first
        if let Ok(cache) = self.route_cache.lock() {
            if let Some(&totals) = cache.get(&route_hash) {
                return totals;
            }
        }
        
        // Calculate new totals
        let totals = self.transport_cost.get_route_totals(route);
        
        // Cache the result
        if let Ok(mut cache) = self.route_cache.lock() {
            // Limit cache size to prevent memory growth
            if cache.len() > 1000 {
                cache.clear(); // Simple eviction strategy
            }
            cache.insert(route_hash, totals);
        }
        
        totals
    }

    /// Clears the route totals cache. Useful for testing or when memory usage is a concern.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.route_cache.lock() {
            cache.clear();
        }
    }

    /// Returns the current cache size. Useful for monitoring and testing.
    pub fn cache_size(&self) -> usize {
        self.route_cache.lock().map(|cache| cache.len()).unwrap_or(0)
    }
}

impl ActivityCost for CoordinatedCostCalculator {
    fn calculate_route_totals(&self, route: &Route) -> (Distance, Duration) {
        // Use cached route totals calculation
        self.get_or_calculate_route_totals(route)
    }

    fn cost_with_route_totals(
        &self, 
        route: &Route, 
        activity: &Activity, 
        arrival: Timestamp,
        route_totals: Option<(Distance, Duration)>
    ) -> Cost {
        // Use provided route totals or get from cache
        let route_totals = route_totals.unwrap_or_else(|| self.get_or_calculate_route_totals(route));
        
        let actor = route.actor.as_ref();
        let waiting = if activity.place.time.start > arrival { activity.place.time.start - arrival } else { 0. };
        let service = activity.place.duration;

        // Check if tiered costs are available for time-based calculations
        let (driver_service_rate, vehicle_service_rate, driver_waiting_rate, vehicle_waiting_rate) = 
            if actor.driver.tiered_costs.is_some() || actor.vehicle.tiered_costs.is_some() {
                let (_, total_duration) = route_totals;

                let driver_service_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.driver.costs.per_service_time);
                    
                let vehicle_service_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.vehicle.costs.per_service_time);
                    
                let driver_waiting_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.driver.costs.per_waiting_time);
                    
                let vehicle_waiting_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.vehicle.costs.per_waiting_time);
                    
                (driver_service_rate, vehicle_service_rate, driver_waiting_rate, vehicle_waiting_rate)
            } else {
                // Use fixed costs
                (actor.driver.costs.per_service_time, actor.vehicle.costs.per_service_time,
                 actor.driver.costs.per_waiting_time, actor.vehicle.costs.per_waiting_time)
            };

        waiting * (driver_waiting_rate + vehicle_waiting_rate)
            + service * (driver_service_rate + vehicle_service_rate)
    }

    fn estimate_departure(&self, route: &Route, activity: &Activity, arrival: Timestamp) -> Timestamp {
        self.activity_cost.estimate_departure(route, activity, arrival)
    }

    fn estimate_arrival(&self, route: &Route, activity: &Activity, departure: Timestamp) -> Timestamp {
        self.activity_cost.estimate_arrival(route, activity, departure)
    }
}

impl TransportCost for CoordinatedCostCalculator {
    fn cost(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Cost {
        let actor = route.actor.as_ref();

        let distance = self.distance(route, from, to, travel_time);
        let duration = self.duration(route, from, to, travel_time);

        // Check if tiered costs are available, otherwise use fixed costs
        let (driver_distance_rate, vehicle_distance_rate, driver_time_rate, vehicle_time_rate) = 
            if actor.driver.tiered_costs.is_some() || actor.vehicle.tiered_costs.is_some() {
                // Use cached route totals for tiered cost evaluation
                let (total_distance, total_duration) = self.get_or_calculate_route_totals(route);

                let driver_distance_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_distance.calculate_rate(total_distance))
                    .unwrap_or(actor.driver.costs.per_distance);
                    
                let vehicle_distance_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_distance.calculate_rate(total_distance))
                    .unwrap_or(actor.vehicle.costs.per_distance);
                    
                let driver_time_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.driver.costs.per_driving_time);
                    
                let vehicle_time_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.vehicle.costs.per_driving_time);
                    
                (driver_distance_rate, vehicle_distance_rate, driver_time_rate, vehicle_time_rate)
            } else {
                // Use fixed costs
                (actor.driver.costs.per_distance, actor.vehicle.costs.per_distance,
                 actor.driver.costs.per_driving_time, actor.vehicle.costs.per_driving_time)
            };

        distance * (driver_distance_rate + vehicle_distance_rate)
            + duration * (driver_time_rate + vehicle_time_rate)
    }

    fn get_route_totals(&self, route: &Route) -> (Distance, Duration) {
        // Use cached implementation
        self.get_or_calculate_route_totals(route)
    }

    fn duration_approx(&self, profile: &Profile, from: Location, to: Location) -> Duration {
        self.transport_cost.duration_approx(profile, from, to)
    }

    fn distance_approx(&self, profile: &Profile, from: Location, to: Location) -> Distance {
        self.transport_cost.distance_approx(profile, from, to)
    }

    fn duration(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Duration {
        self.transport_cost.duration(route, from, to, travel_time)
    }

    fn distance(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Distance {
        self.transport_cost.distance(route, from, to, travel_time)
    }

    fn size(&self) -> usize {
        self.transport_cost.size()
    }
}

/// Provides the way to get routing information for specific locations and actor.
pub trait TransportCost: Send + Sync {
    /// Returns time-dependent transport cost between two locations for given actor.
    fn cost(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Cost {
        let actor = route.actor.as_ref();

        let distance = self.distance(route, from, to, travel_time);
        let duration = self.duration(route, from, to, travel_time);

        // Check if tiered costs are available, otherwise use fixed costs
        let (driver_distance_rate, vehicle_distance_rate, driver_time_rate, vehicle_time_rate) = 
            if actor.driver.tiered_costs.is_some() || actor.vehicle.tiered_costs.is_some() {
                // Calculate route totals for tiered cost evaluation
                let (total_distance, total_duration) = self.get_route_totals(route);

                let driver_distance_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_distance.calculate_rate(total_distance))
                    .unwrap_or(actor.driver.costs.per_distance);
                    
                let vehicle_distance_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_distance.calculate_rate(total_distance))
                    .unwrap_or(actor.vehicle.costs.per_distance);
                    
                let driver_time_rate = actor.driver.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.driver.costs.per_driving_time);
                    
                let vehicle_time_rate = actor.vehicle.tiered_costs
                    .as_ref()
                    .map(|tc| tc.per_driving_time.calculate_rate(total_duration))
                    .unwrap_or(actor.vehicle.costs.per_driving_time);
                    
                (driver_distance_rate, vehicle_distance_rate, driver_time_rate, vehicle_time_rate)
            } else {
                // Use fixed costs
                (actor.driver.costs.per_distance, actor.vehicle.costs.per_distance,
                 actor.driver.costs.per_driving_time, actor.vehicle.costs.per_driving_time)
            };

        distance * (driver_distance_rate + vehicle_distance_rate)
            + duration * (driver_time_rate + vehicle_time_rate)
    }

    /// Gets the total distance and duration for the entire route.
    /// Default implementation calculates from all tour activities.
    fn get_route_totals(&self, route: &Route) -> (Distance, Duration) {
        let mut total_distance = 0.0;
        let mut total_duration = 0.0;

        let activities = &route.tour.all_activities().collect::<Vec<_>>();
        for window in activities.windows(2) {
            if let [from_activity, to_activity] = window {
                total_distance += self.distance_approx(&route.actor.vehicle.profile, from_activity.place.location, to_activity.place.location);
                total_duration += self.duration_approx(&route.actor.vehicle.profile, from_activity.place.location, to_activity.place.location);
            }
        }

        (total_distance, total_duration)
    }

    /// Returns time-independent travel duration between locations specific for given profile.
    fn duration_approx(&self, profile: &Profile, from: Location, to: Location) -> Duration;

    /// Returns time-independent travel distance between locations specific for given profile.
    fn distance_approx(&self, profile: &Profile, from: Location, to: Location) -> Distance;

    /// Returns time-dependent travel duration between locations specific for given actor.
    fn duration(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Duration;

    /// Returns time-dependent travel distance between locations specific for given actor.
    fn distance(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Distance;

    /// Returns size of known locations
    fn size(&self) -> usize;
}

/// A simple implementation of transport costs around a single matrix.
/// This implementation is used to support examples and simple use cases.
pub struct SimpleTransportCost {
    durations: Vec<Duration>,
    distances: Vec<Distance>,
    size: usize,
}

impl SimpleTransportCost {
    /// Creates a new instance of `SimpleTransportCost`.
    pub fn new(durations: Vec<Duration>, distances: Vec<Distance>) -> GenericResult<Self> {
        let size = (durations.len() as Float).sqrt().round() as usize;

        if (distances.len() as Float).sqrt().round() as usize != size {
            return Err("distance-duration lengths don't match".into());
        }

        Ok(Self { durations, distances, size })
    }
}

impl TransportCost for SimpleTransportCost {
    fn duration_approx(&self, _: &Profile, from: Location, to: Location) -> Duration {
        self.durations.get(from * self.size + to).copied().unwrap_or(0.)
    }

    fn distance_approx(&self, _: &Profile, from: Location, to: Location) -> Distance {
        self.distances.get(from * self.size + to).copied().unwrap_or(0.)
    }

    fn duration(&self, route: &Route, from: Location, to: Location, _: TravelTime) -> Duration {
        self.duration_approx(&route.actor.vehicle.profile, from, to)
    }

    fn distance(&self, route: &Route, from: Location, to: Location, _: TravelTime) -> Distance {
        self.distance_approx(&route.actor.vehicle.profile, from, to)
    }

    fn size(&self) -> usize {
        self.size
    }
}

/// Contains matrix routing data for specific profile and, optionally, time.
pub struct MatrixData {
    /// A routing profile index.
    pub index: usize,
    /// A timestamp for which routing info is applicable.
    pub timestamp: Option<Timestamp>,
    /// Travel durations.
    pub durations: Vec<Duration>,
    /// Travel distances.
    pub distances: Vec<Distance>,
}

impl MatrixData {
    /// Creates `MatrixData` instance.
    pub fn new(index: usize, timestamp: Option<Timestamp>, durations: Vec<Duration>, distances: Vec<Distance>) -> Self {
        Self { index, timestamp, durations, distances }
    }
}

/// A fallback for transport costs if from->to entry is not defined.
pub trait TransportFallback: Send + Sync {
    /// Returns fallback duration.
    fn duration(&self, profile: &Profile, from: Location, to: Location) -> Duration;

    /// Returns fallback distance.
    fn distance(&self, profile: &Profile, from: Location, to: Location) -> Distance;
}

/// A trivial implementation of no fallback for transport cost.
struct NoFallback;

impl TransportFallback for NoFallback {
    fn duration(&self, profile: &Profile, from: Location, to: Location) -> Duration {
        panic!("cannot get duration for {from}->{to} for {profile:?}")
    }

    fn distance(&self, profile: &Profile, from: Location, to: Location) -> Distance {
        panic!("cannot get distance for {from}->{to} for {profile:?}")
    }
}

/// Creates time agnostic or time aware routing costs based on matrix data passed.
/// Panics at runtime if given route path is not present in matrix data.
pub fn create_matrix_transport_cost(costs: Vec<MatrixData>) -> GenericResult<Arc<dyn TransportCost>> {
    create_matrix_transport_cost_with_fallback(costs, NoFallback)
}

/// Creates time agnostic or time aware routing costs based on matrix data passed using
/// a fallback function for unknown route.
pub fn create_matrix_transport_cost_with_fallback<T: TransportFallback + 'static>(
    costs: Vec<MatrixData>,
    fallback: T,
) -> GenericResult<Arc<dyn TransportCost>> {
    if costs.is_empty() {
        return Err("no matrix data found".into());
    }

    let size = (costs.first().unwrap().durations.len() as Float).sqrt().round() as usize;

    if costs.iter().any(|matrix| matrix.distances.len() != matrix.durations.len()) {
        return Err("distance and duration collections have different length".into());
    }

    if costs.iter().any(|matrix| (matrix.distances.len() as Float).sqrt().round() as usize != size) {
        return Err("distance lengths don't match".into());
    }

    if costs.iter().any(|matrix| (matrix.durations.len() as Float).sqrt().round() as usize != size) {
        return Err("duration lengths don't match".into());
    }

    Ok(if costs.iter().any(|costs| costs.timestamp.is_some()) {
        Arc::new(TimeAwareMatrixTransportCost::new(costs, size, fallback)?)
    } else {
        Arc::new(TimeAgnosticMatrixTransportCost::new(costs, size, fallback)?)
    })
}

/// A time agnostic matrix routing costs.
struct TimeAgnosticMatrixTransportCost<T: TransportFallback> {
    durations: Vec<Vec<Duration>>,
    distances: Vec<Vec<Distance>>,
    size: usize,
    fallback: T,
}

impl<T: TransportFallback> TimeAgnosticMatrixTransportCost<T> {
    /// Creates an instance of `TimeAgnosticMatrixTransportCost`.
    pub fn new(costs: Vec<MatrixData>, size: usize, fallback: T) -> Result<Self, GenericError> {
        let mut costs = costs;
        costs.sort_by(|a, b| a.index.cmp(&b.index));

        if costs.iter().any(|costs| costs.timestamp.is_some()) {
            return Err("time aware routing".into());
        }

        if (0..).zip(costs.iter().map(|c| &c.index)).any(|(a, &b)| a != b) {
            return Err("duplicate profiles can be passed only for time aware routing".into());
        }

        let (durations, distances) = costs.into_iter().fold((vec![], vec![]), |mut acc, data| {
            acc.0.push(data.durations);
            acc.1.push(data.distances);

            acc
        });

        Ok(Self { durations, distances, size, fallback })
    }
}

impl<T: TransportFallback> TransportCost for TimeAgnosticMatrixTransportCost<T> {
    fn duration_approx(&self, profile: &Profile, from: Location, to: Location) -> Duration {
        self.durations
            .get(profile.index)
            .unwrap()
            .get(from * self.size + to)
            .copied()
            .unwrap_or_else(|| self.fallback.duration(profile, from, to))
            * profile.scale
    }

    fn distance_approx(&self, profile: &Profile, from: Location, to: Location) -> Distance {
        self.distances
            .get(profile.index)
            .unwrap()
            .get(from * self.size + to)
            .copied()
            .unwrap_or_else(|| self.fallback.distance(profile, from, to))
    }

    fn duration(&self, route: &Route, from: Location, to: Location, _: TravelTime) -> Duration {
        self.duration_approx(&route.actor.vehicle.profile, from, to)
    }

    fn distance(&self, route: &Route, from: Location, to: Location, _: TravelTime) -> Distance {
        self.distance_approx(&route.actor.vehicle.profile, from, to)
    }

    fn size(&self) -> usize {
        self.size
    }
}

/// A time aware matrix costs.
struct TimeAwareMatrixTransportCost<T: TransportFallback> {
    costs: HashMap<usize, (Vec<u64>, Vec<MatrixData>)>,
    size: usize,
    fallback: T,
}

impl<T: TransportFallback> TimeAwareMatrixTransportCost<T> {
    /// Creates an instance of `TimeAwareMatrixTransportCost`.
    fn new(costs: Vec<MatrixData>, size: usize, fallback: T) -> Result<Self, GenericError> {
        if costs.iter().any(|matrix| matrix.timestamp.is_none()) {
            return Err("time-aware routing requires all matrices to have timestamp".into());
        }

        let costs = costs.into_iter().collect_group_by_key(|matrix| matrix.index);

        if costs.iter().any(|(_, matrices)| matrices.len() == 1) {
            return Err("should not use time aware matrix routing with single matrix".into());
        }

        let costs = costs
            .into_iter()
            .map(|(profile, mut matrices)| {
                matrices.sort_by(|a, b| (a.timestamp.unwrap() as u64).cmp(&(b.timestamp.unwrap() as u64)));
                let timestamps = matrices.iter().map(|matrix| matrix.timestamp.unwrap() as u64).collect();

                (profile, (timestamps, matrices))
            })
            .collect();

        Ok(Self { costs, size, fallback })
    }

    fn interpolate_duration(
        &self,
        profile: &Profile,
        from: Location,
        to: Location,
        travel_time: TravelTime,
    ) -> Duration {
        let timestamp = match travel_time {
            TravelTime::Arrival(arrival) => arrival,
            TravelTime::Departure(departure) => departure,
        };

        let (timestamps, matrices) = self.costs.get(&profile.index).unwrap();
        let data_idx = from * self.size + to;

        let duration = match timestamps.binary_search(&(timestamp as u64)) {
            Ok(matrix_idx) => matrices.get(matrix_idx).unwrap().durations.get(data_idx).copied(),
            Err(0) => matrices.first().unwrap().durations.get(data_idx).copied(),
            Err(matrix_idx) if matrix_idx == matrices.len() => {
                matrices.last().unwrap().durations.get(data_idx).copied()
            }
            Err(matrix_idx) => {
                let left_matrix = matrices.get(matrix_idx - 1).unwrap();
                let right_matrix = matrices.get(matrix_idx).unwrap();

                matrices
                    .get(matrix_idx - 1)
                    .unwrap()
                    .durations
                    .get(data_idx)
                    .zip(matrices.get(matrix_idx).unwrap().durations.get(data_idx))
                    .map(|(&left_value, &right_value)| {
                        // perform linear interpolation
                        let ratio = (timestamp - left_matrix.timestamp.unwrap())
                            / (right_matrix.timestamp.unwrap() - left_matrix.timestamp.unwrap());

                        left_value + ratio * (right_value - left_value)
                    })
            }
        }
        .unwrap_or_else(|| self.fallback.duration(profile, from, to));

        duration * profile.scale
    }

    fn interpolate_distance(
        &self,
        profile: &Profile,
        from: Location,
        to: Location,
        travel_time: TravelTime,
    ) -> Distance {
        let timestamp = match travel_time {
            TravelTime::Arrival(arrival) => arrival,
            TravelTime::Departure(departure) => departure,
        };

        let (timestamps, matrices) = self.costs.get(&profile.index).unwrap();
        let data_idx = from * self.size + to;

        match timestamps.binary_search(&(timestamp as u64)) {
            Ok(matrix_idx) => matrices.get(matrix_idx).unwrap().distances.get(data_idx),
            Err(0) => matrices.first().unwrap().distances.get(data_idx),
            Err(matrix_idx) if matrix_idx == matrices.len() => matrices.last().unwrap().distances.get(data_idx),
            Err(matrix_idx) => matrices.get(matrix_idx - 1).unwrap().distances.get(data_idx),
        }
        .copied()
        .unwrap_or_else(|| self.fallback.distance(profile, from, to))
    }
}

impl<T: TransportFallback> TransportCost for TimeAwareMatrixTransportCost<T> {
    fn duration_approx(&self, profile: &Profile, from: Location, to: Location) -> Duration {
        self.interpolate_duration(profile, from, to, TravelTime::Departure(0.))
    }

    fn distance_approx(&self, profile: &Profile, from: Location, to: Location) -> Distance {
        self.interpolate_distance(profile, from, to, TravelTime::Departure(0.))
    }

    fn duration(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Duration {
        self.interpolate_duration(&route.actor.vehicle.profile, from, to, travel_time)
    }

    fn distance(&self, route: &Route, from: Location, to: Location, travel_time: TravelTime) -> Distance {
        self.interpolate_distance(&route.actor.vehicle.profile, from, to, travel_time)
    }

    fn size(&self) -> usize {
        self.size
    }
}
