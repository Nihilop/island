// To-dos persistés (module : un seul état partagé, chargé une fois), avec échéance
// facultative et rappel via le centre de notifications.
//
// Dates : chaînes locales « AAAA-MM-JJ » et « HH:MM » (pas d'objet Date stocké, pas de fuseau).
import { ref } from "vue";
import { persisted } from "../../notch/persist";
import { notify } from "../notifications/store";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

export interface Todo {
  id: string;
  text: string;
  done: boolean;
  starred: boolean;
  /** Échéance « AAAA-MM-JJ ». */
  due?: string;
  /** Heure « HH:MM » (facultative, seulement avec `due`). */
  time?: string;
}

/** Échéance saisie (champ d'ajout, sélecteur). */
export interface Due {
  due: string;
  time?: string;
}

interface State {
  items: Todo[];
  /** Rappels déjà envoyés : `<id>|<date>|<heure>`. */
  sent: string[];
}

export const { state: todos, ready } = persisted<State>("todos", { items: [], sent: [] });

// --- Dates ------------------------------------------------------------------------------

/** Heure du rappel d'une tâche sans heure. */
const DEFAULT_TIME = "09:00";

const pad = (n: number) => String(n).padStart(2, "0");
export const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const today = () => iso(new Date());
export const addDays = (s: string, n: number) => {
  const [y, m, d] = s.split("-").map(Number);
  return iso(new Date(y, m - 1, d + n));
};
const parse = (s: string) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
};
/** Horloge à la minute : rend les libellés (« Aujourd'hui », retard) réactifs au temps. */
const clock = ref(Date.now());

const nowTime = () => {
  const d = new Date();
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
};

/** Lundi prochain (« Semaine prochaine »). */
export function nextWeek(): string {
  const d = new Date();
  const wd = (d.getDay() + 6) % 7; // 0 = lundi
  return addDays(today(), 7 - wd);
}

/** En retard : jour passé, ou aujourd'hui avec une heure passée. */
export function overdue(d: Due): boolean {
  void clock.value;
  const now = today();
  return d.due < now || (d.due === now && !!d.time && d.time <= nowTime());
}

/** Libellé relatif : « Aujourd'hui 14:00 », « Demain », « Lun. 20 oct. ». */
export function dueLabel(d: Due): string {
  void clock.value;
  const now = today();
  let day: string;
  if (d.due === now) day = t("todos.due.today");
  else if (d.due === addDays(now, 1)) day = t("todos.due.tomorrow");
  else if (d.due === addDays(now, -1)) day = t("todos.due.yesterday");
  else {
    const date = parse(d.due);
    const opts: Intl.DateTimeFormatOptions = { weekday: "short", day: "numeric", month: "short" };
    if (date.getFullYear() !== new Date().getFullYear()) opts.year = "numeric";
    day = new Intl.DateTimeFormat(locale.value, opts).format(date);
    day = day.charAt(0).toUpperCase() + day.slice(1);
  }
  return d.time ? `${day} ${d.time}` : day;
}

// --- Rappels ----------------------------------------------------------------------------

const keyOf = (it: Todo) => `${it.id}|${it.due}|${it.time ?? ""}`;
const fireAt = (it: Todo) => it.time ?? DEFAULT_TIME;

/** Pose (ou retire) l'échéance d'une tâche. Si le moment du rappel est déjà passé, on le
 *  considère comme envoyé : pas de notification immédiate pour une échéance qu'on vient de
 *  saisir (le rattrapage ne concerne que les rappels manqués PC éteint). */
export function setDue(it: Todo, d: Due | null) {
  it.due = d?.due || undefined;
  it.time = (d?.due && d.time) || undefined;
  if (!it.due) return;
  const key = keyOf(it);
  if (it.due < today() || (it.due === today() && fireAt(it) <= nowTime())) {
    if (!todos.sent.includes(key)) todos.sent.push(key);
  }
}

function checkReminders() {
  const now = today();
  const time = nowTime();
  const sent = new Set(todos.sent);
  for (const it of todos.items) {
    // Rattrapage le jour même seulement : une échéance d'hier ne sonne plus.
    if (it.done || it.due !== now || fireAt(it) > time) continue;
    const key = keyOf(it);
    if (sent.has(key)) continue;
    todos.sent.push(key);
    notify({
      title: it.text,
      body: it.time ? t("todos.notify.at", { time: it.time }) : t("todos.notify.today"),
      app: t("tool.todos"),
    });
  }
  // Ménage : on ne garde que les clés des 7 derniers jours (et à venir).
  const limit = addDays(now, -7);
  if (todos.sent.some((k) => (k.split("|")[1] ?? "") < limit)) {
    todos.sent = todos.sent.filter((k) => (k.split("|")[1] ?? "") >= limit);
  }
}

export function initTodos() {
  void ready.then(() => {
    checkReminders();
    window.setInterval(() => {
      clock.value = Date.now();
      checkReminders();
    }, 60_000);
  });
}
