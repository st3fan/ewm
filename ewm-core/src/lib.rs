//! EWM core: CPU, memory system, machines, and devices — fully headless.
//!
//! This crate contains no SDL (or other frontend) dependencies. The
//! ownership chain matches the C emulator: a machine owns its `Cpu`, and
//! the `Cpu` owns the `Memory` its hardware is composed into.

pub mod alc;
pub mod chr;
pub mod cpu;
pub mod dsk;
pub mod fmt;
pub mod ins;
pub mod mem;
pub mod one;
pub mod pia;
pub mod two;
