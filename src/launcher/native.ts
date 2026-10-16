// Résultats « natifs » du launcher : commandes rapides (créer une tâche, lancer le minuteur,
// noter une idée, ajouter une dépense…) et recherche dans les outils d'Island (notes,
// tâches, presse-papiers, jeux, dépenses). Chaque ligne porte sa propre action.

import { ref, type Component } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhCamera, PhCheckSquare, PhClipboardText, PhGameController, PhNote, PhNotePencil, PhTimer, PhWallet } from "@phosphor-icons/vue";
import { parseQuick, type Quick } from "./quick";
import { todos, dueLabel } from "../tools/todos/store";
import { setMinutes, start as startTimer, timer } from "../tools/timer/store";
import { notes, nameOf, openNote, createNote, searchNotes, initNotesList } from "../tools/notes/store";
import { addExpense, budget, categoryLabel, money, today as budgetToday } from "../tools/budget/store";
import { categoryIcon } from "../tools/budget/icons";
import { entries as clipEntries, copy as clipCopy } from "../tools/clipboard/store";
import { stats as activityStats } from "../tools/activity/store";
import { installed, emitTo } from "../extensions/runtime";
import { selectTool } from "../notch/state";
import { t } from "../notch/i18n";

export interface NativeItem {
  key: string;
  /** Groupe affiché en sous-titre (Commande, Notes, Tâches…). */
  group: string;
  icon: Component;
  /** Couleur de l'icône (classe Tailwind). */
  tint?: string;
  title: string;
  subtitle: string;
  run: () => unknown | Promise<unknown>;
}

// --- Commandes rapides ------------------------------------------------------------------

function quickItem(q: Quick): NativeItem {
  switch (q.kind) {
    case "todo": {
      const when = q.due ? ` · ${dueLabel({ due: q.due, time: q.time })}` : "";
      return {
        key: "quick",
        group: t("launcher.quick"),
        icon: PhCheckSquare,
        tint: "text-sys-green",
        title: t("launcher.quick.todo", { text: q.text }),
        subtitle: t("launcher.quick.todoHelp") + when,
        run: () => {
          todos.items.unshift({ id: crypto.randomUUID(), text: q.text, done: false, starred: false, due: q.due, time: q.due ? q.time : undefined });
          selectTool("todos");
        },
      };
    }
    case "timer":
      return {
        key: "quick",
        group: t("launcher.quick"),
        icon: PhTimer,
        tint: "text-sys-orange",
        title: t("launcher.quick.timer", { n: q.minutes }),
        subtitle: t("launcher.quick.timerHelp"),
        run: () => {
          if (timer.mode !== "focus") timer.mode = "focus";
          setMinutes(q.minutes);
          startTimer();
        },
      };
    case "note":
      return {
        key: "quick",
        group: t("launcher.quick"),
        icon: PhNotePencil,
        tint: "text-sys-yellow",
        title: t("launcher.quick.note", { title: q.title }),
        subtitle: t("launcher.quick.noteHelp"),
        run: async () => {
          const n = await createNote("", q.title);
          await invoke("notes_write", { id: n.id, content: q.text });
          await openNote(n);
        },
      };
    case "expense":
      return {
        key: "quick",
        group: t("launcher.quick"),
        icon: PhWallet,
        tint: "text-sys-link",
        title: t("launcher.quick.expense", { amount: money(q.amount), label: q.label || categoryLabel(q.category) }),
        subtitle: `${categoryLabel(q.category)} · ${t("launcher.quick.expenseHelp")}`,
        run: () => {
          addExpense({ name: q.label || categoryLabel(q.category), icon: categoryIcon(q.category), amount: q.amount, category: q.category, date: budgetToday() });
          selectTool("budget");
        },
      };
    case "capture": {
      const cap = installed.value.find((x) => x.contributes.commands.some((c) => c.id === "shot") && x.permissions.includes("capture"));
      return {
        key: "quick",
        group: t("launcher.quick"),
        icon: PhCamera,
        title: t("launcher.quick.capture"),
        subtitle: cap ? cap.name : t("launcher.quick.captureMissing"),
        run: () => cap && emitTo(cap.id, "command", { id: "shot" }),
      };
    }
  }
}

export const quickRow = (q: string): NativeItem | null => {
  const p = parseQuick(q);
  return p ? quickItem(p) : null;
};

// --- Recherche dans les outils ----------------------------------------------------------

/** Préfixes de source : `n:` notes, `t:` tâches, `j:` jeux, `b:` budget. */
export const PREFIXES: Record<string, "notes" | "todos" | "games" | "budget"> = { n: "notes", t: "todos", j: "games", b: "budget" };

