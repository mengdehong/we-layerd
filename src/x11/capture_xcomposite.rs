use std::{os::fd::AsRawFd, path::Path, ptr::null_mut};

use anyhow::{anyhow, Context, Result};
use image::{ImageBuffer, RgbaImage};
use x11rb::{
    connection::Connection,
    protocol::{
        composite::{self, ConnectionExt as _},
        shm::{self, ConnectionExt as _},
        xproto::{
            ChangeWindowAttributesAux, ConnectionExt as _, EventMask, ImageFormat, Pixmap, Window,
        },
        Event,
    },
    rust_connection::RustConnection,
};

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bgra: Vec<u8>,
}

pub struct XCompositeCapturer {
    conn: RustConnection,
    window: Window,
    shm: Option<ShmSegment>,
    cached_geometry: Option<CachedGeometry>,
    shm_enabled: bool,
    shm_warned: bool,
}

struct ShmSegment {
    seg: shm::Seg,
    addr: *mut u8,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct CachedGeometry {
    width: u16,
    height: u16,
    depth: u8,
}

impl XCompositeCapturer {
    pub fn new(window: Window) -> Result<Self> {
        let (conn, _) =
            RustConnection::connect(None).context("failed to connect to X11 display")?;
        let _ = conn
            .composite_query_version(0, 4)
            .context("failed to query XComposite extension")?
            .reply()
            .context("failed to receive XComposite version reply")?;

        let _ = conn
            .composite_redirect_window(window, composite::Redirect::AUTOMATIC)
            .context("failed to redirect window for XComposite")?;

        let _ = conn.change_window_attributes(
            window,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
        );

        conn.flush().context("failed to flush X11 requests")?;

        let shm_enabled = conn.shm_query_version().ok().and_then(|c| c.reply().ok()).is_some();

        Ok(Self { conn, window, shm: None, cached_geometry: None, shm_enabled, shm_warned: false })
    }

    pub fn capture_frame(&mut self) -> Result<CapturedFrame> {
        self.update_cached_geometry_from_events();

        let pixmap = self.conn.generate_id().context("failed to generate X11 pixmap id")?;
        self.conn
            .composite_name_window_pixmap(self.window, pixmap)
            .context("failed to name XComposite window pixmap")?;
        let geometry =
            self.query_pixmap_geometry(pixmap).context("failed to query pixmap geometry")?;

        let frame = if self.shm_enabled {
            match self.capture_pixmap_bgra_shm(pixmap, geometry) {
                Ok(frame) => Ok(frame),
                Err(err) => {
                    if !self.shm_warned {
                        self.shm_warned = true;
                        tracing::warn!(error = %err, "MIT-SHM capture failed, falling back to XGetImage");
                    }
                    self.shm_enabled = false;
                    self.release_shm();
                    capture_pixmap_bgra(&self.conn, pixmap, geometry)
                }
            }
        } else {
            capture_pixmap_bgra(&self.conn, pixmap, geometry)
        }
            .with_context(|| format!("failed to capture pixmap for window {}", self.window))
            ?;

        let _ = self.conn.free_pixmap(pixmap);
        let _ = self.conn.flush();
        Ok(frame)
    }

    fn capture_pixmap_bgra_shm(
        &mut self,
        pixmap: Pixmap,
        geometry: CachedGeometry,
    ) -> Result<CapturedFrame> {
        let width = u16::max(geometry.width, 1);
        let height = u16::max(geometry.height, 1);
        if geometry.depth < 24 {
            return Err(anyhow!(
                "unsupported X11 image depth {}, expected at least 24",
                geometry.depth
            ));
        }

        let packed_stride = width as usize * 4;
        let stride = x11_stride_for_depth(&self.conn, geometry.depth, width)?;
        if stride < packed_stride {
            return Err(anyhow!("invalid X11 row stride {} for width {}", stride, width));
        }
        let len = stride * height as usize;
        self.ensure_shm_capacity(len)?;

        let shm =
            self.shm.as_ref().ok_or_else(|| anyhow!("internal error: shm segment missing"))?;
        self.conn
            .shm_get_image(
                pixmap,
                0,
                0,
                width,
                height,
                u32::MAX,
                ImageFormat::Z_PIXMAP.into(),
                shm.seg,
                0,
            )
            .context("XShmGetImage request failed")?
            .reply()
            .context("XShmGetImage reply failed")?;

        // SAFETY: `addr` points to an mmap'd shared memory region of at least `len` bytes.
        let shm_data = unsafe { std::slice::from_raw_parts(shm.addr, len) };
        let bgra = if stride == packed_stride {
            shm_data.to_vec()
        } else {
            let mut packed = vec![0u8; packed_stride * height as usize];
            for row in 0..height as usize {
                let src_start = row * stride;
                let src_end = src_start + packed_stride;
                let dst_start = row * packed_stride;
                let dst_end = dst_start + packed_stride;
                packed[dst_start..dst_end].copy_from_slice(&shm_data[src_start..src_end]);
            }
            packed
        };
        Ok(CapturedFrame {
            width: width as u32,
            height: height as u32,
            stride: packed_stride as u32,
            bgra,
        })
    }

