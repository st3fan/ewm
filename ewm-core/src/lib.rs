//! EWM core: the generic 6502 system kernel — CPU, memory system,
//! instruction tables, and formatters. Nothing Apple-specific lives here:
//! the machines, their devices, and the frontends are in the `ewm` crate,
//! which composes them out of this kernel (a machine owns its `Cpu`, the
//! `Cpu` owns the `Memory` its hardware registers into).

pub mod cpu;
pub mod fmt;
pub mod ins;
pub mod mem;
