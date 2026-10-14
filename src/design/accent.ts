// Couleur d'accent dynamique tirée de la pochette. Image réduite à 24 × 24 ; chaque pixel
// pèse saturation² × (1 − |luminosité − 0,5| × 2) : les pixels colorés et de luminosité
// moyenne comptent, gris/noir/blanc presque pas. Moyenne pondérée → HSL ; trop peu saturé
// → blanc (évite les couleurs boueuses), sinon saturation renforcée et luminosité bornée
// pour bien ressortir sur noir. Pendant le calcul on garde l'accent précédent.

const DEFAULT = "#0a84ff";
const cache = new Map<string, string>();

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h / 6, s, l];
}

async function compute(url: string): Promise<string> {
  const img = new Image();
  img.src = url;
  await img.decode();
  const c = document.createElement("canvas");
  c.width = c.height = 24;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return DEFAULT;
  ctx.drawImage(img, 0, 0, 24, 24);
  const px = ctx.getImageData(0, 0, 24, 24).data;
  let wr = 0;
  let wg = 0;
  let wb = 0;
  let wt = 0;
  for (let i = 0; i < px.length; i += 4) {
    const r = px[i] / 255;
    const g = px[i + 1] / 255;
    const b = px[i + 2] / 255;
    const [, s, l] = rgbToHsl(r, g, b);
    const w = s * s * (1 - Math.abs(l - 0.5) * 2) + 1e-4;
    wr += r * w;
    wg += g * w;
    wb += b * w;
    wt += w;
  }
  const [h, s, l] = rgbToHsl(wr / wt, wg / wt, wb / wt);
  if (s < 0.12) return "var(--label)"; // pochette grise : accent = couleur du texte (suit le thème)
  const sat = Math.min(1, s * 1.15 + 0.1);
  const lum = Math.min(0.72, Math.max(0.6, l));
  return `hsl(${Math.round(h * 360)} ${Math.round(sat * 100)}% ${Math.round(lum * 100)}%)`;
}

export async function setAccentFrom(url: string | null) {
  if (!url) return;
  let color = cache.get(url);
  if (!color) {
    color = await compute(url).catch(() => DEFAULT);
    cache.set(url, color);
  }
  document.documentElement.style.setProperty("--tint", color);
}
