//! The Apple 1 / Replica 1 machine, port of the machine half of `one.c`.
//! Like `ewm_one_init`, the machine owns the CPU and composes its hardware —
//! RAM, ROM, and the PIA — as memory regions; it knows nothing about
//! dispatch.

use crate::cpu::{Cpu, Model};
use crate::mem::{DeviceHandle, Memory};
use crate::pia::{A1_PIA6820_ADDR, A1_PIA6820_LENGTH, Pia};

static APPLE1_ROM: &[u8] = include_bytes!("../../rom/apple1.rom");
static KRUSADER_ROM: &[u8] = include_bytes!("../../rom/krusader.rom");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OneModel {
    Apple1,
    Replica1,
}

pub struct One {
    pub model: OneModel,
    pub cpu: Cpu,
    pia: DeviceHandle<Pia>,
}

impl One {
    /// Port of `ewm_one_init`: Apple 1 = 6502 + 8K RAM + Woz monitor ROM at
    /// $FF00; Replica 1 = 65C02 + 32K RAM + Krusader ROM at $E000. The PIA
    /// sits at $D010 on both.
    pub fn new(model: OneModel) -> One {
        let (cpu_model, ram_size, rom, rom_start) = match model {
            OneModel::Apple1 => (Model::M6502, 8 * 1024, APPLE1_ROM, 0xff00),
            OneModel::Replica1 => (Model::M65C02, 32 * 1024, KRUSADER_ROM, 0xe000),
        };
        let mut mem = Memory::new(ram_size);
        mem.add_rom(rom_start, rom.to_vec());
        let pia = mem.add_device(
            A1_PIA6820_ADDR,
            A1_PIA6820_ADDR + A1_PIA6820_LENGTH - 1,
            Pia::new(),
        );
        One {
            model,
            cpu: Cpu::new(cpu_model, mem),
            pia,
        }
    }

    /// Port of `ewm_one_keydown`: latch the key into the PIA with bit 7 set
    /// and raise IRQA1.
    pub fn key(&mut self, key: u8) {
        let pia = self.cpu.mem.device_mut(self.pia);
        pia.set_ina(key | 0x80);
        pia.set_irqa1();
    }

    /// Bytes the machine wrote to the display since the last drain — the
    /// same stream `ewm_one_pia_callback` fed into the tty, including its
    /// model check: the Apple 1 masks display output to 7 bits.
    pub fn drain_display(&mut self) -> Vec<u8> {
        let model = self.model;
        self.cpu
            .mem
            .device_mut(self.pia)
            .drain_out()
            .into_iter()
            .map(|(_ddr, v)| {
                if model == OneModel::Apple1 {
                    v & 0x7f
                } else {
                    v
                }
            })
            .collect()
    }

    /// Add an extra RAM region (`--memory ram:addr:path`). Like the C
    /// linked list, regions added later are dispatched first — but base RAM
    /// wins, per the `addr < ram_size` fast path in mem.c.
    pub fn add_ram(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_ram(start, data);
    }

    /// Add an extra ROM region (`--memory rom:addr:path`).
    pub fn add_rom(&mut self, start: u16, data: Vec<u8>) {
        self.cpu.mem.add_rom(start, data);
    }
}
