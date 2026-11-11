use std::cell::RefCell;
use std::f32::consts::TAU;
use std::rc::Rc;

use glam::Vec3;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wgpu::util::DeviceExt;
use web_sys::HtmlCanvasElement;

const CARDS_PER_TREE: u32 = 22;
const WRAP_LEN: f32 = 900.0;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLE_COUNT: u32 = 4;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            f(app);
        }
    });
}

#[wasm_bindgen]
pub async fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let app = App::new().await?;
    APP.with(|cell| *cell.borrow_mut() = Some(app));
    start_loop();
    Ok(())
}

#[wasm_bindgen]
pub fn set_tree_count(count: f32) {
    with_app(|app| app.rebuild_trees(count as u32));
}

#[wasm_bindgen]
pub fn set_speed(kmh: f32) {
    with_app(|app| app.speed = (kmh / 3.6).clamp(0.0, 140.0));
}

#[wasm_bindgen]
pub fn set_wind(strength: f32) {
    with_app(|app| app.wind = strength.clamp(0.0, 2.0));
}

#[wasm_bindgen]
pub fn set_style(toon: f32) {
    with_app(|app| app.style = toon.clamp(0.0, 1.0));
}

#[wasm_bindgen]
pub fn set_billboard(amount: f32) {
    with_app(|app| app.billboard = amount.clamp(0.0, 1.0));
}

#[wasm_bindgen]
pub fn toggle_pause() -> bool {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.paused = !app.paused;
            app.paused
        } else {
            false
        }
    })
}

#[wasm_bindgen]
pub fn look(dx: f32, dy: f32) {
    with_app(|app| {
        app.yaw += dx * 0.004;
        app.pitch = (app.pitch - dy * 0.004).clamp(-1.15, 1.15);
    });
}

#[wasm_bindgen]
pub fn reset_view() {
    with_app(|app| {
        app.yaw = 0.0;
        app.pitch = -0.04;
        app.cam_z = 0.0;
    });
}

fn start_loop() {
    let holder: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let holder_store = holder.clone();
    let holder_loop = holder.clone();
    *holder_store.borrow_mut() = Some(Closure::wrap(Box::new(move |_now: f64| {
        APP.with(|cell| {
            if let Some(app) = cell.borrow_mut().as_mut() {
                app.frame();
            }
        });
        let cb = holder_loop.borrow();
        request_animation_frame(cb.as_ref().unwrap());
    }) as Box<dyn FnMut(f64)>));
    let cb = holder.borrow();
    request_animation_frame(cb.as_ref().unwrap());
}

