// Budget : dépenses récurrentes (rappels la veille et le jour même), dépenses courantes,
// salaire (valeur par défaut + salaire réel du mois, demandé pendant la semaine de paie).
// Données locales (store `__app__`, clé `budget`), jamais envoyées ailleurs.
//
// Dates : chaînes locales « AAAA-MM-JJ » (pas d'objet Date stocké, pas de fuseau).

import { computed, watch } from "vue";
import { persisted } from "../../notch/persist";
import { notify } from "../notifications/store";
import { presentation } from "../../notch/presentation";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

// --- Modèle ----------------------------------------------------------------------------

/** Identifiant de catégorie : une catégorie d'office (`home`, `food`…) ou une catégorie
 *  créée par l'utilisateur (`c-<uuid>`, cf. `budget.categories`). */
export type Category = string;

/** Catégories proposées d'office. */
export const BUILTIN_CATEGORIES = ["home", "food", "transport", "subscriptions", "bills", "insurance", "health", "leisure", "shopping", "other"] as const;

const BUILTIN_COLOR: Record<string, string> = {
  home: "var(--sys-link)",
  food: "var(--sys-green)",
  transport: "var(--sys-orange)",
  subscriptions: "#bf5af2",
  bills: "var(--sys-yellow)",
  insurance: "#5e5ce6",
  health: "var(--sys-red)",
  leisure: "#ff375f",
  shopping: "#64d2ff",
  other: "var(--label-3)",
};

/** Couleurs proposées pour une catégorie créée. */
export const CATEGORY_PALETTE = ["#ff9f0a", "#30d158", "#64d2ff", "#5e5ce6", "#bf5af2", "#ff375f", "#ffd60a", "#ac8e68"];

export interface CustomCategory {
  id: string;
  name: string;
  color: string;
}

export type Schedule =
  | { kind: "monthly"; day: number }
  | { kind: "yearly"; month: number; day: number }
  | { kind: "weekly"; weekday: number }; // 0 = lundi

export interface Recurring {
  id: string;
  name: string;
  icon: string;
  amount: number;
  category: Category;
  schedule: Schedule;
  /** Première date prise en compte : le 1er du mois de création (les échéances déjà
   *  passées ce mois-là comptent aussi). */
  since: string;
  active: boolean;
  note?: string;
}

export interface Expense {
  id: string;
  name: string;
  icon: string;
  amount: number;
  category: Category;
  date: string;
  note?: string;
}

interface State {
  /** Salaire par défaut et jour de paie (1–31). */
  salary: number | null;
  payday: number;
  /** Rappels des dépenses récurrentes. */
  reminders: boolean;
  /** Salaire réel d'un mois (« AAAA-MM »), sinon le salaire par défaut. */
  salaries: Record<string, number>;
  /** Encart « salaire du mois » fermé pour ce mois. */
  dismissed: Record<string, true>;
  recurring: Recurring[];
  expenses: Expense[];
  /** Rappels déjà envoyés : `<id>|<date d'échéance>|<veille|jour>`. */
  sent: string[];
  /** Mois écoulés figés (« AAAA-MM ») : salaire retenu et échéances récurrentes telles
   *  qu'elles étaient. Modifier ou supprimer une récurrente ne touche plus le passé. Les
   *  dépenses ponctuelles restent lues depuis `expenses` (déjà datées et fiables). */
  months: Record<string, MonthSnapshot>;
  /** Catégories créées par l'utilisateur. */
  categories: CustomCategory[];
}

export interface MonthSnapshot {
  income: number;
  recurring: Line[];
}

export const { state: budget, ready } = persisted<State>("budget", {
  salary: null,
  payday: 1,
  reminders: true,
  salaries: {},
  dismissed: {},
  recurring: [],
  expenses: [],
  sent: [],
  months: {},
  categories: [],
});

// --- Catégories ---------------------------------------------------------------------------

/** Toutes les catégories : d'office (« Autre » en dernier), puis celles de l'utilisateur. */
export const categories = computed<Category[]>(() => [
  ...BUILTIN_CATEGORIES.filter((c) => c !== "other"),
  ...budget.categories.map((c) => c.id),
  "other",
]);

const custom = (id: string) => budget.categories.find((c) => c.id === id);

export const categoryColor = (id: Category) => BUILTIN_COLOR[id] ?? custom(id)?.color ?? BUILTIN_COLOR.other;
export const categoryLabel = (id: Category) => (id in BUILTIN_COLOR ? t(`budget.cat.${id}`) : (custom(id)?.name ?? t("budget.cat.other")));

export function addCategory(name: string, color: string): Category {
  const id = `c-${crypto.randomUUID()}`;
  budget.categories.push({ id, name: name.trim(), color });
  return id;
}

/** Supprime une catégorie créée : ses dépenses (passées comprises) passent dans « Autre ». */
export function removeCategory(id: Category) {
  if (id in BUILTIN_COLOR) return;
  const move = <T extends { category: Category }>(x: T) => x.category === id && (x.category = "other");
  budget.expenses.forEach(move);
  budget.recurring.forEach(move);
  Object.values(budget.months).forEach((mo) => mo.recurring.forEach(move));
  budget.categories = budget.categories.filter((c) => c.id !== id);
}