    fn query_pixmap_geometry(&mut self, pixmap: Pixmap) -> Result<CachedGeometry> {
        if let Some(geometry) = self.cached_geometry {
            return Ok(geometry);
        }

        let geometry = self
            .conn
            .get_geometry(pixmap)
            .context("get_geometry failed")?
            .reply()
            .context("get_geometry reply failed")?;

        let cached = CachedGeometry {
            width: u16::max(geometry.width, 1),
            height: u16::max(geometry.height, 1),
            depth: geometry.depth,
        };
        self.cached_geometry = Some(cached);
        Ok(cached)
    }

    fn update_cached_geometry_from_events(&mut self) {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(Event::ConfigureNotify(event))) if event.window == self.window => {
                    if let Some(geometry) = self.cached_geometry.as_mut() {
                        geometry.width = u16::max(event.width, 1);
                        geometry.height = u16::max(event.height, 1);
                    } else {
                        self.cached_geometry = None;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
    }

    fn ensure_shm_capacity(&mut self, required_len: usize) -> Result<()> {
        if self.shm.as_ref().map(|seg| seg.len >= required_len).unwrap_or(false) {
            return Ok(());
        }

        self.release_shm();

        let seg = self.conn.generate_id().context("failed to generate shm seg id")?;
        let reply = self
            .conn
            .shm_create_segment(seg, required_len as u32, false)
            .context("XShmCreateSegment request failed")?
            .reply()
            .context("XShmCreateSegment reply failed")?;

        // SAFETY: Mapping the server-provided FD for shared memory segment.
        let addr = unsafe {
            libc::mmap(
                null_mut(),
                required_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                reply.shm_fd.as_raw_fd(),
                0,
            )
        };
        if addr == libc::MAP_FAILED {
            let _ = self.conn.shm_detach(seg);
            return Err(anyhow!("mmap failed for XShm segment"));
        }

        self.shm = Some(ShmSegment { seg, addr: addr.cast::<u8>(), len: required_len });
        Ok(())
    }

    fn release_shm(&mut self) {
        if let Some(seg) = self.shm.take() {
            let _ = self.conn.shm_detach(seg.seg);
            // SAFETY: `addr`/`len` were returned by mmap in ensure_shm_capacity.
            let _ = unsafe { libc::munmap(seg.addr.cast(), seg.len) };
        }
    }
}

impl Drop for XCompositeCapturer {
    fn drop(&mut self) {
        self.release_shm();
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

fn capture_pixmap_bgra(
    conn: &RustConnection,
    pixmap: Pixmap,
    geometry: CachedGeometry,
) -> Result<CapturedFrame> {
    let width = u16::max(geometry.width, 1);
    let height = u16::max(geometry.height, 1);

    let image = conn
        .get_image(ImageFormat::Z_PIXMAP, pixmap, 0, 0, width, height, u32::MAX)
        .context("XGetImage request failed")?
        .reply()
        .context("XGetImage reply failed")?;

    if image.depth < 24 {
        return Err(anyhow!("unsupported X11 image depth {}, expected at least 24", image.depth));
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
        stride: (width as u32) * 4,
        bgra,
    })
}

fn x11_stride_for_depth(conn: &RustConnection, depth: u8, width: u16) -> Result<usize> {
    let format = conn
        .setup()
        .pixmap_formats
        .iter()
        .find(|fmt| fmt.depth == depth)
        .ok_or_else(|| anyhow!("missing pixmap format for depth {}", depth))?;

    let bits_per_pixel = usize::from(format.bits_per_pixel);
    if bits_per_pixel < 32 {
        return Err(anyhow!("unsupported bits_per_pixel {} for depth {}", bits_per_pixel, depth));
    }

    let scanline_pad = usize::from(format.scanline_pad.max(8));
    let row_bits = usize::from(width) * bits_per_pixel;
    let stride_bits = ((row_bits + scanline_pad - 1) / scanline_pad) * scanline_pad;
    Ok(stride_bits / 8)
}

pub fn save_frame_png(frame: &CapturedFrame, path: &Path) -> Result<()> {
    let mut rgba = frame.bgra.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let image: RgbaImage = ImageBuffer::from_vec(frame.width, frame.height, rgba)
        .ok_or_else(|| anyhow!("invalid frame dimensions for PNG output"))?;
    image.save(path).with_context(|| format!("failed to save capture PNG: {}", path.display()))
}
