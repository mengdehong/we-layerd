use std::path::Path;

use anyhow::{anyhow, Context, Result};
use image::{ImageBuffer, RgbaImage};
use x11rb::{
    connection::Connection,
    protocol::{
        composite::{self, ConnectionExt as _},
        xproto::{ConnectionExt as _, ImageFormat, Pixmap, Window},
    },
    rust_connection::RustConnection,
};

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn capture_single_frame(window: Window) -> Result<CapturedFrame> {
    let (conn, _) = RustConnection::connect(None).context("failed to connect to X11 display")?;

    let _ = conn
        .composite_query_version(0, 4)
        .context("failed to query XComposite extension")?
        .reply()
        .context("failed to receive XComposite version reply")?;

    let _ = conn
        .composite_redirect_window(window, composite::Redirect::AUTOMATIC)
        .context("failed to redirect window for XComposite")?;

    let pixmap = conn
        .generate_id()
        .context("failed to generate X11 pixmap id")?;
    conn.composite_name_window_pixmap(window, pixmap)
        .context("failed to name XComposite window pixmap")?;

    let frame = capture_pixmap_bgra(&conn, pixmap)
        .with_context(|| format!("failed to capture pixmap for window {}", window));

    let _ = conn.free_pixmap(pixmap);
    let _ = conn.flush();

    frame
}

pub fn probe_xcomposite_support() -> Result<()> {
    let (conn, _) = RustConnection::connect(None).context("failed to connect to X11 display")?;
    conn.composite_query_version(0, 4)
        .context("failed to query XComposite extension")?
        .reply()
        .context("failed to receive XComposite version reply")?;
    Ok(())
}

fn capture_pixmap_bgra(conn: &RustConnection, pixmap: Pixmap) -> Result<CapturedFrame> {
    let geometry = conn
        .get_geometry(pixmap)
        .context("get_geometry failed")?
        .reply()
        .context("get_geometry reply failed")?;

    let width = u16::max(geometry.width, 1);
    let height = u16::max(geometry.height, 1);

    let image = conn
        .get_image(ImageFormat::Z_PIXMAP, pixmap, 0, 0, width, height, u32::MAX)
        .context("XGetImage request failed")?
        .reply()
        .context("XGetImage reply failed")?;

    if image.depth < 24 {
        return Err(anyhow!(
            "unsupported X11 image depth {}, expected at least 24",
            image.depth
        ));
    }

    let expected = width as usize * height as usize * 4;
    if image.data.len() < expected {
        return Err(anyhow!(
            "unexpected image payload size: got {}, expected >= {}",
            image.data.len(),
            expected
        ));
    }

    let mut rgba = vec![0u8; expected];
    for (src, dst) in image
        .data
        .chunks_exact(4)
        .zip(rgba.chunks_exact_mut(4))
        .take(width as usize * height as usize)
    {
        // X11 on little-endian desktops is usually BGRA8888.
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 0xFF;
    }

    Ok(CapturedFrame {
        width: width as u32,
        height: height as u32,
        rgba,
    })
}

pub fn save_frame_png(frame: &CapturedFrame, path: &Path) -> Result<()> {
    let image: RgbaImage = ImageBuffer::from_vec(frame.width, frame.height, frame.rgba.clone())
        .ok_or_else(|| anyhow!("invalid frame dimensions for PNG output"))?;
    image
        .save(path)
        .with_context(|| format!("failed to save capture PNG: {}", path.display()))
}