// --- Dates ------------------------------------------------------------------------------

const pad = (n: number) => String(n).padStart(2, "0");
export const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const today = () => iso(new Date());
export const monthKey = (y: number, m: number) => `${y}-${pad(m + 1)}`; // m : 0–11
export const daysIn = (y: number, m: number) => new Date(y, m + 1, 0).getDate();
export function parse(s: string): Date {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
}
/** Premier jour du mois d'une date. */
export const monthStart = (s: string) => `${s.slice(0, 8)}01`;
export const addDays = (s: string, n: number) => {
  const d = parse(s);
  d.setDate(d.getDate() + n);
  return iso(d);
};

/** Échéances d'une récurrente dans un mois (jour hors du mois → dernier jour). */
export function occurrences(r: Recurring, y: number, m: number): string[] {
  const last = daysIn(y, m);
  let days: number[] = [];
  const s = r.schedule;
  if (s.kind === "monthly") days = [Math.min(s.day, last)];
  else if (s.kind === "yearly") days = s.month === m ? [Math.min(s.day, last)] : [];
  else {
    for (let d = 1; d <= last; d++) if ((new Date(y, m, d).getDay() + 6) % 7 === s.weekday) days.push(d);
  }
  return days.map((d) => `${y}-${pad(m + 1)}-${pad(d)}`).filter((date) => date >= r.since);
}

/** Prochaine échéance (aujourd'hui compris). */
export function nextDue(r: Recurring, from = today()): string | null {
  const d = parse(from);
  for (let i = 0; i < 14; i++) {
    const y = d.getFullYear();
    const m = d.getMonth() + i;
    const date = new Date(y, m, 1);
    const hit = occurrences(r, date.getFullYear(), date.getMonth()).find((x) => x >= from);
    if (hit) return hit;
  }
  return null;
}

// --- Un mois ------------------------------------------------------------------------------

export interface Line {
  key: string;
  kind: "recurring" | "expense";
  id: string;
  name: string;
  icon: string;
  amount: number;
  category: Category;
  date: string;
}

/** Salaire du mois : figé (mois écoulé), sinon réel s'il a été saisi, sinon par défaut. */
export const salaryOf = (ym: string) => budget.months[ym]?.income ?? budget.salaries[ym] ?? budget.salary ?? 0;

/** Échéances récurrentes d'un mois, calculées depuis la liste actuelle. */
function recurringLines(y: number, m: number): Line[] {
  const out: Line[] = [];
  for (const r of budget.recurring) {
    if (!r.active) continue;
    for (const date of occurrences(r, y, m)) out.push({ key: `r|${r.id}|${date}`, kind: "recurring", id: r.id, name: r.name, icon: r.icon, amount: r.amount, category: r.category, date });
  }
  return out;
}

/** Toutes les lignes d'un mois (récurrentes + dépenses), par date. Mois écoulé : les
 *  récurrentes viennent de sa photo figée. */
export function linesOf(y: number, m: number): Line[] {
  const ym = monthKey(y, m);
  const out: Line[] = [...(budget.months[ym]?.recurring ?? recurringLines(y, m))];
  for (const e of budget.expenses) {
    if (e.date.startsWith(ym)) out.push({ key: `e|${e.id}`, kind: "expense", id: e.id, name: e.name, icon: e.icon, amount: e.amount, category: e.category, date: e.date });
  }
  return out.sort((a, b) => a.date.localeCompare(b.date) || b.amount - a.amount);
}

export interface Summary {
  income: number;
  recurring: number;
  expenses: number;
  remaining: number;
  byCategory: { category: Category; amount: number }[];
}

export function summaryOf(y: number, m: number): Summary {
  const lines = linesOf(y, m);
  const recurring = lines.filter((l) => l.kind === "recurring").reduce((s, l) => s + l.amount, 0);
  const expenses = lines.filter((l) => l.kind === "expense").reduce((s, l) => s + l.amount, 0);
  const income = salaryOf(monthKey(y, m));
  const cats = new Map<Category, number>();
  for (const l of lines) cats.set(l.category, (cats.get(l.category) ?? 0) + l.amount);
  return {
    income,
    recurring,
    expenses,
    remaining: income - recurring - expenses,
    byCategory: [...cats].map(([category, amount]) => ({ category, amount })).sort((a, b) => b.amount - a.amount),
  };
}

// --- Saisie -------------------------------------------------------------------------------

export function addExpense(e: Omit<Expense, "id">) {
  budget.expenses.push({ ...e, id: crypto.randomUUID() });
}

export function saveRecurring(r: Omit<Recurring, "id" | "since" | "active"> & { id?: string }) {
  const existing = r.id ? budget.recurring.find((x) => x.id === r.id) : null;
  if (existing) Object.assign(existing, r);
  else budget.recurring.push({ ...r, id: crypto.randomUUID(), since: monthStart(today()), active: true });
}

