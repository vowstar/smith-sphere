//! Platform-independent mathematics and data handling for SmithSphere.
//!
//! The crate owns the impedance and reflection-coefficient model, the sphere
//! mapping, the two planar projections, the file parsers, and the bundled
//! demonstration datasets. It contains no rendering or platform code.

pub mod complex;
pub mod dataset;
pub mod demo;
pub mod expression;
pub mod format;
pub mod impedance;
pub mod parse;
pub mod sphere;

pub use complex::Complex;
pub use dataset::{DataSource, Document, PortVariant, Sample, Trace, TraceOrigin};
pub use impedance::{Impedance, Normalized, Reflection};
pub use parse::{FrequencyUnit, ParseError};
pub use sphere::{Region, SpherePoint, negative_chart, positive_chart, sphere_point};
