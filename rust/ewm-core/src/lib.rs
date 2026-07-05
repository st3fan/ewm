//! EWM core: CPU, bus, machines, and devices — fully headless.
//!
//! This crate contains no SDL (or other frontend) dependencies. Modules are
//! added phase by phase; see REWRITE.md at the repository root.

pub mod bus;
pub mod cpu;
pub mod ins;
