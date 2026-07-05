//! SDL helpers, port of `sdl.c`: pick a supported texture pixel format and
//! the green phosphor color packed for that format.

use sdl2::pixels::PixelFormatEnum;
use sdl2::render::WindowCanvas;

/// Port of `ewm_sdl_pixel_format`: the first of ARGB8888 / RGBA8888 /
/// RGB888 the renderer supports.
pub fn pixel_format(canvas: &WindowCanvas) -> Option<PixelFormatEnum> {
    canvas
        .info()
        .texture_formats
        .iter()
        .copied()
        .find(|format| {
            matches!(
                format,
                PixelFormatEnum::ARGB8888 | PixelFormatEnum::RGBA8888 | PixelFormatEnum::RGB888
            )
        })
}

/// Port of `ewm_sdl_check_renderer`.
pub fn check_renderer(canvas: &WindowCanvas) -> Result<(), String> {
    let info = canvas.info();
    if info.flags & sdl2::sys::SDL_RendererFlags::SDL_RENDERER_ACCELERATED as u32 == 0 {
        return Err("ewm: sdl: require accelerated renderer".into());
    }
    if pixel_format(canvas).is_none() {
        return Err(
            "ewm: sdl: cannot find supported pixel format (ARGB888, RGBA8888, RGB888)".into(),
        );
    }
    Ok(())
}

/// Port of `ewm_sdl_green`: full green packed for the renderer's format.
pub fn green(canvas: &WindowCanvas) -> u32 {
    match pixel_format(canvas) {
        Some(PixelFormatEnum::RGBA8888) => 0x00ff00ff,
        Some(PixelFormatEnum::ARGB8888) => 0xff00ff00,
        Some(PixelFormatEnum::RGB888) => 0x00ff0000,
        _ => 0xffffff,
    }
}
