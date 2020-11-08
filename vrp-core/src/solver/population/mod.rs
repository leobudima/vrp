//! Specifies population types.

mod dominance;
pub use self::dominance::DominancePopulation;

mod rosomaxa;
pub use self::rosomaxa::RosomaxaPopulation;

use crate::construction::heuristics::InsertionContext;
use crate::solver::Statistics;
use std::cmp::Ordering;

/// Represents solution in population defined as actual solution.
pub type Individual = InsertionContext;

/// A trait which models a population with individuals (solutions).
pub trait Population {
    /// Adds all individuals into the population, then sorts and shrinks population if necessary.
    /// Returns true if any of newly added individuals is considered as best known.
    fn add_all(&mut self, individuals: Vec<Individual>) -> bool;

    /// Adds an individual into the population.
    /// Returns true if newly added individual is considered as best known.
    fn add(&mut self, individual: Individual) -> bool;

    /// Compares two solutions the same way as population does.
    fn cmp(&self, a: &Individual, b: &Individual) -> Ordering;

    /// Selects parent from population based on refinement statistics.
    fn select<'a>(&'a self, statistics: &Statistics) -> Box<dyn Iterator<Item = &Individual> + 'a>;

    /// Returns subset of individuals within their rank sorted according their quality.
    fn ranked<'a>(&'a self) -> Box<dyn Iterator<Item = (&Individual, usize)> + 'a>;

    /// Returns population size.
    fn size(&self) -> usize;
}