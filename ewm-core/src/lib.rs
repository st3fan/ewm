//! EWM core: CPU, bus, machines, and devices — fully headless.
//!
//! This crate contains no SDL (or other frontend) dependencies. Modules are
//! added phase by phase; see REWRITE.md at the repository root.

pub mod alc;
pub mod bus;
pub mod chr;
pub mod cpu;
pub mod dsk;
pub mod fmt;
pub mod ins;
pub mod one;
pub mod pia;
pub mod two;
