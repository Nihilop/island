// Moteur des fonds animés : UN thread qui possède les fenêtres (une par écran, enfants du
// calque `WorkerW`, cf. `services::desktop`), pompe leurs messages et les dessine avec
// wgpu (DX12 seul). Piloté par messages (`Cmd`) ; sans scène, le thread dort.
//
// Rendu d'un écran, à chaque image :
//   1. les calques visibles, du bas vers le haut, dans une image intermédiaire (couleur
//      prémultipliée ; un pipeline par mode de fusion) — `layer.wgsl` ;
//   2. la passe finale vers la fenêtre (ondulations, traînée, couleur, vignette, grain) —
//      `composite.wgsl`.
//
// Vidéos de calques : un lecteur Media Foundation par fichier (`video.rs`), sur son fil ;
// la dernière image décodée est copiée dans la texture du calque avant chaque rendu.
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
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClassNameW, GetForegroundWindow, GetWindowRect, IsWindow,
    IsZoomed, PeekMessageW, RegisterClassExW, TranslateMessage, MSG, PM_REMOVE, WM_ERASEBKGND, WNDCLASSEXW, WS_CHILD,
    WS_EX_NOACTIVATE, WS_VISIBLE,
};

use super::scene::{self, Frame, Scene};
use crate::services::desktop::windows as desktop;
use crate::services::hardware::{displays, DisplayInfo};

/// Une scène chargée, prête pour le moteur (`dir` = dossier de ses images ; None pour un
/// préréglage, qui n'en a pas).
#[derive(Clone)]
pub struct Loaded {
    pub id: String,
    pub scene: Box<Scene>,
    pub dir: Option<PathBuf>,
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

/// Formats sans sRGB : mêmes calculs de couleur que l'aperçu WebGPU de l'éditeur.
const TEX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

type Tex = (wgpu::TextureView, (f32, f32));

struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Un pipeline de calque par mode de fusion (`scene::blend_index`).
    layer_pipelines: [wgpu::RenderPipeline; 4],
    composite_pipeline: wgpu::RenderPipeline,
    /// Calque : uniformes, image, échantillonneur, masques (calque, eau, chaleur).
    layer_layout: wgpu::BindGroupLayout,
    /// Passe finale : uniformes, image composée, échantillonneur.
    composite_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Texture 1×1 transparente des calques sans image.
    blank: Tex,
    /// Texture 1×1 blanche : « pas de masque » (effet / calque partout).
    white: Tex,
}

/// Couleur prémultipliée : normal, ajout, superposition (screen), produit (multiply).
fn blend_state(i: usize) -> wgpu::BlendState {
    use wgpu::{BlendComponent as C, BlendFactor as F, BlendOperation as O};
    let comp = |src, dst| C { src_factor: src, dst_factor: dst, operation: O::Add };
    let alpha = comp(F::One, F::OneMinusSrcAlpha);
    let color = match i {
        1 => comp(F::One, F::One),
        2 => comp(F::One, F::OneMinusSrc),
        3 => comp(F::Dst, F::OneMinusSrcAlpha),
        _ => comp(F::One, F::OneMinusSrcAlpha),
    };
    wgpu::BlendState { color, alpha }
}

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
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty, count: None };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let sampling = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer"),
            entries: &[entry(0, uniform), entry(1, texture), entry(2, sampling), entry(3, texture), entry(4, texture), entry(5, texture)],
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[entry(0, uniform), entry(1, texture), entry(2, sampling)],
        });
        let layer_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layer_layout)], immediate_size: 0 });
        let composite_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&composite_layout)], immediate_size: 0 });
        let pipeline = |module: &wgpu::ShaderModule, pl: &wgpu::PipelineLayout, format, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(pl),
                vertex: wgpu::VertexState { module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let layer = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("layer"), source: wgpu::ShaderSource::Wgsl(include_str!("layer.wgsl").into()) });
        let composite = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("composite"), source: wgpu::ShaderSource::Wgsl(include_str!("composite.wgsl").into()) });
        let layer_pipelines = [0, 1, 2, 3].map(|i| pipeline(&layer, &layer_pl, TEX_FORMAT, Some(blend_state(i))));
        let composite_pipeline = pipeline(&composite, &composite_pl, SURFACE_FORMAT, None);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let blank = upload(&device, &queue, &image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0])));
        let white = upload(&device, &queue, &image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])));
        Ok(Self { instance, adapter, device, queue, layer_pipelines, composite_pipeline, layer_layout, composite_layout, sampler, blank, white })
    }

    fn upload(&self, rgba: &image::RgbaImage) -> Tex {
        upload(&self.device, &self.queue, rgba)
    }

    /// Image d'un calque, bornée à 8192 px (limite des textures) ; None si illisible.
    fn image(&self, path: &PathBuf) -> Option<Tex> {
        let img = image::open(path).ok()?;
        let img = if img.width() > 8192 || img.height() > 8192 { img.resize(8192, 8192, image::imageops::FilterType::Triangle) } else { img };
        Some(self.upload(&img.to_rgba8()))
    }

    fn bind_composite(&self, buf: &wgpu::Buffer, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    /// Calque : image + masques (calque, eau, chaleur).
    fn bind_layer(&self, buf: &wgpu::Buffer, image: &wgpu::TextureView, masks: [&wgpu::TextureView; 3]) -> wgpu::BindGroup {
        let tex = |binding, view| wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) };
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layer_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                tex(1, image),
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                tex(3, masks[0]),
                tex(4, masks[1]),
                tex(5, masks[2]),
            ],
        })
    }

    fn buffer(&self, vec4s: usize) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (vec4s * 16) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Image intermédiaire (calques composés) d'un écran.
    fn target(&self, w: u32, h: u32) -> wgpu::TextureView {
        self.device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("composite"),
                size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TEX_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, rgba: &image::RgbaImage) -> Tex {
    let (w, h) = rgba.dimensions();
    let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TEX_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
        size,
    );
    (tex.create_view(&Default::default()), (w as f32, h as f32))
}

