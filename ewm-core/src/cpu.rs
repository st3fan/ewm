//! The 6502 CPU core. Port of `cpu.c`/`cpu.h`. As in C, the CPU owns the
//! memory system: machines build a `Memory` (RAM, ROM, devices) and hand it
//! to `Cpu::new`, and all access goes through `cpu.mem`.

use crate::ins::{Handler, INSTRUCTIONS_6502, Instruction, instructions_65c02};
use crate::mem::Memory;

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
    pub mem: Memory,
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
    /// Cycle penalties accrued by the currently executing instruction
    /// (taken branches, page-cross reads); folded into the step cost.
    pub extra_cycles: u8,
    pub strict: bool,
    /// When set, every step writes one line of disassembly + state before
    /// executing. The C only ever opened the trace file (the write path was
    /// dead code); the Rust build makes `--trace` functional using the
    /// fmt.c formatters — a documented divergence.
    pub trace: Option<Box<dyn std::io::Write>>,
    /// PC breakpoints (WozBug, notes/DEBUGGING_TOOLS.md). Empty in normal
    /// operation: `step()` pays one always-false branch per instruction,
    /// measured as noise against the Dormann suite.
    breakpoints: Vec<u16>,
    /// Set when a breakpoint hits (or `stop()` is called): `step()` becomes
    /// a no-op returning 0 cycles until `resume()`. Burst loops must check
    /// `stopped()` or they will spin.
    stopped: bool,
    /// After `resume()`, the address whose breakpoint is skipped once — so
    /// resuming from a hit executes the instruction instead of re-breaking.
    skip_breakpoint: Option<u16>,
    /// Why the last watchpoint stop happened (the instruction that touched
    /// the watched range has already executed). Cleared by `resume()`.
    watch_stop: Option<crate::mem::WatchHit>,
    pub(crate) instructions: &'static [Instruction; 256],
}

impl Cpu {
    pub fn new(model: Model, mem: Memory) -> Cpu {
        Cpu {
            model,
            mem,
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
            extra_cycles: 0,
            strict: false,
            trace: None,
            breakpoints: Vec::new(),
            stopped: false,
            skip_breakpoint: None,
            watch_stop: None,
            instructions: match model {
                Model::M6502 => &INSTRUCTIONS_6502,
                Model::M65C02 => instructions_65c02(),
            },
        }
    }

    // Stack management. The C code pokes cpu->ram directly; page 1 is plain
    // RAM on every machine, so going through the memory system is equivalent.

    pub fn push_byte(&mut self, b: u8) {
        self.mem.write(0x0100 + self.sp as u16, b);
        self.sp = self.sp.wrapping_sub(1);
    }

    pub fn push_word(&mut self, w: u16) {
        self.push_byte((w >> 8) as u8);
        self.push_byte(w as u8);
    }

    pub fn pull_byte(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.mem.read(0x0100 + self.sp as u16)
    }

