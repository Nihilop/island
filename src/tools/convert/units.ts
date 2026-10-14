// Conversion d'unités : chaque catégorie a une unité de base ; température à part
// (formules dédiées via °C).

export type Category = "length" | "mass" | "temperature" | "volume" | "speed";

interface Unit {
  id: string;
  /** Facteur vers l'unité de base (absent pour la température). */
  f?: number;
}

export const UNITS: Record<Category, { units: Unit[]; pair: [string, string] }> = {
  length: {
    units: [
      { id: "mm", f: 0.001 },
      { id: "cm", f: 0.01 },
      { id: "m", f: 1 },
      { id: "km", f: 1000 },
      { id: "in", f: 0.0254 },
      { id: "ft", f: 0.3048 },
      { id: "yd", f: 0.9144 },
      { id: "mi", f: 1609.344 },
    ],
    pair: ["cm", "in"],
  },
  mass: {
    units: [
      { id: "mg", f: 1e-6 },
      { id: "g", f: 0.001 },
      { id: "kg", f: 1 },
      { id: "t", f: 1000 },
      { id: "oz", f: 0.028349523125 },
      { id: "lb", f: 0.45359237 },
    ],
    pair: ["kg", "lb"],
  },
  temperature: { units: [{ id: "°C" }, { id: "°F" }, { id: "K" }], pair: ["°C", "°F"] },
  volume: {
    units: [
      { id: "mL", f: 0.001 },
      { id: "L", f: 1 },
      { id: "m³", f: 1000 },
      { id: "tsp", f: 0.00492892 },
      { id: "cup", f: 0.2365882365 },
      { id: "fl oz", f: 0.0295735295625 },
      { id: "gal", f: 3.785411784 },
    ],
    pair: ["L", "gal"],
  },
  speed: {
    units: [
      { id: "m/s", f: 1 },
      { id: "km/h", f: 1 / 3.6 },
      { id: "mph", f: 0.44704 },
      { id: "kn", f: 0.514444 },
    ],
    pair: ["km/h", "mph"],
  },
};

function toCelsius(v: number, u: string): number {
  return u === "°F" ? ((v - 32) * 5) / 9 : u === "K" ? v - 273.15 : v;
}
function fromCelsius(c: number, u: string): number {
  return u === "°F" ? (c * 9) / 5 + 32 : u === "K" ? c + 273.15 : c;
}

export function convert(cat: Category, value: number, from: string, to: string): number {
  if (cat === "temperature") return fromCelsius(toCelsius(value, from), to);
  const us = UNITS[cat].units;
  const a = us.find((u) => u.id === from)?.f ?? 1;
  const b = us.find((u) => u.id === to)?.f ?? 1;
  return (value * a) / b;
}

/** Saisie : chiffres, point, virgule (= séparateur décimal), moins. */
export function parse(input: string): number | null {
  const s = input.replace(/\s/g, "").replace(",", ".");
  if (!/^-?\d*\.?\d+$|^-?\d+\.?$/.test(s)) return null;
  const n = Number(s);
  return Number.isFinite(n) ? n : null;
}

/** 2 décimales si |n| ≥ 1000, 4 si ≥ 1, sinon 6 ; « – » si invalide. */
export function format(n: number | null, locale: string): string {
  if (n === null || !Number.isFinite(n)) return "–";
  const a = Math.abs(n);
  const max = a >= 1000 ? 2 : a >= 1 ? 4 : 6;
  return new Intl.NumberFormat(locale, { maximumFractionDigits: max }).format(n);
}
