//! The bootloader menu, port of `boo.c`: a tty-rendered menu that picks
//! which machine to start.

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};

use crate::sdl;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, TTY_ROWS, Tty};

const BOO_FPS: u32 = 40;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BooChoice {
    Quit,
    BootApple1,
    BootReplica1,
    BootApple2Plus,
}

static MENU: [&str; TTY_ROWS] = [
    "****************************************",
    "*                                      *",
    "*       _______ ________ _______       *",
    "*      !    ___!  !  !  !   !   !      *",
    "*      !    ___!  !  !  !       !      *",
    "*      !_______!________!__!_!__!      *",
    "*                                      *",
    "*        GITHUB.COM/ST3FAN/EWM         *",
    "*                                      *",
    "* WHAT WOULD YOU LIKE TO EMULATE?      *",
    "*                                      *",
    "*   1) APPLE 1                         *",
    "*      6502 / 8KB / WOZ MONITOR        *",
    "*                                      *",
    "*   2) REPLICA 1                       *",
    "*      65C02 / 48KB / KRUSADER         *",
    "*                                      *",
    "*   3) APPLE ][+                       *",
    "*      6502 / 64KB (LANGUAGE CARD)     *",
    "*      DISK II / AUTOSTART ROM         *",
    "*                                      *",
    "* START WITH --HELP TO SEE ALL OPTIONS *",
    "*                                      *",
    "****************************************",
];

pub fn main(_args: &[String]) -> BooChoice {
    // Setup SDL

    let context = match sdl2::init() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("Failed to initialize SDL: {e}");
            return BooChoice::Quit;
        }
    };
    let video = context.video().expect("Failed to initialize SDL video");
    let timer = context.timer().expect("Failed to initialize SDL timer");

    let window = video
        .window("EWM v0.1 - Bootloader", 280 * 3, 192 * 3)
        .position_centered()
        .build()
        .expect("Failed create window");

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .expect("Failed to create renderer");

    if let Err(e) = sdl::check_renderer(&canvas) {
        eprintln!("{e}");
        return BooChoice::Quit;
    }

    canvas
        .set_logical_size(TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to set logical size");

    // We only need a tty to display the menu. (The C passes {255,255,0} —
    // yellow — in a variable called green.)
    let format = sdl::pixel_format(&canvas).unwrap_or(PixelFormatEnum::ARGB8888);
    let yellow = match format {
        PixelFormatEnum::RGBA8888 => 0xffff00ffu32,
        PixelFormatEnum::RGB888 => 0x00ffff00u32,
        _ => 0xffffff00u32, // ARGB8888
    };
    let mut tty = Tty::new(yellow);

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(format, TTY_PIXEL_WIDTH as u32, TTY_PIXEL_HEIGHT as u32)
        .expect("Failed to create texture");

    let mut event_pump = context.event_pump().expect("Failed to get event pump");
    let mut ticks = timer.ticks();
    let mut phase: u32 = 1;

    loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => return BooChoice::Quit,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Num1 => return BooChoice::BootApple1,
                    Keycode::Num2 => return BooChoice::BootReplica1,
                    Keycode::Num3 => return BooChoice::BootApple2Plus,
                    _ => {}
                },
                _ => {}
            }
        }

        if (timer.ticks() - ticks) >= (1000 / BOO_FPS) {
            if tty.screen_dirty || phase == 0 || phase.is_multiple_of(BOO_FPS / 4) {
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
                canvas.clear();

                tty.set_screen(&MENU);
                tty.cursor_column = 34;
                tty.cursor_row = 9;

                tty.refresh(phase, BOO_FPS);
                tty.screen_dirty = false;

                let mut bytes = Vec::with_capacity(tty.pixels.len() * 4);
                for p in &tty.pixels {
                    bytes.extend_from_slice(&p.to_ne_bytes());
                }
                texture
                    .update(None, &bytes, TTY_PIXEL_WIDTH * 4)
                    .expect("Failed to update texture");
                canvas
                    .copy(&texture, None, None)
                    .expect("Failed to copy texture");

                canvas.present();
            }

            ticks = timer.ticks();
            phase += 1;
            if phase == BOO_FPS {
                phase = 0;
            }
        }
    }
}
