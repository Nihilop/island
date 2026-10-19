// Moteur des fonds animés : UN thread qui possède les fenêtres (une par écran, enfants du
// calque `WorkerW`, cf. `services::desktop`), pompe leurs messages et les dessine avec
// wgpu (DX12 seul). Piloté par messages (`Cmd`) ; sans scène, le thread dort.
//
// Économie : pause d'un écran couvert par une fenêtre maximisée / plein écran, pause de
// tous pendant une app plein écran (détection de l'hôte) ou un jeu (service Activité) ;
// aucune image n'est alors calculée. La souris est LUE à chaque image (pas de hook) et
// seulement prise en compte quand elle est sur le bureau visible.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClassNameW, GetForegroundWindow, GetWindowRect, IsWindow,
    IsZoomed, PeekMessageW, RegisterClassExW, TranslateMessage, MSG, PM_REMOVE, WM_ERASEBKGND, WNDCLASSEXW, WS_CHILD,
    WS_EX_NOACTIVATE, WS_VISIBLE,
};

use super::scene::{self, Frame, Scene};
use crate::services::desktop::windows as desktop;
use crate::services::hardware::{displays, DisplayInfo};

/// Une scène chargée, prête pour le moteur (image de fond résolue en chemin absolu).
#[derive(Clone)]
pub struct Loaded {
    pub id: String,
    pub scene: Box<Scene>,
    pub image: Option<PathBuf>,
}

pub enum Cmd {
    /// Affectation : `all` pour tous les écrans, `screens` pour ceux réglés à part (None =
    /// fond Windows sur cet écran). Résolue à chaque changement d'écrans.
    Show { all: Option<Loaded>, screens: BTreeMap<String, Option<Loaded>> },
    Stop,
    Options { fps: u32, pause_in_game: bool },
}

#[derive(serde::Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub running: bool,
    /// Tous les écrans en pause (plein écran, jeu, fenêtres maximisées).
    pub paused: bool,
    pub error: Option<String>,
}

static TX: Mutex<Option<Sender<Cmd>>> = Mutex::new(None);
static STATUS: Mutex<Status> = Mutex::new(Status { running: false, paused: false, error: None });

pub fn status() -> Status {
    STATUS.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Envoie une commande ; démarre le thread au premier appel.
pub fn send(app: &AppHandle, cmd: Cmd) {
    let mut tx = TX.lock().unwrap_or_else(|p| p.into_inner());
    if tx.is_none() {
        let (t, rx) = mpsc::channel();
        let app = app.clone();
        let _ = std::thread::Builder::new().name("island-wallpaper".into()).spawn(move || run(app, rx));
        *tx = Some(t);
    }
    if let Some(t) = tx.as_ref() {
        let _ = t.send(cmd);
    }
}

fn set_status(app: &AppHandle, s: Status) {
    let mut cur = STATUS.lock().unwrap_or_else(|p| p.into_inner());
    if *cur != s {
        *cur = s.clone();
        let _ = app.emit("wallpaper://status", s);
    }
}

// --- GPU ---------------------------------------------------------------------------------

struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// Format sans sRGB : mêmes calculs de couleur que l'aperçu WebGPU de l'éditeur.
const TEX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Gpu {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            // Le GPU intégré s'il existe : un fond d'écran n'a pas besoin du gros GPU.
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("island-wallpaper"),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Bgra8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Ok(Self { instance, adapter, device, queue, pipeline, layout, sampler })
    }

    /// Image de fond (1×1 noir pour un dégradé). Bornée à 8192 px (limite des textures).
    fn texture(&self, path: Option<&PathBuf>) -> (wgpu::TextureView, (f32, f32)) {
        let img = path.and_then(|p| image::open(p).ok()).map(|i| {
            let max = 8192;
            if i.width() > max || i.height() > max { i.resize(max, max, image::imageops::FilterType::Triangle) } else { i }
        });
        let rgba = img.map(|i| i.to_rgba8()).unwrap_or_else(|| image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])));
        let (w, h) = rgba.dimensions();
        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("background"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEX_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            size,
        );
        (tex.create_view(&Default::default()), (w as f32, h as f32))
    }
}

// --- Fenêtres ----------------------------------------------------------------------------

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_ERASEBKGND {
        return LRESULT(1); // jamais d'effacement : pas de flash entre deux images
    }
    DefWindowProcW(h, msg, wp, lp)
}

