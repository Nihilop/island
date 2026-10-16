// Commandes rapides du launcher (cf. docs/internal/automations-launcher-spec.md §2) : une
// saisie comme « todo acheter du pain demain 18h » devient une action prête à exécuter,
// avec l'aperçu de ce qui sera créé. Analyse pure (testable), en français et en anglais.

export type Quick =
  | { kind: "todo"; text: string; due?: string; time?: string }
  | { kind: "timer"; minutes: number }
  | { kind: "note"; text: string; title: string }
  | { kind: "expense"; amount: number; label: string; category: string }
  | { kind: "capture" };

const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const plus = (d: Date, n: number) => {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
};

/** Accents retirés, minuscules (comparaison des mots-clés). */
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const WEEKDAYS: Record<string, number> = {
  dimanche: 0, lundi: 1, mardi: 2, mercredi: 3, jeudi: 4, vendredi: 5, samedi: 6,
  sunday: 0, monday: 1, tuesday: 2, wednesday: 3, thursday: 4, friday: 5, saturday: 6,
};

/** Échéance et heure en fin de texte (« … demain 18h », « … vendredi à 9:30 », « … 24/12 »). */
export function splitDue(input: string, now = new Date()): { text: string; due?: string; time?: string } {
  let words = input.trim().split(/\s+/);
  let due: string | undefined;
  let time: string | undefined;
  // On consomme les mots reconnus depuis la fin, dans n'importe quel ordre.
  for (let guard = 0; guard < 4 && words.length > 1; guard++) {
    const last = fold(words[words.length - 1]);
    const prev = words.length > 2 ? fold(words[words.length - 2]) : "";
    let m: RegExpMatchArray | null;
    if (!time && (m = last.match(/^(\d{1,2})(?:h|:)(\d{2})?$/) ?? last.match(/^(\d{1,2})h(\d{2})$/))) {
      const h = Number(m[1]);
      const min = Number(m[2] ?? 0);
      if (h > 23 || min > 59) break;
      time = `${pad(h)}:${pad(min)}`;
      words.pop();
      if (["a", "at", "vers"].includes(fold(words[words.length - 1] ?? ""))) words.pop();
      continue;
    }
    if (!due && (last === "aujourd'hui" || last === "aujourdhui" || last === "today" || last === "auj")) {
      due = iso(now);
    } else if (!due && (last === "demain" || last === "tomorrow")) {
      due = prev === "apres" ? iso(plus(now, 2)) : iso(plus(now, 1));
      if (prev === "apres") words.pop();
    } else if (!due && last === "apres-demain") {
      due = iso(plus(now, 2));
    } else if (!due && last in WEEKDAYS) {
      const diff = (WEEKDAYS[last] - now.getDay() + 7) % 7;
      due = iso(plus(now, diff));
    } else if (!due && (m = last.match(/^(\d{1,2})\/(\d{1,2})(?:\/(\d{2,4}))?$/))) {
      const d = Number(m[1]);
      const mo = Number(m[2]);
      if (d < 1 || d > 31 || mo < 1 || mo > 12) break;
      let y = m[3] ? Number(m[3].length === 2 ? `20${m[3]}` : m[3]) : now.getFullYear();
      // Date déjà passée cette année (sans année précisée) : l'an prochain.
      if (!m[3] && new Date(y, mo - 1, d) < new Date(now.getFullYear(), now.getMonth(), now.getDate())) y++;
      due = `${y}-${pad(mo)}-${pad(d)}`;
    } else break;
    words.pop();
    if (["le", "on", "pour"].includes(fold(words[words.length - 1] ?? "")) && words.length > 1) words.pop();
  }
  // Une heure seule : aujourd'hui (comme dans l'outil Tâches).
  if (time && !due) due = iso(now);
  words = words.filter(Boolean);
  return { text: words.join(" "), due, time };
}

/** Catégorie de budget devinée d'après le libellé (catégories intégrées). */
const CATEGORY_WORDS: [string, string[]][] = [
  ["food", ["courses", "resto", "restaurant", "cafe", "boulangerie", "pizza", "burger", "sushi", "dejeuner", "diner", "food", "groceries", "lunch", "dinner", "coffee", "ubereats", "deliveroo", "marche", "supermarche"]],
  ["transport", ["essence", "carburant", "train", "sncf", "uber", "taxi", "bus", "metro", "navigo", "parking", "peage", "fuel", "gas", "velo"]],
  ["subscriptions", ["netflix", "spotify", "abonnement", "disney", "prime", "youtube", "subscription", "deezer", "icloud"]],
  ["bills", ["facture", "edf", "electricite", "internet", "box", "telephone", "forfait", "eau", "gaz", "bill"]],
  ["home", ["loyer", "rent", "ikea", "maison", "meuble", "bricolage", "leroy", "home"]],
  ["health", ["pharmacie", "medecin", "docteur", "dentiste", "sante", "pharmacy", "doctor", "health"]],
  ["leisure", ["cinema", "concert", "jeu", "steam", "sortie", "bar", "livre", "game", "movie", "book", "playstation", "xbox", "nintendo"]],
  ["shopping", ["amazon", "vetements", "chaussures", "zara", "fnac", "clothes", "shoes", "cadeau", "gift"]],
  ["insurance", ["assurance", "mutuelle", "insurance"]],
];

export function guessCategory(label: string): string {
  const words = fold(label).split(/[^a-z0-9]+/);
  for (const [cat, keys] of CATEGORY_WORDS) if (words.some((w) => keys.includes(w))) return cat;
  return "other";
}

/** Commande rapide reconnue, ou null. */
export function parseQuick(q: string, now = new Date()): Quick | null {
  const s = q.trim();
  if (!s) return null;
  let m: RegExpMatchArray | null;

  if ((m = s.match(/^(?:todo|t[âa]che|task|à faire|a faire)\s+(.+)$/i))) {
    const r = splitDue(m[1], now);
    return r.text ? { kind: "todo", text: r.text, due: r.due, time: r.time } : null;
  }
  if ((m = s.match(/^(?:timer|minuteur|chrono|pomodoro)(?:\s+(\d{1,3}))?\s*(?:min(?:utes?)?|m)?$/i))) {
    const minutes = m[1] ? Number(m[1]) : 25;
    return minutes >= 1 && minutes <= 180 ? { kind: "timer", minutes } : null;
  }
  if ((m = s.match(/^(\d{1,3})\s*(?:min|minutes?)$/i))) {
    const minutes = Number(m[1]);
    return minutes >= 1 && minutes <= 180 ? { kind: "timer", minutes } : null;
  }
  if ((m = s.match(/^note\s+(.+)$/i))) {
    const text = m[1].trim();
    // Titre = début du texte (6 mots, 60 caractères), sans ponctuation de fin.
    const title = text.split(/\s+/).slice(0, 6).join(" ").slice(0, 60).replace(/[.,;:!?…]+$/, "");
    return { kind: "note", text, title };
  }
  if ((m = s.match(/^(?:d[ée]pense|expense|spent|spend)\s+(\d+(?:[.,]\d{1,2})?)\s*(?:€|eur|euros?|\$)?\s*(.*)$/i))) {
    const amount = Number(m[1].replace(",", "."));
    if (!(amount > 0)) return null;
    const label = m[2].trim();
    return { kind: "expense", amount, label, category: guessCategory(label) };
  }
  if (/^(?:capture|screenshot|capture d'[ée]cran)$/i.test(s)) return { kind: "capture" };
  return null;
}
