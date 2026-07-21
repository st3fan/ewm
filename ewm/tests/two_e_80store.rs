//! Enhanced //e 80STORE display-page routing (Phase 4c). When 80STORE
//! (`$C000`/`$C001`) is on, PAGE2 (`$C054`/`$C055`) routes text page 1
//! (`$0400-$07FF`) and — with HIRES on — hi-res page 1 (`$2000-$3FFF`)
//! between main and aux, overriding RAMRD/RAMWRT. Everything else in
//! `$0200-$BFFF` still follows RAMRD/RAMWRT. Headless — the switches are
//! driven through the memory bus.

use ewm::two::{Two, TwoType};

const STORE80_OFF: u16 = 0xc000;
const STORE80_ON: u16 = 0xc001;
const RAMRD_OFF: u16 = 0xc002;
const RAMRD_ON: u16 = 0xc003;
const RAMWRT_OFF: u16 = 0xc004;
const RAMWRT_ON: u16 = 0xc005;
const PAGE1: u16 = 0xc054;
const PAGE2: u16 = 0xc055;
const LORES: u16 = 0xc056;
const HIRES: u16 = 0xc057;

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0); // write-to-set soft switch
}

/// With RAMRD/RAMWRT pointing the "wrong" way, prove the 80STORE override
/// still lands the access in the PAGE2-selected bank.
fn assert_routes_to(two: &mut Two, addr: u16, aux_expected: bool, what: &str) {
    // Point RAMRD/RAMWRT at the *opposite* bank so any leakage is visible.
    set(two, if aux_expected { RAMWRT_OFF } else { RAMWRT_ON });
    set(two, if aux_expected { RAMRD_OFF } else { RAMRD_ON });
    two.cpu.mem.write(addr, 0x5a);
    if aux_expected {
        assert_eq!(two.aux_ram()[addr as usize], 0x5a, "{what}: write -> aux");
        assert_eq!(two.ram()[addr as usize], 0x00, "{what}: main untouched");
    } else {
        assert_eq!(two.ram()[addr as usize], 0x5a, "{what}: write -> main");
        assert_eq!(two.aux_ram()[addr as usize], 0x00, "{what}: aux untouched");
    }
    assert_eq!(
        two.cpu.mem.read(addr),
        0x5a,
        "{what}: read-back follows PAGE2"
    );
}

#[test]
fn text_page1_follows_page2_under_80store() {
    for &addr in &[0x0400u16, 0x07ff] {
        let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
        set(&mut two, STORE80_ON);

        set(&mut two, PAGE1);
        assert_routes_to(&mut two, addr, false, "80STORE+PAGE1");

        // Fresh machine so the previous sentinel does not confuse the check.
        let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
        set(&mut two, STORE80_ON);
        set(&mut two, PAGE2);
        assert_routes_to(&mut two, addr, true, "80STORE+PAGE2");
    }
}

#[test]
fn hires_page1_follows_page2_only_when_hires_on() {
    // HIRES off: $2000 ignores 80STORE/PAGE2 and follows RAMRD/RAMWRT.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    set(&mut two, STORE80_ON);
    set(&mut two, PAGE2);
    set(&mut two, LORES); // HIRES off
    set(&mut two, RAMWRT_OFF); // -> main despite PAGE2
    two.cpu.mem.write(0x2000, 0x5a);
    assert_eq!(two.ram()[0x2000], 0x5a, "HIRES off: $2000 follows RAMWRT");
    assert_eq!(two.aux_ram()[0x2000], 0x00);

    // HIRES on: $2000/$3FFF now follow PAGE2 under 80STORE.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    set(&mut two, STORE80_ON);
    set(&mut two, PAGE2);
    set(&mut two, HIRES);
    assert_routes_to(&mut two, 0x2000, true, "80STORE+HIRES+PAGE2 lo");
    assert_routes_to(&mut two, 0x3fff, true, "80STORE+HIRES+PAGE2 hi");
}

#[test]
fn store80_only_claims_page1() {
    // Page 2 text ($0800), hi-res page 2 ($4000) and ordinary RAM ($0300)
    // keep following RAMRD/RAMWRT even with 80STORE + HIRES + PAGE2 on.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    set(&mut two, STORE80_ON);
    set(&mut two, PAGE2);
    set(&mut two, HIRES);
    set(&mut two, RAMWRT_OFF); // main
    for &addr in &[0x0800u16, 0x4000, 0x0300] {
        two.cpu.mem.write(addr, 0x5a);
        assert_eq!(two.ram()[addr as usize], 0x5a, "${addr:04X} follows RAMWRT");
        assert_eq!(two.aux_ram()[addr as usize], 0x00, "${addr:04X} not aux");
    }
}

#[test]
fn page2_does_not_route_memory_when_80store_off() {
    // Regression guard: with 80STORE off, PAGE2 is just the display selector
    // and text page 1 follows RAMRD/RAMWRT.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    set(&mut two, STORE80_OFF);
    set(&mut two, PAGE2);
    set(&mut two, RAMWRT_OFF); // main
    two.cpu.mem.write(0x0400, 0x5a);
    assert_eq!(two.ram()[0x0400], 0x5a, "80STORE off: $0400 follows RAMWRT");
    assert_eq!(two.aux_ram()[0x0400], 0x00);
}

#[test]
fn rd80store_reports_state() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    assert_eq!(two.cpu.mem.read(0xc018) & 0x80, 0x00, "RD80STORE off");
    set(&mut two, STORE80_ON);
    assert_eq!(two.cpu.mem.read(0xc018) & 0x80, 0x80, "RD80STORE on");
    set(&mut two, STORE80_OFF);
    assert_eq!(two.cpu.mem.read(0xc018) & 0x80, 0x00, "RD80STORE off again");
}
