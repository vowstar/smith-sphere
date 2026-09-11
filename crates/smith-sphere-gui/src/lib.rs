//! Native and browser application shell for SmithSphere.

mod app;
mod dialogs;
mod fonts;
mod io;
mod theme;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use app::SmithSphereApp;
