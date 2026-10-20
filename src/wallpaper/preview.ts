// Aperçu WebGPU d'une scène : MÊMES shaders que le fond natif (`layer.wgsl`, `composite.wgsl`,
// importés tels quels), mêmes uniformes (`packLayer`, `packComposite`), même rendu en deux
// temps (calques dans une image intermédiaire, puis passe finale). La souris sur le canvas
// joue le rôle du curseur sur le bureau : parallaxe, traînée, ondulation au clic — sauf
// quand l'éditeur la réserve au déplacement d'un calque (`interactive = false`).

import { current as currentCond, refreshReal, visibleWhen } from "./conditions";
import { invoke } from "@tauri-apps/api/core";
import layerShader from "../../src-tauri/src/wallpaper/layer.wgsl?raw";
import compositeShader from "../../src-tauri/src/wallpaper/composite.wgsl?raw";
import { blendIndex, COMPOSITE_VEC4S, LAYER_VEC4S, maskNames, packComposite, packLayer, RIPPLES, TRAIL, type Frame, type Layer, type Scene } from "./scene";

/* eslint-disable @typescript-eslint/no-explicit-any -- types WebGPU absents du lib DOM. */
type GPU = any;

// Drapeaux WebGPU : ceux des tampons et ceux des textures n'ont PAS les mêmes valeurs
// (COPY_DST vaut 0x8 pour un tampon, 0x2 pour une texture).
const BUFFER = { COPY_DST: 0x8, UNIFORM: 0x40 };
const TEXTURE = { COPY_SRC: 0x1, COPY_DST: 0x2, TEXTURE_BINDING: 0x4, RENDER_ATTACHMENT: 0x10 };
const FORMAT = "rgba8unorm";
const CANVAS_FORMAT = "bgra8unorm";

