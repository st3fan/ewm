//! SDL helpers, port of `sdl.c`: pick a supported texture pixel format and
//! the green phosphor color packed for that format.

use sdl3::pixels::PixelFormat;
use sdl3::render::WindowCanvas;
use sdl3::sys::pixels::SDL_PixelFormat;
use sdl3::sys::properties::SDL_GetPointerProperty;
use sdl3::sys::render::{SDL_GetRendererProperties, SDL_PROP_RENDERER_TEXTURE_FORMATS_POINTER};

/// The texture formats the renderer supports. SDL3 removed
/// `SDL_RendererInfo`, so this walks the renderer's properties: an
/// `SDL_PixelFormat` array terminated by `UNKNOWN`.
fn texture_formats(canvas: &WindowCanvas) -> Vec<PixelFormat> {
    let mut formats = Vec::new();
    unsafe {
        let props = SDL_GetRendererProperties(canvas.raw());
        let mut p = SDL_GetPointerProperty(
            props,
            SDL_PROP_RENDERER_TEXTURE_FORMATS_POINTER,
            std::ptr::null_mut(),
        ) as *const SDL_PixelFormat;
        if !p.is_null() {
            while *p != SDL_PixelFormat::UNKNOWN {
                formats.push(PixelFormat::from_ll(*p));
                p = p.add(1);
            }
        }
    }
    formats
}

/// Port of `ewm_sdl_pixel_format`: the first of ARGB8888 / RGBA8888 /
/// XRGB8888 (SDL2's RGB888) the renderer supports.
pub fn pixel_format(canvas: &WindowCanvas) -> Option<PixelFormat> {
    texture_formats(canvas).into_iter().find(|&format| {
        format == PixelFormat::ARGB8888
            || format == PixelFormat::RGBA8888
            || format == PixelFormat::XRGB8888
    })
}

/// Port of `ewm_sdl_check_renderer`. SDL3 removed the ACCELERATED flag;
/// the software renderer is the only non-accelerated one, so check by name.
pub fn check_renderer(canvas: &WindowCanvas) -> Result<(), String> {
    if canvas.renderer_name == "software" {
        return Err("ewm: sdl: require accelerated renderer".into());
    }
    if pixel_format(canvas).is_none() {
        return Err(
            "ewm: sdl: cannot find supported pixel format (ARGB888, RGBA8888, RGB888)".into(),
        );
    }
    Ok(())
}

/// Window pixels between the screen contents and the window border,
/// from `EWM_WINDOW_PADDING` (default 4).
pub fn window_padding() -> u32 {
    std::env::var("EWM_WINDOW_PADDING")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4)
}

/// Port of `ewm_sdl_green`: full green packed for the renderer's format.
pub fn green(canvas: &WindowCanvas) -> u32 {
    match pixel_format(canvas) {
        Some(format) if format == PixelFormat::RGBA8888 => 0x00ff00ff,
        Some(format) if format == PixelFormat::ARGB8888 => 0xff00ff00,
        Some(format) if format == PixelFormat::XRGB8888 => 0x00ff0000,
        _ => 0xffffff,
    }
}
