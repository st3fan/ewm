//! Enhanced //e auxiliary memory (Phase 4a): a second 48K bank for
//! `$0000-$BFFF`. Reads of `$0200-$BFFF` follow RAMRD, writes follow RAMWRT;
//! zero page and the stack (`$0000-$01FF`) stay in main until ALTZP (Phase 4b).
//! Headless — the switches are driven through the memory bus.

use ewm::two::{Two, TwoType};

const RAMRD_OFF: u16 = 0xc002;
const RAMRD_ON: u16 = 0xc003;
const RAMWRT_OFF: u16 = 0xc004;
const RAMWRT_ON: u16 = 0xc005;

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0); // write-to-set soft switch
}

#[test]
fn writes_and_reads_route_to_the_selected_bank() {
    let addr = 0x0300u16;
    let mut two = Two::new(TwoType::Apple2E).unwrap();

    // Write distinct sentinels to each bank via RAMWRT.
    set(&mut two, RAMWRT_OFF);
    two.cpu.mem.write(addr, 0x11); // -> main
    set(&mut two, RAMWRT_ON);
    two.cpu.mem.write(addr, 0x22); // -> aux

    // The banks hold what we wrote.
    assert_eq!(two.ram()[addr as usize], 0x11, "main bank");
    assert_eq!(two.aux_ram()[addr as usize], 0x22, "aux bank");

    // Reads follow RAMRD.
    set(&mut two, RAMRD_OFF);
    assert_eq!(two.cpu.mem.read(addr), 0x11, "RAMRD off reads main");
    set(&mut two, RAMRD_ON);
    assert_eq!(two.cpu.mem.read(addr), 0x22, "RAMRD on reads aux");
}

#[test]
fn ramrd_ramwrt_truth_table() {
    // Fresh banks are zeroed, so a read of the *other* bank returns 0. The
    // written value only comes back when RAMRD selects the bank RAMWRT wrote.
    let addr = 0x0400u16;
    for (wrt, rd, same_bank) in [
        (false, false, true), // main -> main
        (true, true, true),   // aux  -> aux
        (true, false, false), // aux  -> main (stale)
        (false, true, false), // main -> aux  (stale)
    ] {
        let mut two = Two::new(TwoType::Apple2E).unwrap();
        set(&mut two, if wrt { RAMWRT_ON } else { RAMWRT_OFF });
        two.cpu.mem.write(addr, 0x5a);
        set(&mut two, if rd { RAMRD_ON } else { RAMRD_OFF });
        let got = two.cpu.mem.read(addr);
        let want = if same_bank { 0x5a } else { 0x00 };
        assert_eq!(got, want, "RAMWRT={wrt} RAMRD={rd}");
    }
}

#[test]
fn zero_page_and_stack_stay_in_main() {
    // $0000-$01FF ignore RAMRD/RAMWRT in Phase 4a (ALTZP is 4b).
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, RAMWRT_ON);
    two.cpu.mem.write(0x0050, 0x99); // zero page
    two.cpu.mem.write(0x01ff, 0x88); // stack
    assert_eq!(two.ram()[0x0050], 0x99, "ZP write went to main");
    assert_eq!(two.ram()[0x01ff], 0x88, "stack write went to main");
    assert_eq!(two.aux_ram()[0x0050], 0x00, "aux ZP untouched");

    set(&mut two, RAMRD_ON);
    assert_eq!(two.cpu.mem.read(0x0050), 0x99, "ZP read stays main");
    assert_eq!(two.cpu.mem.read(0x01ff), 0x88, "stack read stays main");
}

#[test]
fn rdramrd_rdramwrt_report_state() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    assert_eq!(two.cpu.mem.read(0xc013) & 0x80, 0x00); // RDRAMRD
    assert_eq!(two.cpu.mem.read(0xc014) & 0x80, 0x00); // RDRAMWRT
    set(&mut two, RAMRD_ON);
    set(&mut two, RAMWRT_ON);
    assert_eq!(
        two.cpu.mem.read(0xc013) & 0x80,
        0x80,
        "RDRAMRD reflects RAMRD"
    );
    assert_eq!(
        two.cpu.mem.read(0xc014) & 0x80,
        0x80,
        "RDRAMWRT reflects RAMWRT"
    );
}
