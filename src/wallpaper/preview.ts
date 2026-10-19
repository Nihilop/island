// Aperçu WebGPU d'une scène : MÊME shader que le fond natif (`scene.wgsl`, importé tel
// quel) et mêmes uniformes (`pack`). La souris sur le canvas joue le rôle du curseur sur le
// bureau : parallaxe, traînée, ondulation au clic.

import shader from "../../src-tauri/src/wallpaper/scene.wgsl?raw";
import { pack, RIPPLES, TRAIL, VEC4S, type Scene } from "./scene";

/* eslint-disable @typescript-eslint/no-explicit-any -- types WebGPU absents du lib DOM. */
type GPU = any;

export class Preview {
  private device: GPU;
  private ctx: GPU;
  private pipeline: GPU;
  private uniforms: GPU;
  private sampler: GPU;
  private bind: GPU = null;
  private image: [number, number] = [1, 1];
  private raf = 0;
  private t0 = performance.now();
  private mouse: [number, number] | null = null;
  private ripples: [number, number, number][] = [];
  private trail: [number, number, number][] = [];
  private scene: Scene;
  private ro: ResizeObserver;
  private off: (() => void)[] = [];

  private constructor(private canvas: HTMLCanvasElement, device: GPU, scene: Scene) {
    this.device = device;
    this.scene = scene;
    this.ctx = canvas.getContext("webgpu");
    this.ctx.configure({ device, format: "bgra8unorm", alphaMode: "opaque" });
    const module = device.createShaderModule({ code: shader });
    this.pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: { module, entryPoint: "fs", targets: [{ format: "bgra8unorm" }] },
      primitive: { topology: "triangle-list" },
    });
    this.uniforms = device.createBuffer({ size: VEC4S * 16, usage: 0x40 | 0x8 }); // UNIFORM | COPY_DST
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
    this.ro = new ResizeObserver(() => this.resize());
    this.ro.observe(canvas);
    this.resize();
    this.listen();
  }

  /** null si WebGPU est indisponible (vieux pilote, WebView2 trop ancien). */
  static async create(canvas: HTMLCanvasElement, scene: Scene): Promise<Preview | null> {
    const gpu = (navigator as any).gpu;
    if (!gpu) return null;
    const adapter = await gpu.requestAdapter({ powerPreference: "low-power" }).catch(() => null);
    const device = await adapter?.requestDevice().catch(() => null);
    if (!device) return null;
    const p = new Preview(canvas, device, scene);
    await p.setImage(null);
    p.loop();
    return p;
  }

  setScene(s: Scene) {
    this.scene = s;
  }

  /** Image de fond (null = 1×1 noir, pour un dégradé). */
  async setImage(bitmap: ImageBitmap | null) {
    const [w, h] = bitmap ? [bitmap.width, bitmap.height] : [1, 1];
    const tex = this.device.createTexture({ size: [w, h], format: "rgba8unorm", usage: 0x4 | 0x2 | 0x10 }); // TEXTURE_BINDING | COPY_DST | RENDER_ATTACHMENT
    if (bitmap) this.device.queue.copyExternalImageToTexture({ source: bitmap }, { texture: tex }, [w, h]);
    else this.device.queue.writeTexture({ texture: tex }, new Uint8Array([0, 0, 0, 255]), { bytesPerRow: 4 }, [1, 1]);
    this.image = [w, h];
    this.bind = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.uniforms } },
        { binding: 1, resource: tex.createView() },
        { binding: 2, resource: this.sampler },
      ],
    });
  }

  private resize() {
    const r = this.canvas.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.canvas.width = Math.max(1, Math.round(r.width * dpr));
    this.canvas.height = Math.max(1, Math.round(r.height * dpr));
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
      const p = pos(e);
      this.mouse = p;
      const last = this.trail[0];
      if (!last || Math.abs(last[0] - p[0]) + Math.abs(last[1] - p[1]) > 2) {
        this.trail.unshift([p[0], p[1], this.now()]);
        this.trail.length = Math.min(this.trail.length, TRAIL);
      }
    };
    const down = (e: PointerEvent) => {
      const p = pos(e);
      this.ripples.unshift([p[0], p[1], this.now()]);
      this.ripples.length = Math.min(this.ripples.length, RIPPLES);
    };
    const leave = () => (this.mouse = null);
    this.canvas.addEventListener("pointermove", move);
    this.canvas.addEventListener("pointerdown", down);
    this.canvas.addEventListener("pointerleave", leave);
    this.off.push(() => {
      this.canvas.removeEventListener("pointermove", move);
      this.canvas.removeEventListener("pointerdown", down);
      this.canvas.removeEventListener("pointerleave", leave);
    });
  }

  private loop = () => {
    this.raf = requestAnimationFrame(this.loop);
    if (!this.bind || document.hidden) return;
    const t = this.now();
    const s = this.scene;
    const fade = s.interactions.trail?.fade ?? 0;
    this.trail = this.trail.filter(([, , ts]) => t - ts < Math.max(fade, 0.05) + 0.1);
    this.ripples = this.ripples.filter(([, , ts]) => t - ts < 6);
    const u = pack(s, {
      width: this.canvas.width,
      height: this.canvas.height,
      image: this.image,
      time: t,
      mouse: this.mouse,
      ripples: s.interactions.ripple ? this.ripples : [],
      trail: s.interactions.trail ? this.trail : [],
    });
    this.device.queue.writeBuffer(this.uniforms, 0, u);
    const enc = this.device.createCommandEncoder();
    const pass = enc.beginRenderPass({
      colorAttachments: [{ view: this.ctx.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 1] }],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bind);
    pass.draw(3);
    pass.end();
    this.device.queue.submit([enc.finish()]);
  };

  destroy() {
    cancelAnimationFrame(this.raf);
    this.ro.disconnect();
    this.off.forEach((f) => f());
    this.device.destroy?.();
  }
}