fn register_class() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: GetModuleHandleW(None).map(|h| h.into()).unwrap_or_default(),
            lpszClassName: w!("IslandWallpaper"),
            ..Default::default()
        };
        RegisterClassExW(&wc);
    });
}

/// Fenêtre détruite à la libération.
struct Win(HWND);

impl Drop for Win {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// Un écran : sa scène, sa fenêtre enfant du calque, sa surface et ses uniformes. Les
/// champs sont libérés dans l'ordre : la surface AVANT sa fenêtre (`_win` en dernier).
struct Output {
    display: DisplayInfo,
    scene: Box<Scene>,
    image: (f32, f32),
    hwnd: HWND,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    paused: bool,
    _win: Win,
}

fn create_output(gpu: &Gpu, layer: HWND, d: &DisplayInfo, scene: &Loaded, tex: &(wgpu::TextureView, (f32, f32))) -> Result<Output, String> {
    let view = &tex.0;
    register_class();
    // Coordonnées du calque (il couvre tout le bureau virtuel, origine variable).
    let mut p = POINT { x: d.x, y: d.y };
    unsafe {
        let _ = ScreenToClient(layer, &mut p);
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE,
            w!("IslandWallpaper"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            p.x,
            p.y,
            d.width as i32,
            d.height as i32,
            Some(layer),
            None,
            GetModuleHandleW(None).ok().map(|h| h.into()),
            None,
        )
    }
    .map_err(|e| e.to_string())?;
    let handle = raw_window_handle::Win32WindowHandle::new(std::num::NonZeroIsize::new(hwnd.0 as isize).ok_or("hwnd")?);
    let surface = unsafe {
        gpu.instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::new())),
            raw_window_handle: raw_window_handle::RawWindowHandle::Win32(handle),
        })
    }
    .map_err(|e| {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        e.to_string()
    })?;
    let mut config = surface.get_default_config(&gpu.adapter, d.width.max(1), d.height.max(1)).ok_or("surfaceConfig")?;
    config.format = wgpu::TextureFormat::Bgra8Unorm;
    config.present_mode = wgpu::PresentMode::Fifo;
    config.desired_maximum_frame_latency = 1;
    surface.configure(&gpu.device, &config);
    let uniforms = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uniforms"),
        size: (scene::VEC4S * 16) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind = bind_group(gpu, &uniforms, view);
    Ok(Output { display: d.clone(), scene: scene.scene.clone(), image: tex.1, hwnd, surface, config, uniforms, bind, paused: false, _win: Win(hwnd) })
}

fn bind_group(gpu: &Gpu, uniforms: &wgpu::Buffer, view: &wgpu::TextureView) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &gpu.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&gpu.sampler) },
        ],
    })
}

fn render(gpu: &Gpu, o: &mut Output, f: &Frame) {
    let frame = match o.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            o.surface.configure(&gpu.device, &o.config);
            return;
        }
        _ => return,
    };
    let u = scene::pack(&o.scene, f);
    let bytes = unsafe { std::slice::from_raw_parts(u.as_ptr() as *const u8, std::mem::size_of_val(&u)) };
    gpu.queue.write_buffer(&o.uniforms, 0, bytes);
    let view = frame.texture.create_view(&Default::default());
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &o.bind, &[]);
        pass.draw(0..3, 0..1);
    }
    gpu.queue.submit([enc.finish()]);
    gpu.queue.present(frame);
}

// --- Pause -------------------------------------------------------------------------------

