// Rapports du Budget : agrégats sur une période (mois écoulés figés + mois en cours) et
// export CSV de toutes les opérations.

import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { budget, firstMonth, linesOf, monthKey, summaryOf, type Category, type Line, type Summary, categoryLabel, categories } from "./store";
import { t } from "../../notch/i18n";

export type Range = "6m" | "12m" | "year";

export interface MonthStat extends Summary {
  ym: string;
  y: number;
  m: number;
}

/** Mois de la période (du plus ancien au plus récent), à partir du premier mois avec des données. */
export function monthsOf(range: Range): MonthStat[] {
  const now = new Date();
  const first = firstMonth();
  const count = range === "6m" ? 6 : range === "12m" ? 12 : now.getMonth() + 1;
  const out: MonthStat[] = [];
  for (let i = count - 1; i >= 0; i--) {
    const d = new Date(now.getFullYear(), now.getMonth() - i, 1);
    const ym = monthKey(d.getFullYear(), d.getMonth());
    if (first && ym < first) continue;
    out.push({ ym, y: d.getFullYear(), m: d.getMonth(), ...summaryOf(d.getFullYear(), d.getMonth()) });
  }
  return out;
}

export interface Report {
  months: MonthStat[];
  income: number;
  spent: number;
  recurring: number;
  /** Dépense moyenne par mois. */
  average: number;
  /** Part épargnée des revenus (0–1), null sans revenu. */
  savingsRate: number | null;
  /** Part des récurrentes dans les dépenses (0–1). */
  recurringShare: number;
  /** Mois le plus dépensier. */
  peak: MonthStat | null;
  byCategory: { category: Category; amount: number }[];
  top: Line[];
}

export function report(range: Range): Report {
  const months = monthsOf(range);
  const income = months.reduce((s, x) => s + x.income, 0);
  const recurring = months.reduce((s, x) => s + x.recurring, 0);
  const spent = months.reduce((s, x) => s + x.recurring + x.expenses, 0);
  const cats = new Map<Category, number>();
  for (const x of months) for (const c of x.byCategory) cats.set(c.category, (cats.get(c.category) ?? 0) + c.amount);
  const lines = months.flatMap((x) => linesOf(x.y, x.m));
  return {
    months,
    income,
    spent,
    recurring,
    average: months.length ? spent / months.length : 0,
    savingsRate: income > 0 ? (income - spent) / income : null,
    recurringShare: spent > 0 ? recurring / spent : 0,
    peak: months.reduce<MonthStat | null>((p, x) => (!p || x.recurring + x.expenses > p.recurring + p.expenses ? x : p), null),
    byCategory: categories.value.map((category) => ({ category, amount: cats.get(category) ?? 0 }))
      .filter((c) => c.amount > 0)
      .sort((a, b) => b.amount - a.amount),
    top: lines
      .filter((l) => l.kind === "expense")
      .sort((a, b) => b.amount - a.amount)
      .slice(0, 5),
  };
}

// --- Export CSV ------------------------------------------------------------------------------

const csvCell = (v: string | number) => {
  const s = typeof v === "number" ? v.toFixed(2).replace(".", ",") : v;
  return /[;"\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};

/** Toutes les opérations (salaires, récurrentes, dépenses) depuis le premier mois, au
 *  format CSV « ; » + BOM UTF-8 (ouvre correctement dans Excel en français). */
export function toCsv(): string {
  const rows: (string | number)[][] = [[t("budget.csv.date"), t("budget.csv.type"), t("budget.csv.name"), t("budget.csv.category"), t("budget.csv.amount")]];
  const first = firstMonth();
  if (first) {
    const now = new Date();
    let [y, m] = first.split("-").map(Number);
    m -= 1;
    while (monthKey(y, m) <= monthKey(now.getFullYear(), now.getMonth())) {
      const ym = monthKey(y, m);
      const pay = `${ym}-${String(Math.min(budget.payday, new Date(y, m + 1, 0).getDate())).padStart(2, "0")}`;
      rows.push([pay, t("budget.csv.income"), t("budget.salary"), "", summaryOf(y, m).income]);
      for (const l of linesOf(y, m)) {
        rows.push([l.date, t(l.kind === "recurring" ? "budget.csv.recurring" : "budget.csv.expense"), l.name, categoryLabel(l.category), -l.amount]);
      }
      if (++m === 12) {
        m = 0;
        y++;
      }
    }
  }
  return "﻿" + rows.map((r) => r.map(csvCell).join(";")).join("\r\n") + "\r\n";
}

/** Enregistre le CSV où l'utilisateur le choisit. Renvoie false s'il annule. */
export async function exportCsv(): Promise<boolean> {
  const path = await save({ defaultPath: `budget-${new Date().toISOString().slice(0, 10)}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return false;
  await invoke("export_csv", { path, content: toCsv() });
  return true;
}