fn request_animation_frame(cb: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .expect("window")
        .request_animation_frame(cb.as_ref().unchecked_ref())
        .expect("requestAnimationFrame");
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    sun_dir: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    cam_fwd: [f32; 4],
    time: [f32; 4],
    fog_color: [f32; 4],
    misc: [f32; 4],
    style: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CardInstance {
    tree_pos: [f32; 3],
    phase: f32,
    offset: [f32; 3],
    size_x: f32,
    size_y: f32,
    rot: f32,
    tint: f32,
    kind: f32,
    normal: [f32; 3],
    shade: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TrunkInstance {
    pos_scale: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TrunkVertex {
    pos: [f32; 3],
    nrm: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GroundVertex {
    pos: [f32; 2],
}

struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.0 >> 8) & 0x00FF_FFFF) as f32 / 16_777_216.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

fn generate_trees(count: u32) -> (Vec<CardInstance>, Vec<TrunkInstance>) {
    let mut rng = Rng::new(0x9E37_79B9 ^ count.wrapping_mul(2_654_435_761));
    let mut cards = Vec::with_capacity(count as usize * CARDS_PER_TREE as usize);
    let mut trunks = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
        let x = side * rng.range(12.5, 70.0);
        let z = rng.range(0.0, WRAP_LEN);
        let scale = rng.range(0.8, 1.35);
        let yaw = rng.range(0.0, TAU);
        let tint = rng.next();
        let phase = rng.range(0.0, TAU);

        trunks.push(TrunkInstance {
            pos_scale: [x, 0.0, z, scale],
            params: [yaw, tint, phase, 0.0],
        });

        let ca = yaw.cos();
        let sa = yaw.sin();
        let canopy_center = Vec3::new(0.0, 4.9 * scale, 0.0);

        let lobes: [(Vec3, Vec3, u32); 4] = [
            (Vec3::new(0.0, 4.35, 0.0), Vec3::new(2.6, 1.15, 2.4), 7),
            (Vec3::new(1.05, 5.05, 0.35), Vec3::new(1.9, 0.95, 1.7), 6),
            (Vec3::new(-0.95, 5.55, -0.35), Vec3::new(1.55, 0.85, 1.45), 5),
            (Vec3::new(0.15, 6.0, 0.25), Vec3::new(1.05, 0.7, 0.95), 4),
        ];

        for (li, (lobe_center, lobe_radii, count)) in lobes.iter().enumerate() {
            let cx = lobe_center.x + rng.range(-0.3, 0.3);
            let cy = lobe_center.y + rng.range(-0.15, 0.15);
            let cz = lobe_center.z + rng.range(-0.3, 0.3);
            let rx = lobe_radii.x * rng.range(0.86, 1.16) * scale;
            let ry = lobe_radii.y * rng.range(0.86, 1.16) * scale;
            let rz = lobe_radii.z * rng.range(0.86, 1.16) * scale;

            for k in 0..*count {
                let u = ((k as f32 + 0.5) / *count as f32) * 2.0 - 1.0;
                let phi = k as f32 * 2.399_963 + li as f32 * 1.7 + rng.range(-0.35, 0.35);
                let sr = (1.0 - u * u).max(0.0).sqrt();
                let dir = Vec3::new(sr * phi.cos(), u, sr * phi.sin());
                let shell = rng.range(0.72, 1.02);

                let local = Vec3::new(
                    cx + dir.x * rx * shell,
                    cy + dir.y * ry * shell,
                    cz + dir.z * rz * shell,
                );
                let off = Vec3::new(ca * local.x - sa * local.z, local.y, sa * local.x + ca * local.z);
                let nrm = (local - canopy_center).normalize_or_zero();
                let nrm = Vec3::new(ca * nrm.x - sa * nrm.z, nrm.y, sa * nrm.x + ca * nrm.z);
                let size = (1.15 + rng.range(0.0, 1.05) - li as f32 * 0.07) * scale;

                cards.push(CardInstance {
                    tree_pos: [x, 0.0, z],
                    phase,
                    offset: [off.x, off.y, off.z],
                    size_x: size,
                    size_y: size * rng.range(1.0, 1.2),
                    rot: rng.range(0.0, TAU),
                    tint: rng.range(0.0, 1.0),
                    kind: rng.range(0.0, 4.0),
                    normal: [nrm.x, nrm.y, nrm.z],
                    shade: rng.range(0.9, 1.06),
                });
            }
        }
    }

    (cards, trunks)
}

fn add_cylinder(
    verts: &mut Vec<TrunkVertex>,
    indices: &mut Vec<u32>,
    base: Vec3,
    dir: Vec3,
    len: f32,
    r0: f32,
    r1: f32,
    segs: u32,
) {
    let n = dir.normalize();
    let a = if n.y.abs() < 0.99 {
        Vec3::Y.cross(n).normalize()
    } else {
        Vec3::X.cross(n).normalize()
    };
    let b = n.cross(a).normalize();
    let start = verts.len() as u32;

    for ring in 0..2u32 {
        let (c, r) = if ring == 0 {
            (base, r0)
        } else {
            (base + n * len, r1)
        };
        for s in 0..segs {
            let ang = s as f32 / segs as f32 * TAU;
            let radial = a * ang.cos() + b * ang.sin();
            verts.push(TrunkVertex {
                pos: (c + radial * r).into(),
                nrm: radial.into(),
            });
        }
    }

    for s in 0..segs {
        let a0 = start + s;
        let b0 = start + (s + 1) % segs;
        let c0 = a0 + segs;
        let d0 = b0 + segs;
        indices.extend_from_slice(&[a0, b0, c0, b0, d0, c0]);
    }
}

fn build_trunk_mesh() -> (Vec<TrunkVertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    add_cylinder(&mut verts, &mut indices, Vec3::ZERO, Vec3::new(0.03, 1.0, 0.0), 4.6, 0.21, 0.075, 7);
    add_cylinder(&mut verts, &mut indices, Vec3::new(0.0, 2.35, 0.0), Vec3::new(0.5, 1.0, 0.15), 2.15, 0.085, 0.02, 5);
    add_cylinder(&mut verts, &mut indices, Vec3::new(0.0, 2.55, 0.0), Vec3::new(-0.45, 1.0, 0.3), 2.05, 0.08, 0.02, 5);
    add_cylinder(&mut verts, &mut indices, Vec3::new(0.0, 3.05, 0.0), Vec3::new(0.15, 1.0, -0.55), 1.7, 0.07, 0.02, 5);
    add_cylinder(&mut verts, &mut indices, Vec3::new(0.0, 3.5, 0.0), Vec3::new(0.2, 1.0, 0.6), 1.45, 0.06, 0.018, 5);
    add_cylinder(&mut verts, &mut indices, Vec3::new(0.0, 3.9, 0.0), Vec3::new(-0.25, 1.0, -0.2), 1.1, 0.05, 0.015, 5);
    (verts, indices)
}

struct App {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    canvas: HtmlCanvasElement,

    width: u32,
    height: u32,

    sky_pipeline: wgpu::RenderPipeline,
    ground_pipeline: wgpu::RenderPipeline,
    trunk_pipeline: wgpu::RenderPipeline,
    card_pipeline: wgpu::RenderPipeline,

    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,

    ground_buf: wgpu::Buffer,
    trunk_mesh_buf: wgpu::Buffer,
    trunk_index_buf: wgpu::Buffer,
    trunk_index_count: u32,
    trunk_inst_buf: wgpu::Buffer,
    tree_count: u32,
    card_buf: wgpu::Buffer,
    card_count: u32,

    depth_view: wgpu::TextureView,
    msaa_view: wgpu::TextureView,

    clock: f32,
    last_ms: f64,
    cam_z: f32,
    yaw: f32,
    pitch: f32,
    speed: f32,
    wind: f32,
    style: f32,
    billboard: f32,
    paused: bool,
    fps: f32,
    stat_timer: f64,
}

impl App {
    async fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas = document
            .get_element_by_id("gfx")
            .ok_or_else(|| JsValue::from_str("missing #gfx canvas"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("#gfx is not a canvas"))?;

        let dpr = window.device_pixel_ratio().clamp(1.0, 2.0);
        let css_w = canvas.client_width().max(1);
        let css_h = canvas.client_height().max(1);
        let width = (css_w as f64 * dpr).round().max(1.0) as u32;
        let height = (css_h as f64 * dpr).round().max(1.0) as u32;
        canvas.set_width(width);
        canvas.set_height(height);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(js_err)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(js_err)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("fluffy-trees"),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(js_err)?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let msaa_state = wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            mask: !0,
            alpha_to_coverage_enabled: true,
        };

        let depth_state = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        let color_target = [Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                alpha_to_coverage_enabled: false,
                ..msaa_state
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            multiview: None,
            cache: None,
        });

        const GROUND_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
        let ground_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GroundVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &GROUND_ATTRS,
        };
        let ground_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ground"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ground"),
                compilation_options: Default::default(),
                buffers: &[ground_layout],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state.clone()),
            multisample: wgpu::MultisampleState {
                alpha_to_coverage_enabled: false,
                ..msaa_state
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ground"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            multiview: None,
            cache: None,
        });

        const TRUNK_ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        const TRUNK_INST_ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4];
        let trunk_mesh_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TrunkVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &TRUNK_ATTRS,
        };
        let trunk_inst_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TrunkInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &TRUNK_INST_ATTRS,
        };
        let trunk_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("trunk"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_trunk"),
                compilation_options: Default::default(),
                buffers: &[trunk_mesh_layout, trunk_inst_layout],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state.clone()),
            multisample: wgpu::MultisampleState {
                alpha_to_coverage_enabled: false,
                ..msaa_state
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_trunk"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            multiview: None,
            cache: None,
        });

        const CARD_ATTRS: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let card_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CardInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &CARD_ATTRS,
        };
        let card_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cards"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_card"),
                compilation_options: Default::default(),
                buffers: &[card_layout],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state),
            multisample: msaa_state,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_card"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            multiview: None,
            cache: None,
        });

        let (trunk_verts, trunk_indices) = build_trunk_mesh();
        let trunk_mesh_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trunk-mesh"),
            contents: bytemuck::cast_slice(&trunk_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let trunk_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trunk-indices"),
            contents: bytemuck::cast_slice(&trunk_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let ground_verts = [
            GroundVertex { pos: [-1.0, -1.0] },
            GroundVertex { pos: [1.0, -1.0] },
            GroundVertex { pos: [-1.0, 1.0] },
            GroundVertex { pos: [1.0, -1.0] },
            GroundVertex { pos: [1.0, 1.0] },
            GroundVertex { pos: [-1.0, 1.0] },
        ];
        let ground_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ground"),
            contents: bytemuck::cast_slice(&ground_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let (cards, trunks) = generate_trees(1800);
        let card_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cards"),
            contents: bytemuck::cast_slice(&cards),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let trunk_inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trunk-instances"),
            contents: bytemuck::cast_slice(&trunks),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let (depth_view, msaa_view) = create_targets(&device, &config);

        Ok(App {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            canvas,
            width,
            height,
            sky_pipeline,
            ground_pipeline,
            trunk_pipeline,
            card_pipeline,
            bind_group,
            uniform,
            ground_buf,
            trunk_mesh_buf,
            trunk_index_buf,
            trunk_index_count: trunk_indices.len() as u32,
            trunk_inst_buf,
            tree_count: trunks.len() as u32,
            card_buf,
            card_count: cards.len() as u32,
            depth_view,
            msaa_view,
            clock: 0.0,
            last_ms: 0.0,
            cam_z: 0.0,
            yaw: 0.0,
            pitch: -0.04,
            speed: 25.0,
            wind: 0.6,
            style: 0.35,
            billboard: 1.0,
            paused: false,
            fps: 60.0,
            stat_timer: 0.0,
        })
    }

    fn rebuild_trees(&mut self, count: u32) {
        let count = count.clamp(100, 15_000);
        let (cards, trunks) = generate_trees(count);
        self.card_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cards"),
            contents: bytemuck::cast_slice(&cards),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.trunk_inst_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("trunk-instances"),
            contents: bytemuck::cast_slice(&trunks),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.tree_count = trunks.len() as u32;
        self.card_count = cards.len() as u32;
    }

    fn resize_if_needed(&mut self) -> bool {
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0)
            .clamp(1.0, 2.0);
        let css_w = self.canvas.client_width().max(1);
        let css_h = self.canvas.client_height().max(1);
        let width = (css_w as f64 * dpr).round().max(1.0) as u32;
        let height = (css_h as f64 * dpr).round().max(1.0) as u32;
        if width == self.width && height == self.height {
            return false;
        }
        self.width = width;
        self.height = height;
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (depth, msaa) = create_targets(&self.device, &self.config);
        self.depth_view = depth;
        self.msaa_view = msaa;
        true
    }

    fn frame(&mut self) {
        self.resize_if_needed();

        let now = js_sys::Date::now();
        let dt = if self.last_ms == 0.0 {
            0.016
        } else {
            ((now - self.last_ms) / 1000.0).clamp(0.0, 0.1) as f32
        };
        self.last_ms = now;
        self.clock += dt;
        if !self.paused {
            self.cam_z += self.speed * dt;
        }
        if dt > 0.0 {
            self.fps = self.fps * 0.92 + (1.0 / dt) * 0.08;
        }

        let aspect = self.width as f32 / self.height.max(1) as f32;
        let fov_y: f32 = 62.0_f32.to_radians();
        let eye = Vec3::new(0.0, 2.35, self.cam_z);
        let forward = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
        .normalize();
        let right = Vec3::Y.cross(forward).normalize();
        let up = forward.cross(right).normalize();

        let view = glam::camera::rh::view::look_to_mat4(eye, forward, Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective(fov_y, aspect, 0.1, 1200.0);
        let view_proj = proj * view;
        let sun_dir = Vec3::new(0.58, 0.48, 0.66).normalize();

        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            camera_pos: [eye.x, eye.y, eye.z, 1.0],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, 1.0],
            cam_right: [right.x, right.y, right.z, 0.0],
            cam_up: [up.x, up.y, up.z, 0.0],
            cam_fwd: [forward.x, forward.y, forward.z, 0.0],
            time: [self.clock, self.wind, self.cam_z, dt],
            fog_color: [0.71, 0.80, 0.91, 0.0038],
            misc: [(fov_y * 0.5).tan(), aspect, WRAP_LEN, 1.0],
            style: [self.style, self.billboard, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&globals));

        self.stat_timer += dt as f64;
        if self.stat_timer > 0.35 {
            self.stat_timer = 0.0;
            self.update_stats();
        }

        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view_tex = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.msaa_view,
                depth_slice: None,
                resolve_target: Some(&view_tex),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.70,
                        g: 0.79,
                        b: 0.90,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.bind_group, &[]);

            pass.set_pipeline(&self.sky_pipeline);
            pass.draw(0..3, 0..1);

            pass.set_pipeline(&self.ground_pipeline);
            pass.set_vertex_buffer(0, self.ground_buf.slice(..));
            pass.draw(0..6, 0..1);

            pass.set_pipeline(&self.trunk_pipeline);
            pass.set_vertex_buffer(0, self.trunk_mesh_buf.slice(..));
            pass.set_vertex_buffer(1, self.trunk_inst_buf.slice(..));
            pass.set_index_buffer(self.trunk_index_buf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.trunk_index_count, 0, 0..self.tree_count);

            pass.set_pipeline(&self.card_pipeline);
            pass.set_vertex_buffer(0, self.card_buf.slice(..));
            pass.draw(0..6, 0..self.card_count);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }

    fn update_stats(&self) {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(el) = document.get_element_by_id("stats") else {
            return;
        };
        let speed_kmh = self.speed * 3.6;
        let text = format!(
            "{:.0} FPS\n{} trees  ·  {} billboard quads\n{:.3}M tris  ·  4 draw calls\n{:.0} km/h  ·  {:.1} ms",
            self.fps,
            thousands(self.tree_count),
            thousands(self.card_count),
            (self.card_count * 2 + self.tree_count * 64) as f32 / 1_000_000.0,
            speed_kmh,
            1000.0 / self.fps.max(1.0),
        );
        el.set_text_content(Some(&text));
    }
}

fn create_targets(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> (wgpu::TextureView, wgpu::TextureView) {
    let size = wgpu::Extent3d {
        width: config.width.max(1),
        height: config.height.max(1),
        depth_or_array_layers: 1,
    };
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size,
        mip_level_count: 1,
        sample_count: SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let msaa = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("msaa"),
        size,
        mip_level_count: 1,
        sample_count: SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    (
        depth.create_view(&wgpu::TextureViewDescriptor::default()),
        msaa.create_view(&wgpu::TextureViewDescriptor::default()),
    )
}

fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
