//! The Apple 1 / Replica 1 machine, port of the machine half of `one.c`.
//! `One` implements `Bus` and owns RAM, ROM, and the PIA; the SDL loop half
//! of `one.c` lands in the frontend crate in Phase 4.

use crate::bus::Bus;
use crate::cpu::Model;
use crate::pia::{A1_PIA6820_ADDR, A1_PIA6820_LENGTH, Pia};

static APPLE1_ROM: &[u8] = include_bytes!("../../../src/rom/apple1.rom");
static KRUSADER_ROM: &[u8] = include_bytes!("../../../src/rom/krusader.rom");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OneModel {
    Apple1,
    Replica1,
}

pub struct One {
    pub model: OneModel,
    ram: Vec<u8>,
    rom: &'static [u8],
    rom_start: u16,
    pub pia: Pia,
    display: Vec<u8>,
}

impl One {
    /// Port of `ewm_one_init`: Apple 1 = 6502 + 8K RAM + Woz monitor ROM at
    /// $FF00; Replica 1 = 65C02 + 32K RAM + Krusader ROM at $E000. The PIA
    /// sits at $D010 on both.
    pub fn new(model: OneModel) -> One {
        match model {
            OneModel::Apple1 => One {
                model,
                ram: vec![0; 8 * 1024],
                rom: APPLE1_ROM,
                rom_start: 0xff00,
                pia: Pia::new(),
                display: Vec::new(),
            },
            OneModel::Replica1 => One {
                model,
                ram: vec![0; 32 * 1024],
                rom: KRUSADER_ROM,
                rom_start: 0xe000,
                pia: Pia::new(),
                display: Vec::new(),
            },
        }
    }

    /// The CPU model this machine is wired with, per `ewm_one_init`.
    pub fn cpu_model(&self) -> Model {
        match self.model {
            OneModel::Apple1 => Model::M6502,
            OneModel::Replica1 => Model::M65C02,
        }
    }

    /// Port of `ewm_one_keydown`: latch the key into the PIA with bit 7 set
    /// and raise IRQA1.
    pub fn key(&mut self, key: u8) {
        self.pia.set_ina(key | 0x80);
        self.pia.set_irqa1();
    }

    /// Bytes the machine wrote to the display since the last drain — the
    /// same stream `ewm_one_pia_callback` fed into the tty.
    pub fn drain_display(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.display)
    }
}

impl Bus for One {
    fn read(&mut self, addr: u16) -> u8 {
        if (addr as usize) < self.ram.len() {
            return self.ram[addr as usize];
        }
        if (A1_PIA6820_ADDR..A1_PIA6820_ADDR + A1_PIA6820_LENGTH).contains(&addr) {
            return self.pia.read(addr);
        }
        if addr >= self.rom_start {
            let offset = (addr - self.rom_start) as usize;
            if offset < self.rom.len() {
                return self.rom[offset];
            }
        }
        // Unmapped reads return 0, as mem_get_byte does when no region matches.
        0
    }

    fn write(&mut self, addr: u16, b: u8) {
        if (addr as usize) < self.ram.len() {
            self.ram[addr as usize] = b;
            return;
        }
        if (A1_PIA6820_ADDR..A1_PIA6820_ADDR + A1_PIA6820_LENGTH).contains(&addr)
            && let Some((_ddr, v)) = self.pia.write(addr, b)
        {
            // Port of ewm_one_pia_callback: the Apple 1 masks display
            // output to 7 bits; the Replica 1 does not.
            let v = if self.model == OneModel::Apple1 {
                v & 0x7f
            } else {
                v
            };
            self.display.push(v);
        }
        // ROM and unmapped writes are ignored.
    }
}
