//! Backend-neutral chart and sphere scenes with egui painters.
//!
//! `scene` turns loaded traces into plotted geometry (sphere points, planar
//! chart segments split at the `R = 0` boundary, grid curves). `chart` and
//! `sphere` paint that geometry with egui and report pointer interaction.

pub mod camera;
pub mod chart;
pub mod palette;
pub mod scene;
pub mod sphere;

pub use camera::Camera;
pub use chart::{ChartInteraction, ChartLabels, paint_chart};
pub use palette::{Palette, trace_color};
pub use scene::{
    ChartSegment, ChartVertex, GridDetail, PlottedSample, PlottedTrace, PointRef, SelectionState,
    chart_grid, plot_document, sphere_grid,
};
pub use sphere::{SphereInteraction, paint_sphere};
