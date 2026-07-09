//! Throwaway WOZ compatibility sweep (run manually):
//!   cargo test --test zz_woz_sweep -- --ignored --nocapture
//! Boots every reference image and reports what it reaches.

use ewm::two::{ScreenMode, Two, TwoType};

fn step(two: &mut Two, cycles: u64) {
    let mut done = 0u64;
    while done < cycles {
        done += two.cpu.step() as u64;
    }
}

#[test]
#[ignore]
fn sweep() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../disks/woz/WOZ 1.0");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension().and_then(|x| x.to_str()) == Some("woz")).then_some(p)
        })
        .collect();
    paths.sort();

    for path in paths {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let mut two = Two::new(TwoType::Apple2Plus).unwrap();
        two.load_disk(0, path.to_str().unwrap()).unwrap();
        two.cpu.reset();

        let mut gfx_at = None;
        let mut spent = 0u64;
        while spent < 120_000_000 {
            step(&mut two, 1_000_000);
            spent += 1_000_000;
            if gfx_at.is_none() && two.screen_mode() == ScreenMode::Graphics {
                gfx_at = Some(spent);
                // Keep running a little to see if it stays in graphics.
                step(&mut two, 5_000_000);
                break;
            }
        }

        let text = two.text_screen();
        let nonblank = text.chars().filter(|c| !c.is_whitespace()).count();
        let first_line = text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();
        match gfx_at {
            Some(at) => println!(
                "{name:55} GRAPHICS at {:>4}M (text {nonblank})",
                at / 1_000_000
            ),
            None => println!("{name:55} text-only, {nonblank:4} chars: {first_line:.40}"),
        }
    }
}
