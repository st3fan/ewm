//! The memory system, port of `mem.c`/`mem.h`: the CPU owns a `Memory`,
//! which dispatches to a base-RAM fast path (the C `cpu->ram`) and then a
//! list of regions — RAM, ROM, or an IO device — walked newest-first, the
//! C linked list's prepend order. Devices register once and receive the
//! absolute address, exactly like the C `mem_t` handlers with their `obj`
//! pointer.

use std::any::Any;
use std::marker::PhantomData;

/// An IO device mapped into the address space — the Rust shape of the C
/// `cpu_add_iom` read/write handlers plus their `void *obj`. `cycles` is
/// the CPU cycle counter at the start of the current step, which the C
/// handlers read as `cpu->counter` (speaker and paddle timestamps).
pub trait Device: Any {
    fn read(&mut self, addr: u16, cycles: u64) -> u8;
    fn write(&mut self, addr: u16, b: u8, cycles: u64);
}

/// A typed reference to a device owned by a `Memory`, for the machine to
/// reach its peripherals (`one.c` kept a `pia` pointer; this is the
/// ownership-safe equivalent).
pub struct DeviceHandle<T> {
    index: usize,
    _marker: PhantomData<T>,
}

// Derived Copy/Clone would demand T: Copy; the handle is always copiable.
impl<T> Clone for DeviceHandle<T> {
    fn clone(&self) -> DeviceHandle<T> {
        *self
    }
}

impl<T> Copy for DeviceHandle<T> {}

enum Backing {
    Ram(Vec<u8>),
    Rom(Vec<u8>),
    Io(usize),
}

struct Region {
    start: u16,
    end: u16, // inclusive, as in mem_t
    backing: Backing,
}

/// A memory-access watchpoint hit: the address, whether it was a write,
/// and the value read or written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WatchHit {
    pub addr: u16,
    pub write: bool,
    pub value: u8,
}

pub struct Memory {
    base_ram: Vec<u8>,
    regions: Vec<Region>,
    devices: Vec<Box<dyn Device>>,
    /// Mirror of `cpu.counter`, stamped by `Cpu::step` before dispatch so
    /// device handlers can timestamp accesses like the C handlers reading
    /// `cpu->counter`.
    pub cycles: u64,
    /// Access watchpoints as inclusive ranges (WozBug,
    /// notes/DEBUGGING_TOOLS.md). Empty in normal operation: every bus
    /// access pays one always-false branch, measured as noise against the
    /// Dormann suite.
    watchpoints: Vec<(u16, u16)>,
    /// The first watched access since the last `clear_watch_hit` —
    /// `Cpu::step` clears per instruction and stops on a recorded hit.
    watch_hit: Option<WatchHit>,
}

impl Memory {
    /// A memory with `base_ram_size` bytes of RAM at $0000 — the fast path
    /// in `mem_get_byte`/`mem_set_byte` (`addr < cpu->ram_size`). Base RAM
    /// always wins over regions.
    pub fn new(base_ram_size: usize) -> Memory {
        Memory {
            base_ram: vec![0; base_ram_size],
            regions: Vec::new(),
            devices: Vec::new(),
            cycles: 0,
            watchpoints: Vec::new(),
            watch_hit: None,
        }
    }

    /// Watch every access to `from..=to` (endpoints in either order).
    pub fn add_watchpoint(&mut self, from: u16, to: u16) {
        let range = (from.min(to), from.max(to));
        if !self.watchpoints.contains(&range) {
            self.watchpoints.push(range);
        }
    }

    /// Remove a watchpoint previously added with the same range.
    pub fn remove_watchpoint(&mut self, from: u16, to: u16) {
        let range = (from.min(to), from.max(to));
        self.watchpoints.retain(|&r| r != range);
    }

    pub fn watchpoints(&self) -> &[(u16, u16)] {
        &self.watchpoints
    }

    /// Whether any watchpoints exist — `Cpu::step` skips its per-
    /// instruction watch bookkeeping entirely when not.
    pub fn watching(&self) -> bool {
        !self.watchpoints.is_empty()
    }

    pub fn clear_watchpoints(&mut self) {
        self.watchpoints.clear();
        self.watch_hit = None;
    }

    /// Take the recorded hit, clearing it.
    pub fn take_watch_hit(&mut self) -> Option<WatchHit> {
        self.watch_hit.take()
    }