const lower = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Notes trouvées par le Rust (titre + contenu), mises à jour de façon asynchrone. */
const noteHits = ref<string[]>([]);
let noteSeq = 0;
/** Jeux connus de l'Activité (90 derniers jours), chargés à l'ouverture du launcher. */
const games = ref<{ key: string; name: string; seconds: number }[]>([]);

export async function refreshNative(q: string) {
  const seq = ++noteSeq;
  const term = q.trim();
  if (term.length < 2) return void (noteHits.value = []);
  const ids = await searchNotes(term).catch(() => [] as string[]);
  if (seq === noteSeq) noteHits.value = ids;
}

export async function loadGames() {
  // Liste des notes (titres) : chargée une fois, tenue à jour ensuite.
  initNotesList();
  const to = new Date();
  const from = new Date(Date.now() - 90 * 86_400_000);
  const d = (x: Date) => `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
  const days = await activityStats(d(from), d(to)).catch(() => []);
  const map = new Map<string, { key: string; name: string; seconds: number }>();
  for (const day of days)
    for (const g of day.games) {
      const cur = map.get(g.key);
      map.set(g.key, { key: g.key, name: g.name, seconds: (cur?.seconds ?? 0) + g.seconds });
    }
  games.value = [...map.values()].sort((a, b) => b.seconds - a.seconds);
}

/** Cible de lancement d'un jeu (clé de l'Activité) : Steam par son id, sinon son exe. */
function launchTarget(key: string): string | null {
  const steam = key.match(/^steam:(\d+)$/);
  if (steam) return `steam://rungameid/${steam[1]}`;
  if (/^[a-z]:\\/i.test(key) && key.toLowerCase().endsWith(".exe")) return key;
  return null;
}

/** Résultats natifs d'une requête : par source, `limit` par source. */
export function nativeResults(q: string, only: (typeof PREFIXES)[string] | null): NativeItem[] {
  const term = lower(q.trim());
  if (term.length < 2) return [];
  const limit = only ? 8 : 3;
  const out: NativeItem[] = [];
  const want = (s: (typeof PREFIXES)[string]) => !only || only === s;

  if (want("notes")) {
    const byId = new Map(notes.list.filter((n) => !n.trashed).map((n) => [n.id, n]));
    for (const id of noteHits.value.slice(0, limit)) {
      const n = byId.get(id);
      if (!n) continue;
      out.push({ key: `n:${id}`, group: t("launcher.src.notes"), icon: PhNote, tint: "text-sys-yellow", title: nameOf(n.path), subtitle: n.preview || t("launcher.src.notes"), run: () => openNote(n) });
    }
  }
  if (want("todos")) {
    for (const it of todos.items.filter((i) => !i.done && lower(i.text).includes(term)).slice(0, limit))
      out.push({
        key: `t:${it.id}`,
        group: t("launcher.src.todos"),
        icon: PhCheckSquare,
        tint: "text-sys-green",
        title: it.text,
        subtitle: it.due ? `${t("launcher.src.todos")} · ${dueLabel({ due: it.due, time: it.time })}` : t("launcher.src.todos"),
        run: () => selectTool("todos"),
      });
  }
  if (want("games")) {
    for (const g of games.value.filter((g) => lower(g.name).includes(term)).slice(0, limit)) {
      const target = launchTarget(g.key);
      if (!target) continue;
      out.push({
        key: `j:${g.key}`,
        group: t("launcher.src.games"),
        icon: PhGameController,
        tint: "text-sys-green",
        title: g.name,
        subtitle: t("launcher.src.gamesHelp"),
        run: () => invoke("app_launch", { path: target }),
      });
    }
  }
  if (want("budget")) {
    const seen = new Set<string>();
    for (const e of [...budget.expenses].reverse()) {
      if (out.filter((x) => x.key.startsWith("b:")).length >= limit) break;
      if (!lower(e.name).includes(term) || seen.has(lower(e.name))) continue;
      seen.add(lower(e.name));
      out.push({
        key: `b:${e.id}`,
        group: t("launcher.src.budget"),
        icon: PhWallet,
        tint: "text-sys-link",
        title: e.name,
        subtitle: `${money(e.amount)} · ${new Date(e.date + "T12:00").toLocaleDateString()}`,
        run: () => selectTool("budget"),
      });
    }
  }
  // Presse-papiers : quelques entrées texte (le préfixe `c:` garde sa vue complète).
  if (!only) {
    for (const e of clipEntries.value.filter((e) => (e.kind === "text" || e.kind === "link") && e.preview && lower(e.preview).includes(term)).slice(0, 2))
      out.push({
        key: `c:${e.id}`,
        group: t("launcher.src.clip"),
        icon: PhClipboardText,
        title: e.preview!.replace(/\s+/g, " ").trim(),
        subtitle: t("launcher.src.clip"),
        run: () => clipCopy(e.id),
      });
  }
  return out;
}
