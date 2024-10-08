#[cfg(test)]
#[path = "../../tests/unit/common/routing_test.rs"]
mod routing_test;

use std::sync::Arc;
use vrp_core::custom_extra_property;
use vrp_core::models::common::Location;
use vrp_core::models::problem::{create_matrix_transport_cost, MatrixData, TransportCost};
use vrp_core::models::Extras;
use vrp_core::prelude::{GenericError, InfoLogger};
use vrp_core::utils::{Float, Timer};

custom_extra_property!(CoordIndex typeof CoordIndex);

/// Represents a coord index which can be used to analyze customer's locations.
#[derive(Clone, Default)]
pub struct CoordIndex {
    /// Keeps track of locations.
    pub locations: Vec<(i32, i32)>,
}

impl CoordIndex {
    /// Adds location to index.
    pub fn collect(&mut self, location: (i32, i32)) -> Location {
        match self.locations.iter().position(|l| l.0 == location.0 && l.1 == location.1) {
            Some(position) => position,
            _ => {
                self.locations.push(location);
                self.locations.len() - 1
            }
        }
    }

    /// Creates transport (fleet index).
    pub fn create_transport(
        &self,
        is_rounded: bool,
        logger: &InfoLogger,
    ) -> Result<Arc<dyn TransportCost>, GenericError> {
        Timer::measure_duration_with_callback(
            || {
                // NOTE changing to calculating just an upper/lower triangle of the matrix won't improve
                // performance. I think it is related to the fact that we have to change a memory access
                // pattern to less effective.
                let matrix_values = self
                    .locations
                    .iter()
                    .flat_map(|&(x1, y1)| {
                        self.locations.iter().map(move |&(x2, y2)| {
                            let x = x1 as Float - x2 as Float;
                            let y = y1 as Float - y2 as Float;
                            let value = (x * x + y * y).sqrt();

                            if is_rounded {
                                value.round()
                            } else {
                                value
                            }
                        })
                    })
                    .collect::<Vec<Float>>();

                let matrix_data = MatrixData::new(0, None, matrix_values.clone(), matrix_values);

                create_matrix_transport_cost(vec![matrix_data])
            },
            |duration| (logger)(format!("fleet index created in {}ms", duration.as_millis()).as_str()),
        )
    }
}