fn class_of(h: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(h, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// Plein écran, présentation, jeu D3D exclusif : rien n'est visible du bureau.
fn everything_hidden(pause_in_game: bool) -> bool {
    if crate::host::fullscreen_active() {
        return true;
    }
    if let Ok(s) = unsafe { SHQueryUserNotificationState() } {
        if s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE {
            return true;
        }
    }
    pause_in_game && crate::services::activity::activity_state().game.is_some()
}

/// La fenêtre au premier plan couvre-t-elle cet écran (maximisée ou presque) ?
fn covered(d: &DisplayInfo) -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() || matches!(class_of(fg).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return false;
    }
    let mut r = RECT::default();
    if unsafe { GetWindowRect(fg, &mut r) }.is_err() {
        return false;
    }
    let [wx, wy, ww, wh] = d.work;
    let ix = (r.right.min(wx + ww) - r.left.max(wx)).max(0) as i64;
    let iy = (r.bottom.min(wy + wh) - r.top.max(wy)).max(0) as i64;
    let zoomed = unsafe { IsZoomed(fg) }.as_bool();
    (zoomed && ix > 0 && iy > 0) || ix * iy * 100 >= (ww as i64 * wh as i64) * 97
}

// --- Boucle ------------------------------------------------------------------------------

struct Live {
    all: Option<Loaded>,
    screens: BTreeMap<String, Option<Loaded>>,
    /// Une texture par scène affichée (partagée par les écrans qui la montrent).
    textures: HashMap<String, (wgpu::TextureView, (f32, f32))>,
    outputs: Vec<Output>,
}

impl Live {
    fn resolve(&self, d: &DisplayInfo) -> Option<&Loaded> {
        match self.screens.get(&d.id) {
            Some(s) => s.as_ref(),
            None => self.all.as_ref(),
        }
    }

    /// (ondulation, traînée, parallaxe souris, durée max de la traînée) sur l'ensemble des écrans.
    fn interactive(&self) -> (bool, bool, bool, f32) {
        let (mut ripple, mut trail, mut follow, mut fade) = (false, false, false, 0.05f32);
        for o in &self.outputs {
            ripple |= o.scene.interactions.ripple.is_some();
            if let Some(t) = &o.scene.interactions.trail {
                trail = true;
                fade = fade.max(t.fade);
            }
            follow |= o.scene.effects.parallax.as_ref().is_some_and(|p| p.mouse > 0.0);
        }
        (ripple, trail, follow, fade)
    }
}

fn pump() {
    let mut msg = MSG::default();
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// (Re)construit les écrans d'après l'affectation. Un écran qui garde une scène garde sa
/// fenêtre (pas de clignotement) ; un écran qui n'en a plus perd la sienne, et le fond
/// statique est redessiné.
fn layout(gpu: &Gpu, l: &mut Live) -> Result<(), String> {
    let mut old: Vec<Output> = std::mem::take(&mut l.outputs).into_iter().filter(|o| unsafe { IsWindow(Some(o.hwnd)) }.as_bool()).collect();
    let had = old.len();
    let mut kept = 0;
    let mut layer: Option<HWND> = None;
    let mut textures = std::mem::take(&mut l.textures);
    let mut used: HashMap<String, (wgpu::TextureView, (f32, f32))> = HashMap::new();
    let mut out = Vec::new();
    for d in displays() {
        let Some(sc) = l.resolve(&d).cloned() else { continue };
        if !used.contains_key(&sc.id) {
            let tex = textures.remove(&sc.id).unwrap_or_else(|| gpu.texture(sc.image.as_ref()));
            used.insert(sc.id.clone(), tex);
        }
        let tex = &used[&sc.id];
        if let Some(i) = old.iter().position(|o| o.display == d) {
            let mut o = old.swap_remove(i);
            o.scene = sc.scene.clone();
            o.image = tex.1;
            o.bind = bind_group(gpu, &o.uniforms, &tex.0);
            kept += 1;
            out.push(o);
            continue;
        }
        let parent = match layer {
            Some(h) => h,
            None => {
                let h = HWND(desktop::wallpaper_layer().ok_or("noDesktopLayer")? as *mut _);
                layer = Some(h);
                h
            }
        };
        out.push(create_output(gpu, parent, &d, &sc, tex)?);
    }
    l.textures = used;
    l.outputs = out;
    drop(old);
    if kept < had {
        desktop::repaint_wallpaper();
    }
    Ok(())
}

fn run(app: AppHandle, rx: Receiver<Cmd>) {
    let t0 = Instant::now();
    let mut gpu: Option<Gpu> = None;
    let mut live: Option<Live> = None;
    let (mut fps, mut pause_in_game) = (30u32, true);
    let mut last_check = Instant::now();
    let mut last_layout = Instant::now();
    let mut ripples: Vec<(i32, i32, f32)> = Vec::new();
    let mut trail: Vec<(i32, i32, f32)> = Vec::new();
    let mut was_down = false;
    let mut all_paused = false;

    loop {
        // Commandes : bloquant sans scène, sinon on regarde sans attendre.
        let cmd = if live.is_none() {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        } else {
            match rx.recv_timeout(Duration::ZERO) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        match cmd {
            Some(Cmd::Show { all, screens }) => {
                if gpu.is_none() {
                    match Gpu::new() {
                        Ok(g) => gpu = Some(g),
                        Err(e) => {
                            set_status(&app, Status { running: false, paused: false, error: Some(e) });
                            continue;
                        }
                    }
                }
                let g = gpu.as_ref().unwrap();
                let mut l = live.take().unwrap_or(Live { all: None, screens: BTreeMap::new(), textures: HashMap::new(), outputs: Vec::new() });
                // Nouvelle affectation : textures rechargées (l'image a pu changer).
                l.textures.clear();
                l.all = all;
                l.screens = screens;
                match layout(g, &mut l) {
                    Ok(()) if !l.outputs.is_empty() => {
                        live = Some(l);
                        all_paused = false;
                        set_status(&app, Status { running: true, paused: false, error: None });
                    }
                    Ok(()) => set_status(&app, Status::default()),
                    Err(e) => {
                        drop(l);
                        desktop::repaint_wallpaper();
                        set_status(&app, Status { running: false, paused: false, error: Some(e) });
                    }
                }
            }
            Some(Cmd::Stop) => {
                if live.take().is_some() {
                    desktop::repaint_wallpaper();
                }
                ripples.clear();
                trail.clear();
                set_status(&app, Status::default());
                continue;
            }
            Some(Cmd::Options { fps: f, pause_in_game: p }) => {
                fps = f.clamp(15, 60);
                pause_in_game = p;
            }
            None => {}
        }
        let (Some(g), Some(l)) = (gpu.as_ref(), live.as_mut()) else { continue };
        let frame_start = Instant::now();
        pump();

        // Écrans branchés / débranchés, Explorer redémarré : on reconstruit.
        if last_layout.elapsed() > Duration::from_secs(2) {
            last_layout = Instant::now();
            let now: Vec<DisplayInfo> = displays().into_iter().filter(|d| l.resolve(d).is_some()).collect();
            let same = now.len() == l.outputs.len() && now.iter().all(|d| l.outputs.iter().any(|o| o.display == *d));
            let alive = l.outputs.iter().all(|o| unsafe { IsWindow(Some(o.hwnd)) }.as_bool());
            if !same || !alive {
                if let Err(e) = layout(g, l) {
                    live = None;
                    set_status(&app, Status { running: false, paused: false, error: Some(e) });
                    continue;
                }
            }
        }
        // Pause : 2 fois par seconde.
        if last_check.elapsed() > Duration::from_millis(500) {
            last_check = Instant::now();
            let hidden = everything_hidden(pause_in_game);
            for o in &mut l.outputs {
                o.paused = hidden || covered(&o.display);
            }
            let p = l.outputs.iter().all(|o| o.paused);
            if p != all_paused {
                all_paused = p;
                set_status(&app, Status { running: true, paused: p, error: None });
            }
        }
        if all_paused || l.outputs.is_empty() {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }

        // Souris : position, clic (front montant), traînée — seulement sur le bureau visible.
        let t = t0.elapsed().as_secs_f32();
        let (want_ripple, want_trail, follow, max_fade) = l.interactive();
        let on_desktop = if want_ripple || want_trail { desktop::cursor_on_desktop() } else { None };
        let cursor = if follow { desktop::cursor() } else { None };
        let down = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } as u16 & 0x8000 != 0;
        if let Some(p) = on_desktop {
            if want_ripple && down && !was_down {
                ripples.insert(0, (p.x, p.y, t));
                ripples.truncate(scene::RIPPLES);
            }
            if want_trail && trail.first().is_none_or(|(x, y, _)| (x - p.x).abs() + (y - p.y).abs() > 2) {
                trail.insert(0, (p.x, p.y, t));
                trail.truncate(scene::TRAIL);
            }
        }
        was_down = down;
        trail.retain(|(_, _, ts)| t - ts < max_fade + 0.1);
        ripples.retain(|(_, _, ts)| t - ts < 6.0);

        for o in l.outputs.iter_mut().filter(|o| !o.paused) {
            let (dx, dy) = (o.display.x, o.display.y);
            let local = |v: &[(i32, i32, f32)]| v.iter().map(|(x, y, ts)| ((x - dx) as f32, (y - dy) as f32, *ts)).collect::<Vec<_>>();
            let (rl, tl) = (local(&ripples), local(&trail));
            // Parallaxe : le curseur partout (on ne voit le fond que là où il est visible) ;
            // interactions : seulement quand il survole le bureau.
            let mouse = cursor.map(|p| ((p.x - dx) as f32, (p.y - dy) as f32));
            let f = Frame { width: o.config.width as f32, height: o.config.height as f32, image: o.image, time: t, mouse, ripples: &rl, trail: &tl };
            render(g, o, &f);
        }

        let budget = Duration::from_secs_f64(1.0 / fps as f64);
        if let Some(rest) = budget.checked_sub(frame_start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}