    fn watched(&self, addr: u16) -> bool {
        self.watchpoints
            .iter()
            .any(|&(a, b)| addr >= a && addr <= b)
    }

    fn record_watch(&mut self, addr: u16, write: bool, value: u8) {
        if self.watch_hit.is_none() && self.watched(addr) {
            self.watch_hit = Some(WatchHit { addr, write, value });
        }
    }

    /// Read access to base RAM, for renderers that scan the text and hires
    /// pages directly (the C renderers read `cpu->ram`).
    pub fn ram(&self) -> &[u8] {
        &self.base_ram
    }

    fn add_region(&mut self, start: u16, end: u16, backing: Backing) {
        // Prepend, so regions added later shadow earlier ones — the C
        // list-prepend order that --memory relies on.
        self.regions.insert(
            0,
            Region {
                start,
                end,
                backing,
            },
        );
    }

    /// Add a RAM region (`cpu_add_ram_data`).
    pub fn add_ram(&mut self, start: u16, data: Vec<u8>) {
        let end = start.wrapping_add(data.len() as u16).wrapping_sub(1);
        self.add_region(start, end, Backing::Ram(data));
    }

    /// Add a ROM region (`cpu_add_rom_data`): reads only, writes swallowed.
    pub fn add_rom(&mut self, start: u16, data: Vec<u8>) {
        let end = start.wrapping_add(data.len() as u16).wrapping_sub(1);
        self.add_region(start, end, Backing::Rom(data));
    }

    /// Add an IO device over `start..=end` (`cpu_add_iom`). The returned
    /// handle gives the machine typed access to the device afterwards.
    pub fn add_device<T: Device>(&mut self, start: u16, end: u16, device: T) -> DeviceHandle<T> {
        let index = self.devices.len();
        self.devices.push(Box::new(device));
        self.add_region(start, end, Backing::Io(index));
        DeviceHandle {
            index,
            _marker: PhantomData,
        }
    }

    /// Map an additional address range onto an already-added device — the
    /// language card covers both its `$C08x` switches and `$D000-$FFFF`.
    pub fn map_device<T: Device>(&mut self, handle: DeviceHandle<T>, start: u16, end: u16) {
        self.add_region(start, end, Backing::Io(handle.index));
    }

    pub fn device<T: Device>(&self, handle: DeviceHandle<T>) -> &T {
        (&*self.devices[handle.index] as &dyn Any)
            .downcast_ref::<T>()
            .expect("device handle type mismatch")
    }

    pub fn device_mut<T: Device>(&mut self, handle: DeviceHandle<T>) -> &mut T {
        (&mut *self.devices[handle.index] as &mut dyn Any)
            .downcast_mut::<T>()
            .expect("device handle type mismatch")
    }

