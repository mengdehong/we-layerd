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
    pub bgra: Vec<u8>,
}

pub struct XCompositeCapturer {
    conn: RustConnection,
    window: Window,
}

impl XCompositeCapturer {
    pub fn new(window: Window) -> Result<Self> {
        let (conn, _) = RustConnection::connect(None).context("failed to connect to X11 display")?;
        let _ = conn
            .composite_query_version(0, 4)
            .context("failed to query XComposite extension")?
            .reply()
            .context("failed to receive XComposite version reply")?;

        let _ = conn
            .composite_redirect_window(window, composite::Redirect::AUTOMATIC)
            .context("failed to redirect window for XComposite")?;

        conn.flush().context("failed to flush X11 requests")?;

        Ok(Self { conn, window })
    }

    pub fn capture_frame(&mut self) -> Result<CapturedFrame> {
        let pixmap = self
            .conn
            .generate_id()
            .context("failed to generate X11 pixmap id")?;
        self.conn
            .composite_name_window_pixmap(self.window, pixmap)
            .context("failed to name XComposite window pixmap")?;

        let frame = capture_pixmap_bgra(&self.conn, pixmap)
            .with_context(|| format!("failed to capture pixmap for window {}", self.window))
            ?;

        let _ = self.conn.free_pixmap(pixmap);
        let _ = self.conn.flush();
        Ok(frame)
    }
}

impl Drop for XCompositeCapturer {
    fn drop(&mut self) {
        let _ = self.conn.flush();
    }
}

pub fn capture_single_frame(window: Window) -> Result<CapturedFrame> {
    XCompositeCapturer::new(window)?.capture_frame()
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

    let bgra = if image.data.len() == expected {
        image.data
    } else {
        // Some X11 servers pad each row; repack to tightly packed BGRA.
        let row_stride = image.data.len() / height as usize;
        let mut packed = vec![0u8; expected];
        for row in 0..height as usize {
            let src_start = row * row_stride;
            let dst_start = row * width as usize * 4;
            let src_end = src_start + width as usize * 4;
            let dst_end = dst_start + width as usize * 4;
            packed[dst_start..dst_end].copy_from_slice(&image.data[src_start..src_end]);
        }
        packed
    };

    Ok(CapturedFrame {
        width: width as u32,
        height: height as u32,
        bgra,
    })
}

pub fn save_frame_png(frame: &CapturedFrame, path: &Path) -> Result<()> {
    let mut rgba = frame.bgra.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let image: RgbaImage = ImageBuffer::from_vec(frame.width, frame.height, rgba)
        .ok_or_else(|| anyhow!("invalid frame dimensions for PNG output"))?;
    image
        .save(path)
        .with_context(|| format!("failed to save capture PNG: {}", path.display()))
}
