//! The 6502 CPU core. Port of `cpu.c`/`cpu.h`. The CPU owns no memory: all
//! access goes through a `Bus` passed into `step`/`reset`/`irq`/`nmi`.

use crate::bus::Bus;
use crate::ins::{Handler, INSTRUCTIONS_6502, Instruction, instructions_65c02};

pub const VECTOR_NMI: u16 = 0xfffa;
pub const VECTOR_RES: u16 = 0xfffc;
pub const VECTOR_IRQ: u16 = 0xfffe;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    M6502,
    M65C02,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CpuError {
    StackOverflow,
    StackUnderflow,
}

pub struct Cpu {
    pub model: Model,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    // The status flags live in separate fields, packed/unpacked only in
    // status()/set_status(). Like the C code they hold the raw masked value
    // (e.g. n is 0x80 or 0), so truthiness is `!= 0`, not `== 1`.
    pub n: u8,
    pub v: u8,
    pub b: u8,
    pub d: u8,
    pub i: u8,
    pub z: u8,
    pub c: u8,
    pub counter: u64,
    pub strict: bool,
    /// When set, every step writes one line of disassembly + state before
    /// executing. The C only ever opened the trace file (the write path was
    /// dead code); the Rust build makes `--trace` functional using the
    /// fmt.c formatters — a documented divergence.
    pub trace: Option<Box<dyn std::io::Write>>,
    pub(crate) instructions: &'static [Instruction; 256],
}

impl Cpu {
    pub fn new(model: Model) -> Cpu {
        Cpu {
            model,
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0,
            n: 0,
            v: 0,
            b: 0,
            d: 0,
            i: 0,
            z: 0,
            c: 0,
            counter: 0,
            strict: false,
            trace: None,
            instructions: match model {
                Model::M6502 => &INSTRUCTIONS_6502,
                Model::M65C02 => instructions_65c02(),
            },
        }
    }

    // Stack management. The C code pokes cpu->ram directly; page 1 is plain
    // RAM on every machine, so going through the Bus is equivalent.

    pub fn push_byte(&mut self, bus: &mut dyn Bus, b: u8) {
        bus.write(0x0100 + self.sp as u16, b);
        self.sp = self.sp.wrapping_sub(1);
    }

    pub fn push_word(&mut self, bus: &mut dyn Bus, w: u16) {
        self.push_byte(bus, (w >> 8) as u8);
        self.push_byte(bus, w as u8);
    }