    /// Load bytes into memory through the normal write path, like the test
    /// harnesses poking `cpu_memory_set_byte` in a loop.
    pub fn load(&mut self, addr: u16, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            self.write(addr.wrapping_add(i as u16), *b);
        }
    }

    /// Port of `mem_get_byte`: base-RAM fast path, then the region walk.
    /// Unmapped reads return 0.
    pub fn read(&mut self, addr: u16) -> u8 {
        if !self.watchpoints.is_empty() {
            let value = self.read_unwatched(addr);
            self.record_watch(addr, false, value);
            return value;
        }
        self.read_unwatched(addr)
    }

    #[inline]
    fn read_unwatched(&mut self, addr: u16) -> u8 {
        if (addr as usize) < self.base_ram.len() {
            return self.base_ram[addr as usize];
        }
        let cycles = self.cycles;
        for region in &self.regions {
            if addr >= region.start && addr <= region.end {
                return match &region.backing {
                    Backing::Ram(data) | Backing::Rom(data) => data[(addr - region.start) as usize],
                    Backing::Io(index) => self.devices[*index].read(addr, cycles),
                };
            }
        }
        0
    }

    /// Port of `mem_set_byte`: a matched ROM region swallows the write, as
    /// the C walk returns on a region without the write flag. Unmapped
    /// writes are ignored.
    pub fn write(&mut self, addr: u16, b: u8) {
        if !self.watchpoints.is_empty() {
            self.record_watch(addr, true, b);
        }
        if (addr as usize) < self.base_ram.len() {
            self.base_ram[addr as usize] = b;
            return;
        }
        let cycles = self.cycles;
        for region in &mut self.regions {
            if addr >= region.start && addr <= region.end {
                match &mut region.backing {
                    Backing::Ram(data) => data[(addr - region.start) as usize] = b,
                    Backing::Rom(_) => {}
                    Backing::Io(index) => self.devices[*index].write(addr, b, cycles),
                }
                return;
            }
        }
    }

    /// Little-endian word read (`mem_get_word` in `mem.c`). The high byte at
    /// `addr + 1` is read first, matching the C expression; `addr` wraps at
    /// `$FFFF` like C's `uint16_t` truncation.
    pub fn read_word(&mut self, addr: u16) -> u16 {
        ((self.read(addr.wrapping_add(1)) as u16) << 8) | self.read(addr) as u16
    }

    /// Little-endian word write (`mem_set_word` in `mem.c`).
    pub fn write_word(&mut self, addr: u16, w: u16) {
        self.write(addr, w as u8);
        self.write(addr.wrapping_add(1), (w >> 8) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe {
        last_read: Option<(u16, u64)>,
        last_write: Option<(u16, u8, u64)>,
        value: u8,
    }

    impl Device for Probe {
        fn read(&mut self, addr: u16, cycles: u64) -> u8 {
            self.last_read = Some((addr, cycles));
            self.value
        }

        fn write(&mut self, addr: u16, b: u8, cycles: u64) {
            self.last_write = Some((addr, b, cycles));
        }
    }

    #[test]
    fn base_ram_fast_path_wins_over_regions() {
        let mut mem = Memory::new(0x1000);
        mem.add_rom(0x0800, vec![0xee; 0x100]);
        mem.write(0x0800, 0x42);
        assert_eq!(mem.read(0x0800), 0x42, "base RAM wins, as in mem.c");
    }

    #[test]
    fn rom_reads_but_swallows_writes() {
        let mut mem = Memory::new(0x1000);
        mem.add_rom(0x2000, vec![0xaa, 0xbb]);
        assert_eq!(mem.read(0x2000), 0xaa);
        assert_eq!(mem.read(0x2001), 0xbb);
        mem.write(0x2000, 0x42);
        assert_eq!(mem.read(0x2000), 0xaa);
    }

    #[test]
    fn later_regions_shadow_earlier_ones() {
        let mut mem = Memory::new(0x1000);
        mem.add_rom(0x2000, vec![0xaa; 0x100]);
        mem.add_ram(0x2000, vec![0x11; 0x100]);
        assert_eq!(mem.read(0x2000), 0x11);
        mem.write(0x2000, 0x42);
        assert_eq!(mem.read(0x2000), 0x42);
    }

    #[test]
    fn unmapped_reads_zero_and_writes_are_ignored() {
        let mut mem = Memory::new(0x1000);
        assert_eq!(mem.read(0x8000), 0);
        mem.write(0x8000, 0x42); // must not panic
        assert_eq!(mem.read(0x8000), 0);
    }

    #[test]
    fn devices_get_absolute_addresses_and_cycles() {
        let mut mem = Memory::new(0x1000);
        let probe = mem.add_device(
            0xc000,
            0xc0ff,
            Probe {
                last_read: None,
                last_write: None,
                value: 0x99,
            },
        );
        mem.cycles = 1234;
        assert_eq!(mem.read(0xc010), 0x99);
        mem.write(0xc020, 0x42);
        let probe = mem.device(probe);
        assert_eq!(probe.last_read, Some((0xc010, 1234)));
        assert_eq!(probe.last_write, Some((0xc020, 0x42, 1234)));
    }

    #[test]
    fn one_device_can_map_multiple_ranges() {
        let mut mem = Memory::new(0x1000);
        let probe = mem.add_device(
            0xc080,
            0xc08f,
            Probe {
                last_read: None,
                last_write: None,
                value: 0x77,
            },
        );
        mem.map_device(probe, 0xd000, 0xffff);
        assert_eq!(mem.read(0xc081), 0x77);
        assert_eq!(mem.read(0xd123), 0x77);
        assert_eq!(mem.device(probe).last_read, Some((0xd123, 0)));
    }
}
