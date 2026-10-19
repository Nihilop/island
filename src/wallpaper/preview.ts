// Aperçu WebGPU d'une scène : MÊMES shaders que le fond natif (`layer.wgsl`, `composite.wgsl`,
// importés tels quels), mêmes uniformes (`packLayer`, `packComposite`), même rendu en deux
// temps (calques dans une image intermédiaire, puis passe finale). La souris sur le canvas
// joue le rôle du curseur sur le bureau : parallaxe, traînée, ondulation au clic — sauf
// quand l'éditeur la réserve au déplacement d'un calque (`interactive = false`).

import layerShader from "../../src-tauri/src/wallpaper/layer.wgsl?raw";
import compositeShader from "../../src-tauri/src/wallpaper/composite.wgsl?raw";
import { blendIndex, COMPOSITE_VEC4S, LAYER_VEC4S, packComposite, packLayer, RIPPLES, TRAIL, type Frame, type Layer, type Scene } from "./scene";

/* eslint-disable @typescript-eslint/no-explicit-any -- types WebGPU absents du lib DOM. */
type GPU = any;

const USAGE = { UNIFORM: 0x40, COPY_DST: 0x8, TEXTURE_BINDING: 0x4, RENDER_ATTACHMENT: 0x10 };
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
  private layout: GPU;
  private layerPipelines: GPU[];
  private compositePipeline: GPU;
  private sampler: GPU;
  private blank: Tex;
  private target: GPU = null;
  private compUniforms: GPU;
  private compBind: GPU = null;
  private layerRes = new Map<string, { uniforms: GPU; bind: GPU; image: string | null; tex: [number, number] }>();
  private images = new Map<string, Tex | "loading">();
  private raf = 0;
  private t0 = performance.now();
  private mouse: [number, number] | null = null;
  private ripples: [number, number, number][] = [];
  private trail: [number, number, number][] = [];
  private ro: ResizeObserver;
  private off: (() => void)[] = [];
  /** La souris simule le bureau (sinon l'éditeur s'en sert pour déplacer un calque). */
  interactive = true;

  private constructor(
    private canvas: HTMLCanvasElement,
    device: GPU,
    private scene: Scene,
    private loadImage: (name: string) => Promise<ImageBitmap>,
  ) {
    this.device = device;
    this.ctx = canvas.getContext("webgpu");
    this.ctx.configure({ device, format: CANVAS_FORMAT, alphaMode: "opaque" });
    this.layout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: 2, buffer: { type: "uniform" } }, // FRAGMENT
        { binding: 1, visibility: 2, texture: { sampleType: "float" } },
        { binding: 2, visibility: 2, sampler: { type: "filtering" } },
      ],
    });
    const pl = device.createPipelineLayout({ bindGroupLayouts: [this.layout] });
    const layerModule = device.createShaderModule({ code: layerShader });
    const compModule = device.createShaderModule({ code: compositeShader });
    const pipeline = (module: GPU, format: string, blend?: unknown) =>
      device.createRenderPipeline({
        layout: pl,
        vertex: { module, entryPoint: "vs" },
        fragment: { module, entryPoint: "fs", targets: [{ format, ...(blend ? { blend } : {}) }] },
        primitive: { topology: "triangle-list" },
      });
    this.layerPipelines = BLENDS.map((b) => pipeline(layerModule, FORMAT, b));
    this.compositePipeline = pipeline(compModule, CANVAS_FORMAT);
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
    this.blank = this.upload(null);
    this.compUniforms = device.createBuffer({ size: COMPOSITE_VEC4S * 16, usage: USAGE.UNIFORM | USAGE.COPY_DST });
    this.ro = new ResizeObserver(() => this.resize());
    this.ro.observe(canvas);
    this.resize();
    this.listen();
  }

  /** null si WebGPU est indisponible (vieux pilote, WebView2 trop ancien). */
  static async create(canvas: HTMLCanvasElement, scene: Scene, loadImage: (name: string) => Promise<ImageBitmap>): Promise<Preview | null> {
    const gpu = (navigator as any).gpu;
    if (!gpu) return null;
    const adapter = await gpu.requestAdapter({ powerPreference: "low-power" }).catch(() => null);
    const device = await adapter?.requestDevice().catch(() => null);
    if (!device) return null;
    const p = new Preview(canvas, device, scene, loadImage);
    p.loop();
    return p;
  }

  setScene(s: Scene) {
    this.scene = s;
  }

  /** Oublie les images en cache (scène rechargée, image remplacée). */
  resetImages() {
    this.images.clear();
    this.layerRes.clear();
  }

  /** Taille de l'image d'un calque (1×1 tant qu'elle n'est pas chargée). */
  texSize(l: Layer): [number, number] {
    const t = l.image ? this.images.get(l.image) : null;
    return t && t !== "loading" ? t.size : [1, 1];
  }

  /** Taille du canvas en pixels (celle des uniformes). */
  pixelSize(): [number, number] {
    return [this.canvas.width, this.canvas.height];
  }

  private upload(bitmap: ImageBitmap | null): Tex {
    const [w, h] = bitmap ? [bitmap.width, bitmap.height] : [1, 1];
    const tex = this.device.createTexture({ size: [w, h], format: FORMAT, usage: USAGE.TEXTURE_BINDING | USAGE.COPY_DST | USAGE.RENDER_ATTACHMENT });
    if (bitmap) this.device.queue.copyExternalImageToTexture({ source: bitmap }, { texture: tex }, [w, h]);
    else this.device.queue.writeTexture({ texture: tex }, new Uint8Array([0, 0, 0, 0]), { bytesPerRow: 4 }, [1, 1]);
    return { view: tex.createView(), size: [w, h] };
  }

  private image(name: string | null): Tex {
    if (!name) return this.blank;
    const t = this.images.get(name);
    if (t && t !== "loading") return t;
    if (!t) {
      this.images.set(name, "loading");
      void this.loadImage(name)
        .then((b) => this.images.set(name, this.upload(b)))
        .catch(() => this.images.delete(name));
    }
    return this.blank;
  }

  private bind(buffer: GPU, view: GPU) {
    return this.device.createBindGroup({
      layout: this.layout,
      entries: [
        { binding: 0, resource: { buffer } },
        { binding: 1, resource: view },
        { binding: 2, resource: this.sampler },
      ],
    });
  }

  /** Ressources d'un calque (tampon + texture), recréées quand son image change. */
  private res(l: Layer) {
    const tex = this.image(l.kind === "image" ? l.image : null);
    let r = this.layerRes.get(l.id);
    if (!r || r.image !== l.image || r.tex !== tex.size) {
      const uniforms = r?.uniforms ?? this.device.createBuffer({ size: LAYER_VEC4S * 16, usage: USAGE.UNIFORM | USAGE.COPY_DST });
      r = { uniforms, bind: this.bind(uniforms, tex.view), image: l.image, tex: tex.size };
      this.layerRes.set(l.id, r);
    }
    return r;
  }

  private resize() {
    const r = this.canvas.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.canvas.width = Math.max(1, Math.round(r.width * dpr));
    this.canvas.height = Math.max(1, Math.round(r.height * dpr));
    const target = this.device.createTexture({ size: [this.canvas.width, this.canvas.height], format: FORMAT, usage: USAGE.TEXTURE_BINDING | USAGE.RENDER_ATTACHMENT });
    this.target = target.createView();
    this.compBind = this.bind(this.compUniforms, this.target);
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
    const visible = s.layers.filter((l) => l.visible);
    const res = visible.map((l) => this.res(l));
    visible.forEach((l, i) => this.device.queue.writeBuffer(res[i].uniforms, 0, packLayer(s, l, f, res[i].tex)));
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

  destroy() {
    cancelAnimationFrame(this.raf);
    this.ro.disconnect();
    this.off.forEach((f) => f());
    this.device.destroy?.();
  }
}
