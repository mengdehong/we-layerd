use anyhow::{anyhow, Context, Result};
use bytemuck::{Pod, Zeroable};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle};
use wayland_client::{protocol::wl_surface::WlSurface, Connection, Proxy};

const SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var tex: texture_2d<f32>;
@group(0) @binding(1)
var tex_sampler: sampler;
@group(0) @binding(2)
var<uniform> overlay: vec4<f32>; // x=width, y=height, z=fps, w=show(0/1)

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0)
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0)
    );

    var out: VsOut;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

@fragment
fn fs_main(inf: VsOut) -> @location(0) vec4<f32> {
    var color = textureSample(tex, tex_sampler, inf.uv);
    if (overlay.w < 0.5) {
        return color;
    }

    let p = inf.pos.xy;
    let digit_w = 16.0;
    let digit_h = 28.0;
    let thickness = 3.0;
    let gap = 4.0;
    let margin = 16.0;
    let total_w = digit_w * 3.0 + gap * 2.0;
    let origin = vec2<f32>(overlay.x - margin - total_w, overlay.y - margin - digit_h);

    let fps_i = i32(clamp(round(overlay.z), 0.0, 999.0));
    let d0 = fps_i / 100;
    let d1 = (fps_i / 10) % 10;
    let d2 = fps_i % 10;
    let show_hundreds = fps_i >= 100;

    let bg_min = origin - vec2<f32>(8.0, 8.0);
    let bg_max = vec2<f32>(overlay.x - margin + 8.0, overlay.y - margin + 8.0);
    let in_bg = p.x >= bg_min.x && p.x <= bg_max.x && p.y >= bg_min.y && p.y <= bg_max.y;
    if (in_bg) {
        color = color * vec4<f32>(0.6, 0.6, 0.6, 1.0);
    }

    var lit = false;
    if (show_hundreds) {
        lit = lit || draw_digit(p, origin, d0, digit_w, digit_h, thickness);
    }
    lit = lit || draw_digit(p, origin + vec2<f32>(digit_w + gap, 0.0), d1, digit_w, digit_h, thickness);
    lit = lit || draw_digit(p, origin + vec2<f32>((digit_w + gap) * 2.0, 0.0), d2, digit_w, digit_h, thickness);

    if (lit) {
        return vec4<f32>(0.98, 0.95, 0.20, 1.0);
    }

    return color;
}

fn in_rect(p: vec2<f32>, minp: vec2<f32>, maxp: vec2<f32>) -> bool {
    return p.x >= minp.x && p.x <= maxp.x && p.y >= minp.y && p.y <= maxp.y;
}

fn seg_on(d: i32, s: i32) -> bool {
    switch d {
        case 0: { return s != 6; }
        case 1: { return s == 1 || s == 2; }
        case 2: { return s == 0 || s == 1 || s == 6 || s == 4 || s == 3; }
        case 3: { return s == 0 || s == 1 || s == 6 || s == 2 || s == 3; }
        case 4: { return s == 5 || s == 6 || s == 1 || s == 2; }
        case 5: { return s == 0 || s == 5 || s == 6 || s == 2 || s == 3; }
        case 6: { return s == 0 || s == 5 || s == 6 || s == 2 || s == 3 || s == 4; }
        case 7: { return s == 0 || s == 1 || s == 2; }
        case 8: { return true; }
        case 9: { return s != 4; }
        default: { return false; }
    }
}

fn draw_digit(p: vec2<f32>, origin: vec2<f32>, w: i32, digit_w: f32, digit_h: f32, t: f32) -> bool {
    let x0 = origin.x;
    let y0 = origin.y;
    let x1 = x0 + digit_w;
    let y1 = y0 + digit_h;
    let ym = y0 + digit_h * 0.5;
    let inner_l = x0 + t;
    let inner_r = x1 - t;

    var lit = false;
    if (seg_on(w, 0) && in_rect(p, vec2<f32>(inner_l, y0), vec2<f32>(inner_r, y0 + t))) { lit = true; } // top
    if (seg_on(w, 1) && in_rect(p, vec2<f32>(x1 - t, y0 + t), vec2<f32>(x1, ym - t * 0.5))) { lit = true; } // upper right
    if (seg_on(w, 2) && in_rect(p, vec2<f32>(x1 - t, ym + t * 0.5), vec2<f32>(x1, y1 - t))) { lit = true; } // lower right
    if (seg_on(w, 3) && in_rect(p, vec2<f32>(inner_l, y1 - t), vec2<f32>(inner_r, y1))) { lit = true; } // bottom
    if (seg_on(w, 4) && in_rect(p, vec2<f32>(x0, ym + t * 0.5), vec2<f32>(x0 + t, y1 - t))) { lit = true; } // lower left
    if (seg_on(w, 5) && in_rect(p, vec2<f32>(x0, y0 + t), vec2<f32>(x0 + t, ym - t * 0.5))) { lit = true; } // upper left
    if (seg_on(w, 6) && in_rect(p, vec2<f32>(inner_l, ym - t * 0.5), vec2<f32>(inner_r, ym + t * 0.5))) { lit = true; } // middle
    return lit;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OverlayUniform {
    width: f32,
    height: f32,
    fps: f32,
    show: f32,
}

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    overlay_buffer: wgpu::Buffer,
    overlay: OverlayUniform,
    texture: wgpu::Texture,
    texture_size: (u32, u32),
    max_texture_dimension_2d: u32,
}

