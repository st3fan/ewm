//! The memory bus. Replaces the C `mem_t` linked list (`mem.c`/`mem.h`): each
//! machine implements `Bus` and dispatches reads/writes itself.

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, b: u8);

    /// Little-endian word read (`mem_get_word` in `mem.c`). The high byte at
    /// `addr + 1` is read first, matching the C expression; `addr` wraps at
    /// `$FFFF` like C's `uint16_t` truncation.
    fn read_word(&mut self, addr: u16) -> u16 {
        ((self.read(addr.wrapping_add(1)) as u16) << 8) | self.read(addr) as u16
    }

    /// Little-endian word write (`mem_set_word` in `mem.c`).
    fn write_word(&mut self, addr: u16, w: u16) {
        self.write(addr, w as u8);
        self.write(addr.wrapping_add(1), (w >> 8) as u8);
    }
}

/// A flat 64K RAM bus, used by the CPU tests (the C `cpu_test.c` harness adds
/// a single 64K RAM region).
pub struct TestBus {
    ram: Box<[u8; 0x10000]>,
}

impl TestBus {
    pub fn new() -> TestBus {
        TestBus {
            ram: vec![0u8; 0x10000].into_boxed_slice().try_into().unwrap(),
        }
    }

    pub fn load(&mut self, addr: u16, data: &[u8]) {
        self.ram[addr as usize..addr as usize + data.len()].copy_from_slice(data);
    }
}

impl Default for TestBus {
    fn default() -> TestBus {
        TestBus::new()
    }
}

impl Bus for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, b: u8) {
        self.ram[addr as usize] = b;
    }
}
