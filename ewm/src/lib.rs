//! EWM: the Apple 1, Replica 1, and Apple ][+ — machines, devices, and the
//! SDL frontends, built on the generic 6502 kernel in `ewm-core`.
//!
//! The file layout mirrors the original C sources: `one` and `two` each hold
//! a machine *and* its SDL loop, as `one.c` and `two.c` did. The library
//! target exists so the headless machine tests and example consoles can
//! import the machines; the `ewm` binary is a thin dispatcher over
//! `one::main`, `two::main`, and the `boo` bootloader menu.

pub mod alc;
pub mod aux;
pub mod boo;
pub mod chr;
pub mod clk;
pub mod dsk;
pub mod hdd;
pub mod one;
pub mod palette;
pub mod pia;
pub mod scr;
pub mod sdl;
pub mod snd;
pub mod tty;
pub mod two;
pub mod woz;