impl WgpuRenderer {
    pub fn new(conn: &Connection, wl_surface: &WlSurface, width: u32, height: u32) -> Result<Self> {
        let backend = conn.backend();
        let display_ptr = wayland_backend::sys::client::Backend::display_ptr(&backend).cast();
        let surface_ptr = wl_surface.id().as_ptr().cast();

        let display_nn =
            std::ptr::NonNull::new(display_ptr).context("failed to resolve wl_display pointer")?;
        let surface_nn =
            std::ptr::NonNull::new(surface_ptr).context("failed to resolve wl_surface pointer")?;

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_nn));
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_nn));

        let instance = wgpu::Instance::default();
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle,
                    raw_window_handle,
                })
                .context("failed to create wgpu surface for Wayland layer surface")?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow!("no suitable wgpu adapter found"))?;

        let adapter_limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("we-layerd-device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter_limits.clone(),
            },
            None,
        ))
        .context("failed to create wgpu device")?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| anyhow!("no surface formats reported by adapter"))?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque),
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("we-layerd-texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("we-layerd-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("we-layerd-texture-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("we-layerd-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let overlay = OverlayUniform {
            width: width.max(1) as f32,
            height: height.max(1) as f32,
            fps: 0.0,
            show: 0.0,
        };
        let overlay_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("we-layerd-overlay-uniform"),
            size: std::mem::size_of::<OverlayUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&overlay_buffer, 0, bytemuck::bytes_of(&overlay));

        let (texture, bind_group) = create_texture_resources(&device, &queue, &bind_group_layout, &overlay_buffer, 2, 2, 2 * 4, &[
            40, 40, 40, 255, 80, 80, 80, 255,
            80, 80, 80, 255, 40, 40, 40, 255,
        ]);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group_layout,
            bind_group,
            overlay_buffer,
            overlay,
            texture,
            texture_size: (2, 2),
            max_texture_dimension_2d: adapter_limits.max_texture_dimension_2d,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.overlay.width = self.config.width as f32;
        self.overlay.height = self.config.height as f32;
        self.queue
            .write_buffer(&self.overlay_buffer, 0, bytemuck::bytes_of(&self.overlay));
    }

    pub fn set_fps_overlay(&mut self, fps: f32, show: bool) {
        self.overlay.fps = fps.max(0.0);
        self.overlay.show = if show { 1.0 } else { 0.0 };
        self.queue
            .write_buffer(&self.overlay_buffer, 0, bytemuck::bytes_of(&self.overlay));
    }

    pub fn upload_bgra(&mut self, width: u32, height: u32, stride: u32, bgra: &[u8]) -> Result<()> {
        if width > self.max_texture_dimension_2d || height > self.max_texture_dimension_2d {
            return Err(anyhow!(
                "frame size {}x{} exceeds GPU texture limit {}x{}",
                width,
                height,
                self.max_texture_dimension_2d,
                self.max_texture_dimension_2d
            ));
        }

        let packed_row = width as usize * 4;
        let stride = stride as usize;
        if stride < packed_row {
            return Err(anyhow!(
                "invalid frame stride: got {}, expected >= {}",
                stride,
                packed_row
            ));
        }
        let expected = if height == 0 {
            0
        } else {
            stride * (height as usize - 1) + packed_row
        };
        if bgra.len() < expected {
            return Err(anyhow!(
                "invalid frame payload: got {}, expected at least {}",
                bgra.len(),
                expected
            ));
        }

        if self.texture_size != (width, height) {
            let (texture, bind_group) =
                create_texture_resources(
                    &self.device,
                    &self.queue,
                    &self.bind_group_layout,
                    &self.overlay_buffer,
                    width,
                    height,
                    stride as u32,
                    bgra,
                );
            self.texture = texture;
            self.bind_group = bind_group;
            self.texture_size = (width, height);
            return Ok(());
        }

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bgra[..expected],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        let frame = self
            .surface
            .get_current_texture()
            .context("failed to acquire frame from wgpu surface")?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("we-layerd-frame-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("we-layerd-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

fn create_texture_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bind_group_layout: &wgpu::BindGroupLayout,
    overlay_buffer: &wgpu::Buffer,
    width: u32,
    height: u32,
    stride: u32,
    bgra: &[u8],
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("we-layerd-frame-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bgra,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("we-layerd-frame-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("we-layerd-frame-bind-group"),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: overlay_buffer.as_entire_binding(),
            },
        ],
    });

    (texture, bind_group)
}
