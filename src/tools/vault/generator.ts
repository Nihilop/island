// Générateur de mots de passe : aléa de l'OS avec échantillonnage par rejet (distribution
// uniforme), jeux sans caractères ambigus, au moins un caractère de chaque jeu choisi,
// mélange Fisher-Yates. Force = longueur × log2(taille du pool).

const LOWER = "abcdefghijkmnopqrstuvwxyz"; // sans l
const UPPER = "ABCDEFGHJKLMNPQRSTUVWXYZ"; // sans I, O
const DIGITS = "23456789"; // sans 0, 1
const SYMBOLS = "!#$%&*+-=?@^_~";

export interface GenOptions {
  length: number; // 8–64
  digits: boolean;
  symbols: boolean;
}

export const DEFAULT_GEN: GenOptions = { length: 20, digits: true, symbols: true };

function randomIndex(n: number): number {
  const limit = Math.floor(0x1_0000_0000 / n) * n;
  const buf = new Uint32Array(1);
  for (;;) {
    crypto.getRandomValues(buf);
    if (buf[0] < limit) return buf[0] % n;
  }
}

export function generate(o: GenOptions): string {
  const sets = [LOWER, UPPER, ...(o.digits ? [DIGITS] : []), ...(o.symbols ? [SYMBOLS] : [])];
  const pool = sets.join("");
  const len = Math.min(64, Math.max(8, o.length));
  const chars = sets.map((s) => s[randomIndex(s.length)]);
  while (chars.length < len) chars.push(pool[randomIndex(pool.length)]);
  for (let i = chars.length - 1; i > 0; i--) {
    const j = randomIndex(i + 1);
    [chars[i], chars[j]] = [chars[j], chars[i]];
  }
  return chars.join("");
}

export type Strength = "strong" | "medium" | "weak";

export function strength(pw: string): { bits: number; level: Strength } {
  let pool = 0;
  if (/[a-z]/.test(pw)) pool += 26;
  if (/[A-Z]/.test(pw)) pool += 26;
  if (/[0-9]/.test(pw)) pool += 10;
  if (/[^a-zA-Z0-9]/.test(pw)) pool += 20;
  const bits = pw.length * Math.log2(Math.max(pool, 1));
  return { bits, level: bits >= 80 ? "strong" : bits >= 55 ? "medium" : "weak" };
}