/** Couleur prémultipliée : normal, ajout, superposition, produit — comme `engine.rs`. */
const BLENDS = [
  { color: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } },
  { color: { srcFactor: "one", dstFactor: "one" } },
  { color: { srcFactor: "one", dstFactor: "one-minus-src" } },
  { color: { srcFactor: "dst", dstFactor: "one-minus-src-alpha" } },
].map((b) => ({ color: { ...b.color, operation: "add" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" } }));

interface Tex {
  view: GPU;
  size: [number, number];
}

export class Preview {
  private device: GPU;
  private ctx: GPU;
  private layerLayout: GPU;
  private compositeLayout: GPU;
  private layerPipelines: GPU[];
  private compositePipeline: GPU;
  private sampler: GPU;
  private blank: Tex;
  /** « Pas de masque ». */
  private white: Tex;
  private target: GPU = null;
  private compUniforms: GPU;
  private compBind: GPU = null;
  private layerRes = new Map<string, { uniforms: GPU; bind: GPU; key: string; tex: [number, number] }>();
  private images = new Map<string, Tex | "loading">();
  /** Textes des calques : image rendue par le Rust (même rendu que le fond), par calque. */
  private texts = new Map<string, { key: string; tex: Tex; at: number; busy: boolean }>();
  /** Vidéos des calques : balise <video> en boucle, copiée dans sa texture à chaque image. */
  private videos = new Map<string, { el: HTMLVideoElement; url: string; texture: GPU | null; tex: Tex | null; start: number }>();
  private raf = 0;
  private t0 = performance.now();
  private mouse: [number, number] | null = null;
  private ripples: [number, number, number][] = [];
  private trail: [number, number, number][] = [];
  private ro: ResizeObserver;
  /** Conditions : fondu en cours par calque, et horloge des fondus. */
  private presence = new Map<string, number>();
  private lastFrame = performance.now();
  private condTimer = 0;
  private off: (() => void)[] = [];
  /** La souris simule le bureau (sinon l'éditeur s'en sert pour déplacer un calque). */
  interactive = true;

  private constructor(
    private canvas: HTMLCanvasElement,
    device: GPU,
    private scene: Scene,
    private loadImage: (name: string) => Promise<ImageBitmap>,
    private loadVideo: (name: string) => Promise<string>,
  ) {
    this.device = device;
    this.ctx = canvas.getContext("webgpu");
    this.ctx.configure({ device, format: CANVAS_FORMAT, alphaMode: "opaque" });
    // Visibilité 2 = FRAGMENT. Calque : + masques (calque, eau, chaleur) en 3, 4, 5.
    const tex = (binding: number) => ({ binding, visibility: 2, texture: { sampleType: "float" } });
    const base = [{ binding: 0, visibility: 2, buffer: { type: "uniform" } }, tex(1), { binding: 2, visibility: 2, sampler: { type: "filtering" } }];
    this.layerLayout = device.createBindGroupLayout({ entries: [...base, tex(3), tex(4), tex(5)] });
    this.compositeLayout = device.createBindGroupLayout({ entries: base });
    const layerModule = device.createShaderModule({ code: layerShader });
    const compModule = device.createShaderModule({ code: compositeShader });
    const pipeline = (module: GPU, layout: GPU, format: string, blend?: unknown) =>
      device.createRenderPipeline({
        layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
        vertex: { module, entryPoint: "vs" },
        fragment: { module, entryPoint: "fs", targets: [{ format, ...(blend ? { blend } : {}) }] },
        primitive: { topology: "triangle-list" },
      });
    this.layerPipelines = BLENDS.map((b) => pipeline(layerModule, this.layerLayout, FORMAT, b));
    this.compositePipeline = pipeline(compModule, this.compositeLayout, CANVAS_FORMAT);
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
    this.blank = this.upload(null);
    this.white = this.upload(null, [255, 255, 255, 255]);
    this.compUniforms = device.createBuffer({ size: COMPOSITE_VEC4S * 16, usage: BUFFER.UNIFORM | BUFFER.COPY_DST });
    this.ro = new ResizeObserver(() => this.resize());
    this.ro.observe(canvas);
    this.resize();
    this.listen();
  }

  /** null si WebGPU est indisponible (vieux pilote, WebView2 trop ancien). */
  static async create(
    canvas: HTMLCanvasElement,
    scene: Scene,
    loadImage: (name: string) => Promise<ImageBitmap>,
    loadVideo: (name: string) => Promise<string>,
  ): Promise<Preview | null> {
    const gpu = (navigator as any).gpu;
    if (!gpu) return null;
    const adapter = await gpu.requestAdapter({ powerPreference: "low-power" }).catch(() => null);
    const device = await adapter?.requestDevice().catch(() => null);
    if (!device) return null;
    const p = new Preview(canvas, device, scene, loadImage, loadVideo);
    // État réel des conditions (moment, ciel), relu toutes les 15 s.
    void refreshReal();
    p.condTimer = window.setInterval(() => void refreshReal(), 15_000);
    p.loop();
    return p;
  }

  setScene(s: Scene) {
    this.scene = s;
  }

  /** Oublie les images et vidéos en cache (scène rechargée, image remplacée). */
  resetImages() {
    this.images.clear();
    this.layerRes.clear();
    this.texts.clear();
    this.dropVideos();
  }

  private dropVideos() {
    for (const v of this.videos.values()) {
      v.el.pause();
      v.el.removeAttribute("src");
      URL.revokeObjectURL(v.url);
      v.texture?.destroy();
    }
    this.videos.clear();
  }

  /** Taille de l'image (ou de la vidéo) d'un calque (1×1 tant qu'elle n'est pas chargée). */
  texSize(l: Layer): [number, number] {
    if (l.kind === "video") return (l.video && this.videos.get(l.video)?.tex?.size) || [1, 1];
    if (l.kind === "text") return this.texts.get(l.id)?.tex.size ?? [1, 1];
    const t = l.image ? this.images.get(l.image) : null;
    return t && t !== "loading" ? t.size : [1, 1];
  }

  /** Texture d'une vidéo de calque ; la balise est créée au premier appel. */
  private video(l: Layer): Tex {
    const name = l.video;
    if (!name) return this.blank;
    let v = this.videos.get(name);
    if (!v) {
      const el = document.createElement("video");
      el.muted = true;
      el.playsInline = true;
      el.preload = "auto";
      const entry = { el, url: "", texture: null as GPU | null, tex: null as Tex | null, start: l.playback.start };
      v = entry;
      this.videos.set(name, entry);
      el.addEventListener("loadedmetadata", () => {
        const [w, h] = [el.videoWidth || 1, el.videoHeight || 1];
        entry.texture = this.device.createTexture({ size: [w, h], format: FORMAT, usage: TEXTURE.TEXTURE_BINDING | TEXTURE.COPY_DST | TEXTURE.RENDER_ATTACHMENT });
        entry.tex = { view: entry.texture.createView(), size: [w, h] };
        el.currentTime = Math.min(entry.start, el.duration || 0);
        void el.play().catch(() => {});
      });
      // Boucle « à la main » : elle repart du début choisi, pas de zéro.
      el.addEventListener("ended", () => {
        el.currentTime = entry.start < (el.duration || 0) ? entry.start : 0;
        void el.play().catch(() => {});
      });
      void this.loadVideo(name)
        .then((url) => {
          entry.url = url;
          el.src = url;
        })
        .catch(() => this.videos.delete(name));
    }
    // Réglages modifiés dans l'éditeur.
    const speed = Math.min(4, Math.max(0.25, l.playback.speed || 1));
    if (v.el.playbackRate !== speed) v.el.playbackRate = speed;
    if (v.start !== l.playback.start) {
      v.start = Math.max(0, l.playback.start || 0);
      if (v.el.readyState > 0) v.el.currentTime = Math.min(v.start, v.el.duration || 0);
    }
    return v.tex ?? this.blank;
  }

  /** Texture d'un texte : redemandée quand le style ou la taille changent, et chaque seconde
   * (variables : heure, météo…). */
  private text(l: Layer): Tex {
    if (!l.text) return this.blank;
    const key = `${JSON.stringify(l.text)}|${this.canvas.height}`;
    let t = this.texts.get(l.id);
    const now = performance.now();
    if (!t || (!t.busy && (t.key !== key || now - t.at > 1000))) {
      const entry = t ?? { key, tex: this.blank, at: 0, busy: false };
      entry.busy = true;
      entry.key = key;
      entry.at = now;
      this.texts.set(l.id, entry);
      t = entry;
      void invoke<ArrayBuffer>("wallpaper_text_raster", { style: l.text, height: this.canvas.height })
        .then((buf) => {
          const head = new DataView(buf, 0, 8);
          const [w, h] = [head.getUint32(0, true), head.getUint32(4, true)];
          const tex = this.device.createTexture({ size: [w, h], format: FORMAT, usage: TEXTURE.TEXTURE_BINDING | TEXTURE.COPY_DST });
          this.device.queue.writeTexture({ texture: tex }, new Uint8Array(buf, 8), { bytesPerRow: w * 4 }, [w, h]);
          entry.tex = { view: tex.createView(), size: [w, h] };
        })
        .catch(() => {})
        .finally(() => (entry.busy = false));
    }
    return t.tex;
  }

  /** Image, vidéo ou texte affiché par un calque. */
  private layerTex(l: Layer): Tex {
    if (l.kind === "video") return this.video(l);
    if (l.kind === "text") return this.text(l);
    return this.image(l.kind === "image" ? l.image : null);
  }

  /** Copie l'image courante de chaque vidéo dans sa texture. */
  private uploadVideos() {
    for (const v of this.videos.values()) {
      if (v.texture && v.tex && v.el.readyState >= 2) {
        this.device.queue.copyExternalImageToTexture({ source: v.el }, { texture: v.texture }, v.tex.size);
      }
    }
  }

  /** Taille du canvas en pixels (celle des uniformes). */
  pixelSize(): [number, number] {
    return [this.canvas.width, this.canvas.height];
  }

  /** Remplace une image / un masque en cache (masque tout juste enregistré). */
  putImage(name: string, bitmap: ImageBitmap) {
    this.images.set(name, this.upload(bitmap));
  }

  private overrides = new Map<string, Tex>();
  /** Masque en cours de peinture (slot 0 calque, 1 eau, 2 chaleur) ; null = retiré. */
  setOverride(layerId: string, slot: number, bitmap: ImageBitmap | null) {
    const k = `${layerId}:${slot}`;
    if (bitmap) this.overrides.set(k, this.upload(bitmap));
    else this.overrides.delete(k);
  }

  private upload(bitmap: ImageBitmap | null, pixel = [0, 0, 0, 0]): Tex {
    const [w, h] = bitmap ? [bitmap.width, bitmap.height] : [1, 1];
    const tex = this.device.createTexture({ size: [w, h], format: FORMAT, usage: TEXTURE.TEXTURE_BINDING | TEXTURE.COPY_DST | TEXTURE.RENDER_ATTACHMENT });
    if (bitmap) this.device.queue.copyExternalImageToTexture({ source: bitmap }, { texture: tex }, [w, h]);
    else this.device.queue.writeTexture({ texture: tex }, new Uint8Array(pixel), { bytesPerRow: 4 }, [1, 1]);
    return { view: tex.createView(), size: [w, h] };
  }

  private image(name: string | null, fallback = this.blank): Tex {
    if (!name) return fallback;
    const t = this.images.get(name);
    if (t && t !== "loading") return t;
    if (!t) {
      this.images.set(name, "loading");
      void this.loadImage(name)
        .then((b) => this.images.set(name, this.upload(b)))
        .catch(() => this.images.delete(name));
    }
    return fallback;
  }

  private bind(layout: GPU, buffer: GPU, views: GPU[]) {
    const [image, ...masks] = views;
    return this.device.createBindGroup({
      layout,
      entries: [
        { binding: 0, resource: { buffer } },
        { binding: 1, resource: image },
        { binding: 2, resource: this.sampler },
        ...masks.map((v, i) => ({ binding: 3 + i, resource: v })),
      ],
    });
  }

  /** Ressources d'un calque (tampon, image, masques), recréées quand l'une change. */
  private res(l: Layer) {
    const tex = this.layerTex(l);
    const masks = maskNames(l).map((m, i) => this.overrides.get(`${l.id}:${i}`) ?? this.image(m, this.white));
    let r = this.layerRes.get(l.id);
    // Clé : les textures elles-mêmes (une image qui finit de charger change sa vue).
    const views = [tex, ...masks];
    const key = views.map((v) => this.viewId(v)).join("|");
    if (!r || r.key !== key) {
      const uniforms = r?.uniforms ?? this.device.createBuffer({ size: LAYER_VEC4S * 16, usage: BUFFER.UNIFORM | BUFFER.COPY_DST });
      r = { uniforms, bind: this.bind(this.layerLayout, uniforms, views.map((v) => v.view)), key, tex: tex.size };
      this.layerRes.set(l.id, r);
    }
    return r;
  }

  private ids = new WeakMap<Tex, number>();
  private nextId = 1;
  private viewId(t: Tex): number {
    let id = this.ids.get(t);
    if (!id) this.ids.set(t, (id = this.nextId++));
    return id;
  }

  private resize() {
    const r = this.canvas.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.canvas.width = Math.max(1, Math.round(r.width * dpr));
    this.canvas.height = Math.max(1, Math.round(r.height * dpr));
    const target = this.device.createTexture({ size: [this.canvas.width, this.canvas.height], format: FORMAT, usage: TEXTURE.TEXTURE_BINDING | TEXTURE.RENDER_ATTACHMENT });
    this.target = target.createView();
    this.compBind = this.bind(this.compositeLayout, this.compUniforms, [this.target]);
  }

  private now() {
    return (performance.now() - this.t0) / 1000;
  }

  private listen() {
    const pos = (e: PointerEvent): [number, number] => {
      const r = this.canvas.getBoundingClientRect();
      return [((e.clientX - r.left) / r.width) * this.canvas.width, ((e.clientY - r.top) / r.height) * this.canvas.height];
    };
    const move = (e: PointerEvent) => {
      if (!this.interactive) return;
      const p = pos(e);
      this.mouse = p;
      const last = this.trail[0];
      if (!last || Math.abs(last[0] - p[0]) + Math.abs(last[1] - p[1]) > 2) {
        this.trail.unshift([p[0], p[1], this.now()]);
        this.trail.length = Math.min(this.trail.length, TRAIL);
      }
    };
    const down = (e: PointerEvent) => {
      if (!this.interactive) return;
      const p = pos(e);
      this.ripples.unshift([p[0], p[1], this.now()]);
      this.ripples.length = Math.min(this.ripples.length, RIPPLES);
    };
    const leave = () => (this.mouse = null);
    const el = this.canvas.parentElement ?? this.canvas;
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerdown", down);
    el.addEventListener("pointerleave", leave);
    this.off.push(() => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerdown", down);
      el.removeEventListener("pointerleave", leave);
    });
  }

  private loop = () => {
    this.raf = requestAnimationFrame(this.loop);
    if (!this.compBind || document.hidden) return;
    const t = this.now();
    const s = this.scene;
    const fade = s.interactions.trail?.fade ?? 0;
    this.trail = this.trail.filter(([, , ts]) => t - ts < Math.max(fade, 0.05) + 0.1);
    this.ripples = this.ripples.filter(([, , ts]) => t - ts < 6);
    const f: Frame = {
      width: this.canvas.width,
      height: this.canvas.height,
      time: t,
      mouse: this.mouse,
      ripples: s.interactions.ripple ? this.ripples : [],
      trail: s.interactions.trail ? this.trail : [],
    };
    // Conditions : même règle que le fond (état réel, ou simulé dans Scène), mêmes fondus.
    const nowMs = performance.now();
    const dt = Math.min(0.5, (nowMs - this.lastFrame) / 1000);
    this.lastFrame = nowMs;
    const cond = currentCond();
    for (const l of s.layers) {
      if (!l.when) continue;
      const target = visibleWhen(l.when, cond) ? 1 : 0;
      const p = this.presence.get(l.id) ?? target;
      const step = l.when.fade <= 0.05 ? 1 : dt / l.when.fade;
      this.presence.set(l.id, target > p ? Math.min(1, p + step) : Math.max(0, p - step));
    }
    const presenceOf = (l: Layer) => (l.when ? (this.presence.get(l.id) ?? 1) : 1);
    const visible = s.layers.filter((l) => l.visible && presenceOf(l) > 0.001);
    const res = visible.map((l) => this.res(l));
    this.uploadVideos();
    visible.forEach((l, i) => this.device.queue.writeBuffer(res[i].uniforms, 0, packLayer(s, l, f, res[i].tex, presenceOf(l))));
    this.device.queue.writeBuffer(this.compUniforms, 0, packComposite(s, f));
    const enc = this.device.createCommandEncoder();
    const layers = enc.beginRenderPass({ colorAttachments: [{ view: this.target, loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 1] }] });
    visible.forEach((l, i) => {
      layers.setPipeline(this.layerPipelines[blendIndex(l.blend)]);
      layers.setBindGroup(0, res[i].bind);
      layers.draw(3);
    });
    layers.end();
    const comp = enc.beginRenderPass({ colorAttachments: [{ view: this.ctx.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 1] }] });
    comp.setPipeline(this.compositePipeline);
    comp.setBindGroup(0, this.compBind);
    comp.draw(3);
    comp.end();
    this.device.queue.submit([enc.finish()]);
  };

  /**
   * Rendu de la scène en PNG (vignette du magasin et de l'outil) : mêmes passes que
   * l'aperçu, dans des textures à part, sans souris ni interactions, à `time` secondes
   * d'animation (particules en place). Attend la fin du chargement des images.
   */
  async snapshot(width = 960, height = 540, time = 3): Promise<Blob> {
    const pending = () => [...this.images.values()].includes("loading") || [...this.videos.values()].some((v) => !v.tex || v.el.readyState < 2);
    for (let i = 0; i < 60 && pending(); i++) await new Promise((r) => setTimeout(r, 50));
    this.uploadVideos();
    const d = this.device;
    const s = this.scene;
    const f: Frame = { width, height, time, mouse: null, ripples: [], trail: [] };
    const target = d.createTexture({ size: [width, height], format: FORMAT, usage: TEXTURE.TEXTURE_BINDING | TEXTURE.RENDER_ATTACHMENT });
    const out = d.createTexture({ size: [width, height], format: CANVAS_FORMAT, usage: TEXTURE.RENDER_ATTACHMENT | TEXTURE.COPY_SRC });
    const comp = d.createBuffer({ size: COMPOSITE_VEC4S * 16, usage: BUFFER.UNIFORM | BUFFER.COPY_DST });
    d.queue.writeBuffer(comp, 0, packComposite(s, f));
    const enc = d.createCommandEncoder();
    const layers = enc.beginRenderPass({ colorAttachments: [{ view: target.createView(), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 1] }] });
    // Vignette : la scène telle qu'elle s'affiche maintenant (conditions comprises).
    const presenceOf = (l: Layer) => (l.when ? (this.presence.get(l.id) ?? 1) : 1);
    for (const l of s.layers.filter((x) => x.visible && presenceOf(x) > 0.001)) {
      // Tampon à part : l'aperçu continue d'écrire les siens à chaque image.
      const r = this.res(l);
      const buf = d.createBuffer({ size: LAYER_VEC4S * 16, usage: BUFFER.UNIFORM | BUFFER.COPY_DST });
      d.queue.writeBuffer(buf, 0, packLayer(s, l, f, r.tex, presenceOf(l)));
      const tex = this.layerTex(l);
      const masks = maskNames(l).map((m) => this.image(m, this.white));
      layers.setPipeline(this.layerPipelines[blendIndex(l.blend)]);
      layers.setBindGroup(0, this.bind(this.layerLayout, buf, [tex, ...masks].map((v) => v.view)));
      layers.draw(3);
    }
    layers.end();
    const final = enc.beginRenderPass({ colorAttachments: [{ view: out.createView(), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 1] }] });
    final.setPipeline(this.compositePipeline);
    final.setBindGroup(0, this.bind(this.compositeLayout, comp, [target.createView()]));
    final.draw(3);
    final.end();
    // Relecture : lignes alignées sur 256 octets (exigence de WebGPU).
    const row = Math.ceil((width * 4) / 256) * 256;
    const read = d.createBuffer({ size: row * height, usage: BUFFER.COPY_DST | 0x1 }); // MAP_READ
    enc.copyTextureToBuffer({ texture: out }, { buffer: read, bytesPerRow: row }, [width, height]);
    d.queue.submit([enc.finish()]);
    await read.mapAsync(0x1); // GPUMapMode.READ
    const src = new Uint8Array(read.getMappedRange());
    const img = new ImageData(width, height);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const i = y * row + x * 4;
        const o = (y * width + x) * 4;
        img.data[o] = src[i + 2]; // BGRA → RGBA
        img.data[o + 1] = src[i + 1];
        img.data[o + 2] = src[i];
        img.data[o + 3] = 255;
      }
    }
    read.unmap();
    [target, out, comp, read].forEach((x) => x.destroy());
    const canvas = new OffscreenCanvas(width, height);
    canvas.getContext("2d")!.putImageData(img, 0, 0);
    return canvas.convertToBlob({ type: "image/png" });
  }

  destroy() {
    cancelAnimationFrame(this.raf);
    window.clearInterval(this.condTimer);
    this.dropVideos();
    this.ro.disconnect();
    this.off.forEach((f) => f());
    this.device.destroy?.();
  }
}
