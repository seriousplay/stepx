use anyhow::{Context, Result};
use winit::window::Icon;

/// X11 `_NET_WM_ICON` requests stay reliable at this edge. 512px RGBA is
/// over 1 MiB and can fail silently, leaving the dock on a generic glyph.
const MAX_ICON_EDGE: u32 = 128;

/// Decode a PNG into a winit window icon for `_NET_WM_ICON`.
///
/// KDE Icon Tasks uses this property for the *running* window. Without it the
/// pinned `.desktop` icon and the live task can disagree, so the dock shows a
/// generic/wrong glyph next to the real FaCloud mark.
pub(crate) fn icon_from_png(png: &[u8]) -> Result<Icon> {
    let mut image = image::load_from_memory(png)
        .context("decode window icon png")?
        .into_rgba8();
    if image.width() > MAX_ICON_EDGE || image.height() > MAX_ICON_EDGE {
        image = image::imageops::resize(
            &image,
            MAX_ICON_EDGE,
            MAX_ICON_EDGE,
            image::imageops::FilterType::Triangle,
        );
    }
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).context("create winit window icon")
}

#[cfg(test)]
#[path = "window_icon_tests.rs"]
mod tests;