    pub fn pull_byte(&mut self, bus: &mut dyn Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 + self.sp as u16)
    }

    pub fn pull_word(&mut self, bus: &mut dyn Bus) -> u16 {
        let w = self.pull_byte(bus) as u16;
        w | ((self.pull_byte(bus) as u16) << 8)
    }

    pub fn stack_free(&self) -> u8 {
        self.sp
    }

    pub fn stack_used(&self) -> u8 {
        0xff - self.sp
    }

    /// Pack the separate flag fields into a status register byte. Bits 4 and
    /// 5 (B and the unused bit) always read as set, as in `_cpu_get_status`.
    pub fn status(&self) -> u8 {
        0x30 | (((self.n != 0) as u8) << 7)
            | (((self.v != 0) as u8) << 6)
            | (((self.b != 0) as u8) << 4)
            | (((self.d != 0) as u8) << 3)
            | (((self.i != 0) as u8) << 2)
            | (((self.z != 0) as u8) << 1)
            | ((self.c != 0) as u8)
    }

    pub fn set_status(&mut self, status: u8) {
        self.n = status & (1 << 7);
        self.v = status & (1 << 6);
        self.b = status & (1 << 4);
        self.d = status & (1 << 3);
        self.i = status & (1 << 2);
        self.z = status & (1 << 1);
        self.c = status & 1;
    }

    pub fn reset(&mut self, bus: &mut dyn Bus) {
        self.pc = bus.read_word(VECTOR_RES);
        self.a = 0x00;
        self.x = 0x00;
        self.y = 0x00;
        self.n = 0;
        self.v = 0;
        self.b = 0;
        self.d = 0;
        self.i = 1;
        self.z = 0;
        self.c = 0;
        self.sp = 0xff;
    }

    pub fn irq(&mut self, bus: &mut dyn Bus) -> Result<(), CpuError> {
        if self.strict && self.stack_free() < 3 {
            return Err(CpuError::StackOverflow);
        }

        self.push_word(bus, self.pc.wrapping_add(1)); // TODO +1?? Spec says +2 but test fails then
        self.push_byte(bus, self.status());
        self.i = 1;
        self.pc = bus.read_word(VECTOR_IRQ);

        Ok(())
    }

    pub fn nmi(&mut self, bus: &mut dyn Bus) -> Result<(), CpuError> {
        if self.strict && self.stack_free() < 3 {
            return Err(CpuError::StackOverflow);
        }

        self.push_word(bus, self.pc.wrapping_add(1)); // TODO +1?? Spec says +2 but test fails then
        self.push_byte(bus, self.status());
        self.i = 1;
        self.pc = bus.read_word(VECTOR_NMI);

        Ok(())
    }

    /// Execute one instruction and return the cycles it took (the fixed
    /// per-opcode count from the table, as in `cpu_execute_instruction`).
    pub fn step(&mut self, bus: &mut dyn Bus) -> u32 {
        if self.trace.is_some() {
            let line = format!(
                "{:04X}: {:<24} {}\n",
                self.pc,
                crate::fmt::format_instruction(self, bus),
                crate::fmt::format_state(self)
            );
            if let Some(trace) = &mut self.trace {
                let _ = trace.write_all(line.as_bytes());
            }
        }

        // Fetch instruction
        let instructions = self.instructions;
        let ins = &instructions[bus.read(self.pc) as usize];

        // Remember and advance the pc
        let pc = self.pc;
        self.pc = self.pc.wrapping_add(ins.bytes as u16);

        // Execute instruction
        match ins.handler {
            Handler::Implied(f) => f(self, bus),
            Handler::Byte(f) => {
                let oper = bus.read(pc.wrapping_add(1));
                f(self, bus, oper);
            }
            Handler::Word(f) => {
                let oper = bus.read_word(pc.wrapping_add(1));
                f(self, bus, oper);
            }
        }

        self.counter += ins.cycles as u64;

        ins.cycles as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::TestBus;

    #[test]
    fn status_pack_unpack_round_trip() {
        let mut cpu = Cpu::new(Model::M6502);
        for v in 0..=255u8 {
            cpu.set_status(v);
            assert_eq!(cpu.status(), v | 0x30, "status round-trip for {v:#04x}");
        }
    }

    #[test]
    fn trace_writes_disassembly_and_state() {
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Sink {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut bus = TestBus::new();
        bus.load(0x0400, &[0xa9, 0x42, 0xea]); // LDA #$42, NOP

        let mut cpu = Cpu::new(Model::M6502);
        cpu.reset(&mut bus);
        cpu.pc = 0x0400;

        let sink = Sink(Arc::new(Mutex::new(Vec::new())));
        cpu.trace = Some(Box::new(sink.clone()));
        cpu.step(&mut bus);
        cpu.step(&mut bus);

        let out = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert_eq!(
            out,
            "0400: LDA  #$42                A=00 X=00 Y=00 S=34 SP=FF -----I--\n\
             0402: NOP                      A=42 X=00 Y=00 S=34 SP=FF -----I--\n"
        );
    }

    #[test]
    fn stack_wraparound() {
        let mut cpu = Cpu::new(Model::M6502);
        let mut bus = TestBus::new();

        // Pushing with sp at 0x00 writes $0100 and wraps sp to 0xff.
        cpu.sp = 0x00;
        cpu.push_byte(&mut bus, 0x42);
        assert_eq!(cpu.sp, 0xff);
        assert_eq!(bus.read(0x0100), 0x42);

        // Pulling with sp at 0xff wraps back to 0x00.
        let b = cpu.pull_byte(&mut bus);
        assert_eq!(b, 0x42);
        assert_eq!(cpu.sp, 0x00);

        // A word pushed across the wrap point round-trips.
        cpu.sp = 0x00;
        cpu.push_word(&mut bus, 0xbeef);
        assert_eq!(cpu.sp, 0xfe);
        assert_eq!(cpu.pull_word(&mut bus), 0xbeef);
        assert_eq!(cpu.sp, 0x00);
    }
}