export const removeExpense = (id: string) => (budget.expenses = budget.expenses.filter((e) => e.id !== id));
export const removeRecurring = (id: string) => (budget.recurring = budget.recurring.filter((r) => r.id !== id));

/** Noms déjà saisis (suggestions), du plus récent au plus ancien, avec leur dernière saisie. */
export const history = computed(() => {
  const seen = new Map<string, Expense>();
  for (const e of [...budget.expenses].reverse()) if (!seen.has(e.name.toLowerCase())) seen.set(e.name.toLowerCase(), e);
  return [...seen.values()].slice(0, 30);
});

// --- Salaire du mois ------------------------------------------------------------------

/** Mois dont le salaire est à demander : du jour de paie à J+6 (la semaine peut déborder
 *  sur le mois suivant), tant qu'il n'est ni saisi ni fermé. */
export const salaryPrompt = computed<string | null>(() => {
  if (budget.salary === null) return null;
  const now = today();
  const d = new Date();
  // Paie de ce mois-ci, puis celle du mois précédent (fenêtre à cheval sur deux mois).
  for (const back of [0, 1]) {
    const ref = new Date(d.getFullYear(), d.getMonth() - back, 1);
    const y = ref.getFullYear();
    const m = ref.getMonth();
    const pay = `${y}-${pad(m + 1)}-${pad(Math.min(budget.payday, daysIn(y, m)))}`;
    if (now < pay || now > addDays(pay, 6)) continue;
    const ym = monthKey(y, m);
    return budget.salaries[ym] !== undefined || budget.dismissed[ym] ? null : ym;
  }
  return null;
});

export function setMonthSalary(ym: string, amount: number) {
  budget.salaries[ym] = amount;
  const frozen = budget.months[ym];
  if (frozen) frozen.income = amount;
}

// --- Mois écoulés ---------------------------------------------------------------------

/** Premier mois qui contient des données (« AAAA-MM »), ou null. */
export function firstMonth(): string | null {
  const dates = [...budget.recurring.map((r) => r.since), ...budget.expenses.map((e) => e.date)];
  return dates.length ? dates.reduce((a, b) => (a < b ? a : b)).slice(0, 7) : null;
}

/** Fige chaque mois écoulé pas encore figé (au démarrage, puis à chaque changement de mois). */
function closeMonths() {
  const first = firstMonth();
  if (!first) return;
  const now = new Date();
  const current = monthKey(now.getFullYear(), now.getMonth());
  let [y, m] = first.split("-").map(Number);
  m -= 1;
  while (monthKey(y, m) < current) {
    const ym = monthKey(y, m);
    if (!budget.months[ym]) budget.months[ym] = { income: salaryOf(ym), recurring: recurringLines(y, m) };
    if (++m === 12) {
      m = 0;
      y++;
    }
  }
}

export function dismissSalary(ym: string) {
  budget.dismissed[ym] = true;
}

// --- Affichage ------------------------------------------------------------------------------

/** Montant formaté (masqué en mode présentation). */
export function money(n: number, opts: { sign?: boolean } = {}): string {
  if (presentation.value) return "••••";
  if (Object.is(n, -0) || Math.abs(n) < 0.005) n = 0; // jamais « -0 € »
  const f = new Intl.NumberFormat(locale.value, { style: "currency", currency: "EUR", maximumFractionDigits: n % 1 ? 2 : 0 });
  return opts.sign && n > 0 ? `+${f.format(n)}` : f.format(n);
}

// --- Rappels ----------------------------------------------------------------------------------

/** Envoie les rappels dus : la veille et le jour de chaque échéance. Rattrape au démarrage
 *  (PC éteint au moment prévu), une seule fois par échéance et par type. */
function checkReminders() {
  if (!budget.reminders) return;
  const now = today();
  const tomorrow = addDays(now, 1);
  const sent = new Set(budget.sent);
  for (const r of budget.recurring) {
    if (!r.active) continue;
    const due = nextDue(r, now);
    if (!due) continue;
    const kind = due === now ? "day" : due === tomorrow ? "eve" : null;
    if (!kind) continue;
    const key = `${r.id}|${due}|${kind}`;
    if (sent.has(key)) continue;
    budget.sent.push(key);
    notify({
      title: t(kind === "day" ? "budget.notify.today" : "budget.notify.tomorrow", { name: r.name }),
      body: money(r.amount),
      app: t("tool.budget"),
    });
  }
  // Ménage : on ne garde que les clés des 60 derniers jours.
  const limit = addDays(now, -60);
  budget.sent = budget.sent.filter((k) => (k.split("|")[1] ?? "") >= limit);
}

export function initBudget() {
  void ready.then(() => {
    // Rattrapage : les récurrentes créées avant ce correctif démarraient le jour même ;
    // elles comptent désormais depuis le 1er de leur mois de création.
    for (const r of budget.recurring) if (r.since !== monthStart(r.since)) r.since = monthStart(r.since);
    closeMonths();
    checkReminders();
    window.setInterval(() => {
      closeMonths();
      checkReminders();
    }, 15 * 60_000);
    // Réglage réactivé : on vérifie tout de suite.
    watch(() => budget.reminders, (on) => on && checkReminders());
  });
}