/// Textures des images de calques, par `<scène>/<fichier>` (partagées entre écrans).
type Textures = HashMap<String, Tex>;

fn file_tex<'a>(textures: &'a Textures, sc: &Loaded, f: Option<&String>) -> Option<&'a Tex> {
    f.and_then(|f| textures.get(&format!("{}/{f}", sc.id)))
}

fn load_textures(gpu: &Gpu, sc: &Loaded, textures: &mut Textures) {
    let Some(dir) = &sc.dir else { return };
    for f in sc.scene.images().filter(|f| !scene::valid_video_name(f)) {
        let key = format!("{}/{f}", sc.id);
        if !textures.contains_key(&key) {
            if let Some(t) = gpu.image(&dir.join(f)) {
                textures.insert(key, t);
            }
        }
    }
}

/// Vidéo d'un calque : son lecteur et la texture qu'il remplit (même clé que `Textures`).
struct Video {
    player: super::video::Player,
    tex: wgpu::Texture,
}

type Videos = HashMap<String, Video>;

/// Lecteurs des vidéos des scènes affichées : ceux déjà ouverts sont gardés, les autres
/// ouverts ; leur texture rejoint `textures` (lue comme une image par les calques).
fn load_videos(gpu: &Gpu, sc: &Loaded, old: &mut Videos, videos: &mut Videos, textures: &mut Textures) {
    let Some(dir) = &sc.dir else { return };
    for l in sc.scene.layers.iter().filter(|l| l.kind == "video") {
        let Some(f) = &l.video else { continue };
        let key = format!("{}/{f}", sc.id);
        if videos.contains_key(&key) {
            continue;
        }
        let v = match old.remove(&key) {
            Some(v) => v,
            None => match super::video::Player::open(&dir.join(f), &l.playback) {
                Ok(player) => {
                    let size = wgpu::Extent3d { width: player.width, height: player.height, depth_or_array_layers: 1 };
                    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("video"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        // RGB32 de Media Foundation = BGRA en mémoire.
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    Video { player, tex }
                }
                Err(e) => {
                    eprintln!("[wallpaper] vidéo {f} : {e}");
                    continue;
                }
            },
        };
        let size = (v.player.width as f32, v.player.height as f32);
        textures.insert(key.clone(), (v.tex.create_view(&Default::default()), size));
        videos.insert(key, v);
    }
}

/// Copie la dernière image de chaque vidéo dans sa texture (rien si elle n'a pas changé).
fn upload_videos(gpu: &Gpu, videos: &Videos) {
    for v in videos.values() {
        if let Some(frame) = v.player.take() {
            let (w, h) = (v.player.width, v.player.height);
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &v.tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &frame,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
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

/// Ressources GPU d'un calque sur un écran.
struct LayerGpu {
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    tex: (f32, f32),
    /// Masques du calque (pour refaire son groupe quand sa texture change : texte).
    masks: [wgpu::TextureView; 3],
}

/// Texte d'un calque sur un écran : la valeur affichée et sa texture.
struct TextGpu {
    shown: String,
    _tex: wgpu::Texture,
}

/// Un écran : sa scène, sa fenêtre enfant du calque, sa surface, son image intermédiaire et
/// les ressources de ses calques. Les champs sont libérés dans l'ordre : la surface AVANT
/// sa fenêtre (`_win` en dernier).
struct Output {
    display: DisplayInfo,
    scene: Box<Scene>,
    layers: Vec<LayerGpu>,
    target: wgpu::TextureView,
    comp_uniforms: wgpu::Buffer,
    comp_bind: wgpu::BindGroup,
    hwnd: HWND,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    paused: bool,
    /// Textes des calques texte (par indice de calque), refaits quand leur valeur change.
    texts: HashMap<usize, TextGpu>,
    _win: Win,
}

impl Output {
    /// (Re)branche une scène sur cet écran : un tampon d'uniformes et une texture par calque.
    fn set_scene(&mut self, gpu: &Gpu, textures: &Textures, sc: &Loaded) {
        self.layers = sc
            .scene
            .layers
            .iter()
            .map(|l| {
                let t = match l.kind.as_str() {
                    "image" => file_tex(textures, sc, l.image.as_ref()),
                    "video" => file_tex(textures, sc, l.video.as_ref()),
                    _ => None,
                }
                .unwrap_or(&gpu.blank);
                let mask = |f: Option<&String>| &file_tex(textures, sc, f).unwrap_or(&gpu.white).0;
                let masks = [
                    mask(l.mask.as_ref()),
                    mask(l.effects.water.as_ref().and_then(|w| w.mask.as_ref())),
                    mask(l.effects.heat.as_ref().and_then(|h| h.mask.as_ref())),
                ];
                let uniforms = gpu.buffer(scene::LAYER_VEC4S);
                let bind = gpu.bind_layer(&uniforms, &t.0, masks);
                LayerGpu { uniforms, bind, tex: t.1, masks: masks.map(|m| m.clone()) }
            })
            .collect();
        self.scene = sc.scene.clone();
        self.texts.clear();
    }

    /// Calques texte : valeur recalculée ; image refaite (à la hauteur de cet écran) si elle a
    /// changé.
    fn update_texts(&mut self, gpu: &Gpu, now: chrono::DateTime<chrono::Utc>, fr: bool) {
        let height = self.config.height as f32;
        for (i, l) in self.scene.layers.iter().enumerate() {
            let Some(style) = l.text.as_ref().filter(|_| l.kind == "text" && l.visible) else { continue };
            let shown = super::text::resolve(&style.content, now, fr);
            if self.texts.get(&i).is_some_and(|t| t.shown == shown) {
                continue;
            }
            let r = super::text::raster(style, &shown, height);
            let Some(img) = image::RgbaImage::from_raw(r.width, r.height, r.rgba) else { continue };
            let (view, size) = gpu.upload(&img);
            let g = &mut self.layers[i];
            let masks = [&g.masks[0], &g.masks[1], &g.masks[2]];
            g.bind = gpu.bind_layer(&g.uniforms, &view, masks);
            g.tex = size;
            // La vue garde la texture en vie ; on la garde aussi, par clarté.
            self.texts.insert(i, TextGpu { shown, _tex: view.texture().clone() });
        }
    }
}

fn create_output(gpu: &Gpu, layer: HWND, d: &DisplayInfo, sc: &Loaded, textures: &Textures) -> Result<Output, String> {
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
    let win = Win(hwnd);
    let handle = raw_window_handle::Win32WindowHandle::new(std::num::NonZeroIsize::new(hwnd.0 as isize).ok_or("hwnd")?);
    let surface = unsafe {
        gpu.instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::new())),
            raw_window_handle: raw_window_handle::RawWindowHandle::Win32(handle),
        })
    }
    .map_err(|e| e.to_string())?;
    let mut config = surface.get_default_config(&gpu.adapter, d.width.max(1), d.height.max(1)).ok_or("surfaceConfig")?;
    config.format = SURFACE_FORMAT;
    config.present_mode = wgpu::PresentMode::Fifo;
    config.desired_maximum_frame_latency = 1;
    surface.configure(&gpu.device, &config);
    let target = gpu.target(config.width, config.height);
    let comp_uniforms = gpu.buffer(scene::COMPOSITE_VEC4S);
    let comp_bind = gpu.bind_composite(&comp_uniforms, &target);
    let mut o = Output {
        display: d.clone(),
        scene: sc.scene.clone(),
        layers: Vec::new(),
        target,
        comp_uniforms,
        comp_bind,
        hwnd,
        surface,
        config,
        paused: false,
        texts: HashMap::new(),
        _win: win,
    };
    o.set_scene(gpu, textures, sc);
    Ok(o)
}