    pub fn pull_word(&mut self) -> u16 {
        let w = self.pull_byte() as u16;
        w | ((self.pull_byte() as u16) << 8)
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

    pub fn reset(&mut self) {
        self.pc = self.mem.read_word(VECTOR_RES);
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

    /// The shared interrupt entry sequence: push the return address and the
    /// status byte, mask further IRQs (`I=1`), and vector. `ret` is the
    /// address `RTI` resumes at; `pushed_status` is the P byte written to the
    /// stack — B clear for a hardware IRQ/NMI, B set for BRK.
    fn enter_interrupt(
        &mut self,
        ret: u16,
        pushed_status: u8,
        vector: u16,
    ) -> Result<(), CpuError> {
        if self.strict && self.stack_free() < 3 {
            return Err(CpuError::StackOverflow);
        }
        self.push_word(ret);
        self.push_byte(pushed_status);
        self.i = 1;
        self.pc = self.mem.read_word(vector);
        Ok(())
    }

    /// A hardware maskable interrupt (IRQ): push the **exact** resume PC and
    /// the status with **B clear**, then vector through `$FFFE`. Callers gate
    /// this on the `I` flag (a real IRQ is only taken when `I==0`); taking it
    /// sets `I` so the handler is not re-entered before it `RTI`s. Distinct
    /// from BRK (see `brk_interrupt`), whose old shared code path pushed
    /// `PC+1` with B set — wrong for a real interrupt's `RTI`.
    pub fn irq(&mut self) -> Result<(), CpuError> {
        self.enter_interrupt(self.pc, self.status() & !0x10, VECTOR_IRQ)
    }

    /// A non-maskable interrupt (NMI): as `irq`, but vectors through `$FFFA`
    /// and is not gated by `I`.
    pub fn nmi(&mut self) -> Result<(), CpuError> {
        self.enter_interrupt(self.pc, self.status() & !0x10, VECTOR_NMI)
    }

    /// The BRK software interrupt: push `PC+1` (past the signature byte the
    /// instruction reserves) with **B set**, then vector through `$FFFE`.
    /// Kept deliberately distinct from a hardware `irq`.
    pub(crate) fn brk_interrupt(&mut self) -> Result<(), CpuError> {
        self.enter_interrupt(self.pc.wrapping_add(1), self.status(), VECTOR_IRQ)
    }

    /// Add a PC breakpoint (idempotent).
    pub fn add_breakpoint(&mut self, addr: u16) {
        if !self.breakpoints.contains(&addr) {
            self.breakpoints.push(addr);
        }
    }

    /// Remove a PC breakpoint.
    pub fn remove_breakpoint(&mut self, addr: u16) {
        self.breakpoints.retain(|&a| a != addr);
    }

    /// The current breakpoints, in the order they were set.
    pub fn breakpoints(&self) -> &[u16] {
        &self.breakpoints
    }

    /// True after a breakpoint hit or `stop()`: `step()` is a no-op until
    /// `resume()`.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Halt the CPU as if a breakpoint had hit (the debugger's STOP).
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    /// Clear the stopped state. The instruction at the current PC executes
    /// even if a breakpoint is set there, so `resume()` + `step()` makes
    /// progress instead of immediately re-breaking.
    pub fn resume(&mut self) {
        self.stopped = false;
        self.skip_breakpoint = Some(self.pc);
        self.watch_stop = None;
    }

    /// When the last stop came from a watchpoint: what was accessed. The
    /// triggering instruction has already executed (the stop is post-hoc,
    /// unlike a PC breakpoint's pre-execution stop).
    pub fn watch_stop(&self) -> Option<crate::mem::WatchHit> {
        self.watch_stop
    }

    /// Execute one instruction and return the cycles it took (the fixed
    /// per-opcode count from the table, as in `cpu_execute_instruction`).
    /// Returns 0 without executing when stopped on a breakpoint — burst
    /// loops that accumulate cycles must check `stopped()`.
    pub fn step(&mut self) -> u32 {
        if self.stopped {
            return 0;
        }
        if !self.breakpoints.is_empty() {
            if self.skip_breakpoint != Some(self.pc) && self.breakpoints.contains(&self.pc) {
                self.stopped = true;
                return 0;
            }
            self.skip_breakpoint = None;
        }

        // Stamp the cycle counter into the memory system so device handlers
        // can read it, the way the C handlers read cpu->counter.
        self.mem.cycles = self.counter;

        if self.trace.is_some() {
            let pc = self.pc;
            let instruction = crate::fmt::format_instruction(self);
            let state = crate::fmt::format_state(self);
            let line = format!("{pc:04X}: {instruction:<24} {state}\n");
            if let Some(trace) = &mut self.trace {
                let _ = trace.write_all(line.as_bytes());
            }
        }

        // A fresh watch scope per instruction: everything from the opcode
        // fetch on counts as this instruction's accesses (the trace
        // formatter's reads above deliberately do not).
        let watching = self.mem.watching();
        if watching {
            self.mem.take_watch_hit();
        }

        // Fetch instruction
        let instructions = self.instructions;
        let ins = &instructions[self.mem.read(self.pc) as usize];

        // Remember and advance the pc
        let pc = self.pc;
        self.pc = self.pc.wrapping_add(ins.bytes as u16);

        // Execute instruction
        self.extra_cycles = 0;
        match ins.handler {
            Handler::Implied(f) => f(self),
            Handler::Byte(f) => {
                let oper = self.mem.read(pc.wrapping_add(1));
                f(self, oper);
            }
            Handler::Word(f) => {
                let oper = self.mem.read_word(pc.wrapping_add(1));
                f(self, oper);
            }
        }

        // A watched access stops the CPU *after* the instruction that made
        // it (post-hoc, unlike the pre-execution PC breakpoint).
        if watching && let Some(hit) = self.mem.take_watch_hit() {
            self.stopped = true;
            self.watch_stop = Some(hit);
        }

        // Base cost from the table plus what the handler accrued (taken
        // branches, page-cross reads).
        let cycles = ins.cycles as u32 + self.extra_cycles as u32;
        self.counter += cycles as u64;

        cycles
    }
}

/// CPU state (notes/STATE.md §5): the registers, the flags via the packed
/// status byte, the cycle counter, and the memory system as a framed child.
/// Deliberately not written: `model` and the instruction table (construction
/// data), `strict`/`trace`/breakpoints/`stopped`/watch state (debug session
/// aids), and `extra_cycles` (transient within a step — save and restore
/// happen only at step boundaries, notes/STATE.md §5 quiescence rule).
impl crate::state::Persist for Cpu {
    fn save(&self, w: &mut crate::state::Writer) {
        w.put_u8(self.a);
        w.put_u8(self.x);
        w.put_u8(self.y);
        w.put_u8(self.sp);
        w.put_u8(self.status());
        w.put_u16(self.pc);
        w.put_u64(self.counter);
        w.chunk(*b"MEM ", |w| self.mem.save(w));
    }

    fn restore(&mut self, r: &mut crate::state::Reader) -> crate::state::Result<()> {
        self.a = r.get_u8()?;
        self.x = r.get_u8()?;
        self.y = r.get_u8()?;
        self.sp = r.get_u8()?;
        let status = r.get_u8()?;
        self.set_status(status);
        self.pc = r.get_u16()?;
        self.counter = r.get_u64()?;
        let mut mem = r.chunk(*b"MEM ")?;
        self.mem.restore(&mut mem)?;
        mem.done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cpu(model: Model) -> Cpu {
        // A flat 64K of RAM, as cpu_test.c sets up.
        Cpu::new(model, Memory::new(0x10000))
    }

    #[test]
    fn hardware_irq_pushes_exact_pc_b_clear_and_rti_resumes() {
        // The M1 gate (plans/20260721-01): a real IRQ vectors through $FFFE,
        // pushes the *exact* resume PC with B clear, sets I, and RTI returns
        // to the interrupted instruction. (BRK, by contrast, pushes PC+1 with
        // B set — see the next test.)
        let mut cpu = test_cpu(Model::M6502);
        cpu.mem.write(0xff00, 0x40); // handler: RTI
        cpu.mem.write(0xfffe, 0x00); // IRQ vector -> $FF00
        cpu.mem.write(0xffff, 0xff);
        cpu.mem.load(0x0400, &[0xea, 0xea]); // NOPs
        cpu.reset();
        cpu.pc = 0x0400;
        cpu.i = 0; // interrupts enabled

        cpu.step(); // NOP; pc -> $0401, the resume point
        assert_eq!(cpu.pc, 0x0401);

        cpu.irq().unwrap();
        assert_eq!(cpu.pc, 0xff00, "vectored through $FFFE");
        assert_eq!(cpu.i, 1, "IRQ masks further interrupts");
        // sp was $FF, so the stack holds $01FF=PCH, $01FE=PCL, $01FD=P.
        assert_eq!(cpu.mem.read(0x01fd) & 0x10, 0, "IRQ pushes B clear");
        assert_eq!(cpu.mem.read(0x01ff), 0x04, "pushed PCH");
        assert_eq!(cpu.mem.read(0x01fe), 0x01, "pushed PCL (exact resume PC)");

        cpu.step(); // RTI
        assert_eq!(cpu.pc, 0x0401, "RTI resumes at the interrupted PC");
        assert_eq!(cpu.sp, 0xff, "stack fully unwound");
        assert_eq!(cpu.i, 0, "RTI restores I=0");
    }

    #[test]
    fn brk_pushes_pc_plus_one_with_b_set() {
        // BRK stays distinct from a hardware IRQ: B set, and PC+1 pushed
        // (past the reserved signature byte).
        let mut cpu = test_cpu(Model::M6502);
        cpu.mem.write(0xff00, 0x40); // handler: RTI
        cpu.mem.write(0xfffe, 0x00);
        cpu.mem.write(0xffff, 0xff);
        cpu.mem.load(0x0400, &[0x00]); // BRK
        cpu.reset();
        cpu.pc = 0x0400;

        cpu.step(); // BRK
        assert_eq!(cpu.pc, 0xff00, "BRK vectors through $FFFE");
        assert_eq!(cpu.mem.read(0x01fd) & 0x10, 0x10, "BRK pushes B set");
        // step advanced pc past the opcode to $0401, and BRK pushes pc+1.
        assert_eq!(cpu.mem.read(0x01ff), 0x04, "pushed PCH");
        assert_eq!(cpu.mem.read(0x01fe), 0x02, "BRK pushes PC+1 = $0402");
    }

    #[test]
    fn breakpoints_stop_and_resume_makes_progress() {
        let mut cpu = test_cpu(Model::M6502);
        cpu.mem.load(0x0400, &[0xea, 0xea, 0xea, 0xea]); // NOPs
        cpu.reset();
        cpu.pc = 0x0400;
        cpu.add_breakpoint(0x0402);

        // Runs freely until the breakpoint address is reached.
        assert!(cpu.step() > 0);
        assert!(!cpu.stopped());
        assert_eq!(cpu.pc, 0x0401);
        assert!(cpu.step() > 0);
        // The instruction at $0402 has NOT executed: the hit is before it.
        assert_eq!(cpu.step(), 0);
        assert!(cpu.stopped());
        assert_eq!(cpu.pc, 0x0402);
        // Stopped: further steps are no-ops.
        assert_eq!(cpu.step(), 0);
        assert_eq!(cpu.pc, 0x0402);

        // Resume executes the instruction under the breakpoint instead of
        // immediately re-breaking.
        cpu.resume();
        assert!(cpu.step() > 0);
        assert_eq!(cpu.pc, 0x0403);

        // A loop back to the breakpoint hits again.
        cpu.pc = 0x0402;
        assert_eq!(cpu.step(), 0);
        assert!(cpu.stopped());

        // Removing it lets execution pass.
        cpu.resume();
        cpu.remove_breakpoint(0x0402);
        assert!(cpu.breakpoints().is_empty());
        cpu.pc = 0x0402;
        assert!(cpu.step() > 0);
    }

    #[test]
    fn watchpoints_stop_after_the_touching_instruction() {
        let mut cpu = test_cpu(Model::M6502);
        // LDA $1000 / STA $2000 / NOP
        cpu.mem
            .load(0x0400, &[0xad, 0x00, 0x10, 0x8d, 0x00, 0x20, 0xea]);
        cpu.mem.write(0x1000, 0x5a);
        cpu.reset();
        cpu.pc = 0x0400;
        cpu.mem.add_watchpoint(0x2000, 0x200f);

        assert!(cpu.step() > 0, "the $1000 read is not watched");
        assert!(!cpu.stopped());
        assert!(cpu.step() > 0, "the store executes, then the CPU stops");
        assert!(cpu.stopped());
        let hit = cpu.watch_stop().expect("watch reason recorded");
        assert_eq!((hit.addr, hit.write, hit.value), (0x2000, true, 0x5a));
        assert_eq!(cpu.mem.read(0x2000), 0x5a, "the write landed");
        assert_eq!(cpu.pc, 0x0406, "the stop is post-instruction");

        // Resume clears the reason and continues.
        cpu.resume();
        assert!(cpu.watch_stop().is_none());
        assert!(cpu.step() > 0);

        // Reads trigger too — the opcode fetch included.
        cpu.mem.clear_watchpoints();
        cpu.mem.add_watchpoint(0x0400, 0x0400);
        cpu.pc = 0x0400;
        assert!(cpu.step() > 0);
        assert!(cpu.stopped(), "executing watched code is a watched read");
        let hit = cpu.watch_stop().expect("fetch hit recorded");
        assert_eq!((hit.addr, hit.write, hit.value), (0x0400, false, 0xad));
    }

    #[test]
    fn stop_halts_like_a_breakpoint() {
        let mut cpu = test_cpu(Model::M6502);
        cpu.mem.load(0x0400, &[0xea]);
        cpu.reset();
        cpu.pc = 0x0400;
        cpu.stop();
        assert_eq!(cpu.step(), 0);
        cpu.resume();
        assert!(cpu.step() > 0);
    }

    #[test]
    fn status_pack_unpack_round_trip() {
        let mut cpu = test_cpu(Model::M6502);
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

        let mut cpu = test_cpu(Model::M6502);
        cpu.mem.load(0x0400, &[0xa9, 0x42, 0xea]); // LDA #$42, NOP
        cpu.reset();
        cpu.pc = 0x0400;

        let sink = Sink(Arc::new(Mutex::new(Vec::new())));
        cpu.trace = Some(Box::new(sink.clone()));
        cpu.step();
        cpu.step();

        let out = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert_eq!(
            out,
            "0400: LDA  #$42                A=00 X=00 Y=00 S=34 SP=FF -----I--\n\
             0402: NOP                      A=42 X=00 Y=00 S=34 SP=FF -----I--\n"
        );
    }

    #[test]
    fn state_round_trip_restores_registers_and_memory() {
        use crate::state::{Persist, Reader, Writer};

        // A machine shape with base RAM, an extra RAM region, and ROM.
        let build = || {
            let mut mem = Memory::new(0x1000);
            mem.add_ram(0x4000, vec![0; 0x100]);
            mem.add_rom(0xf000, vec![0xea; 0x100]);
            Cpu::new(Model::M6502, mem)
        };

        let mut cpu = build();
        cpu.a = 0x42;
        cpu.x = 0x11;
        cpu.y = 0x22;
        cpu.sp = 0xf0;
        cpu.pc = 0xbd00;
        cpu.set_status(0b1010_0101);
        cpu.counter = 123_456_789;
        cpu.mem.write(0x0123, 0x5a);
        cpu.mem.write(0x4042, 0xa5);
        cpu.mem.cycles = 123_456_780;

        let mut w = Writer::new();
        cpu.save(&mut w);
        let bytes = w.into_bytes();

        let mut twin = build();
        let mut r = Reader::new(&bytes);
        twin.restore(&mut r).expect("restore");
        r.done().expect("payload fully consumed");

        assert_eq!(
            (twin.a, twin.x, twin.y, twin.sp, twin.pc),
            (0x42, 0x11, 0x22, 0xf0, 0xbd00)
        );
        assert_eq!(twin.status(), cpu.status());
        assert_eq!(twin.counter, 123_456_789);
        assert_eq!(twin.mem.cycles, 123_456_780);
        assert_eq!(twin.mem.read(0x0123), 0x5a, "base RAM restored");
        assert_eq!(twin.mem.read(0x4042), 0xa5, "RAM region restored");
        assert_eq!(twin.mem.read(0xf000), 0xea, "ROM untouched");
    }

    #[test]
    fn state_restore_rejects_a_differently_shaped_machine() {
        use crate::state::{Persist, Reader, Writer};

        let cpu = Cpu::new(Model::M6502, Memory::new(0x1000));
        let mut w = Writer::new();
        cpu.save(&mut w);
        let bytes = w.into_bytes();

        // Different base RAM size: the same-configuration precondition
        // (notes/STATE.md) is checked where it is cheap.
        let mut other = Cpu::new(Model::M6502, Memory::new(0x2000));
        let err = other
            .restore(&mut Reader::new(&bytes))
            .expect_err("size mismatch rejected")
            .to_string();
        assert!(err.contains("base RAM size mismatch"), "{err}");
    }

    #[test]
    fn stack_wraparound() {
        let mut cpu = test_cpu(Model::M6502);

        // Pushing with sp at 0x00 writes $0100 and wraps sp to 0xff.
        cpu.sp = 0x00;
        cpu.push_byte(0x42);
        assert_eq!(cpu.sp, 0xff);
        assert_eq!(cpu.mem.read(0x0100), 0x42);

        // Pulling with sp at 0xff wraps back to 0x00.
        let b = cpu.pull_byte();
        assert_eq!(b, 0x42);
        assert_eq!(cpu.sp, 0x00);

        // A word pushed across the wrap point round-trips.
        cpu.sp = 0x00;
        cpu.push_word(0xbeef);
        assert_eq!(cpu.sp, 0xfe);
        assert_eq!(cpu.pull_word(), 0xbeef);
        assert_eq!(cpu.sp, 0x00);
    }
}