fn bytes<const N: usize>(u: &[[f32; 4]; N]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(u.as_ptr() as *const u8, std::mem::size_of_val(u)) }
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
    for (l, g) in o.scene.layers.iter().zip(&o.layers) {
        if l.visible {
            gpu.queue.write_buffer(&g.uniforms, 0, bytes(&scene::pack_layer(&o.scene, l, f, g.tex)));
        }
    }
    gpu.queue.write_buffer(&o.comp_uniforms, 0, bytes(&scene::pack_composite(&o.scene, f)));
    let view = frame.texture.create_view(&Default::default());
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    let attach = |view, load| wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } };
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layers"),
            color_attachments: &[Some(attach(&o.target, wgpu::LoadOp::Clear(wgpu::Color::BLACK)))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for (l, g) in o.scene.layers.iter().zip(&o.layers) {
            if l.visible {
                pass.set_pipeline(&gpu.layer_pipelines[scene::blend_index(&l.blend)]);
                pass.set_bind_group(0, &g.bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
    }
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(attach(&view, wgpu::LoadOp::Clear(wgpu::Color::BLACK)))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&gpu.composite_pipeline);
        pass.set_bind_group(0, &o.comp_bind, &[]);
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

/// Une app plein écran au premier plan (détection de l'île : le signal brut de Windows
/// reste « occupé » tant qu'un jeu plein écran tourne, même après un Alt+Tab), ou un jeu
/// en cours si l'option est active : on met tout en pause.
fn everything_hidden(pause_in_game: bool) -> bool {
    crate::host::fullscreen_active() || (pause_in_game && crate::services::activity::activity_state().game.is_some())
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
    textures: Textures,
    videos: Videos,
    outputs: Vec<Output>,
    /// Langue d'Island (noms des jours et des mois des textes), lue à chaque affectation.
    fr: bool,
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
            follow |= o.scene.parallax.as_ref().is_some_and(|p| p.mouse > 0.0);
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
    let mut old_videos = std::mem::take(&mut l.videos);
    let mut videos = Videos::new();
    let mut out = Vec::new();
    let screens = displays();
    // Textures et vidéos des scènes affichées seulement (les autres sont libérées).
    let mut used = Textures::new();
    for d in &screens {
        if let Some(sc) = l.resolve(d) {
            for f in sc.scene.images() {
                let key = format!("{}/{f}", sc.id);
                if let Some(t) = textures.remove(&key) {
                    used.insert(key, t);
                }
            }
            load_textures(gpu, sc, &mut used);
            load_videos(gpu, sc, &mut old_videos, &mut videos, &mut used);
        }
    }
    l.videos = videos;
    for d in screens {
        let Some(sc) = l.resolve(&d).cloned() else { continue };
        if let Some(i) = old.iter().position(|o| o.display == d) {
            let mut o = old.swap_remove(i);
            o.set_scene(gpu, &used, &sc);
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
        out.push(create_output(gpu, parent, &d, &sc, &used)?);
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
    let mut last_text = Instant::now() - Duration::from_secs(1);

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
                let mut l = live.take().unwrap_or(Live { all: None, screens: BTreeMap::new(), textures: Textures::new(), videos: Videos::new(), outputs: Vec::new(), fr: true });
                // Nouvelle affectation : images et vidéos rechargées (elles ont pu changer).
                l.textures.clear();
                l.videos.clear();
                l.fr = super::text::french(&app);
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
                for v in l.videos.values() {
                    v.player.set_paused(p);
                }
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

        upload_videos(g, &l.videos);
        // Textes : valeurs recalculées 4 fois par seconde (refaits seulement s'ils changent).
        if last_text.elapsed() >= Duration::from_millis(250) {
            last_text = Instant::now();
            let now = chrono::Utc::now();
            for o in l.outputs.iter_mut().filter(|o| !o.paused) {
                o.update_texts(g, now, l.fr);
            }
        }
        for o in l.outputs.iter_mut().filter(|o| !o.paused) {
            let (dx, dy) = (o.display.x, o.display.y);
            let local = |v: &[(i32, i32, f32)]| v.iter().map(|(x, y, ts)| ((x - dx) as f32, (y - dy) as f32, *ts)).collect::<Vec<_>>();
            let (rl, tl) = (local(&ripples), local(&trail));
            // Parallaxe : le curseur partout (on ne voit le fond que là où il est visible) ;
            // interactions : seulement quand il survole le bureau.
            let mouse = cursor.map(|p| ((p.x - dx) as f32, (p.y - dy) as f32));
            let f = Frame { width: o.config.width as f32, height: o.config.height as f32, time: t, mouse, ripples: &rl, trail: &tl };
            render(g, o, &f);
        }

        let budget = Duration::from_secs_f64(1.0 / fps as f64);
        if let Some(rest) = budget.checked_sub(frame_start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}
