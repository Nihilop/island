// Vue « GitHub » dans le notch : deux onglets (revues demandées, mes PR), état de la CI,
// clic = ouvrir dans le navigateur. Affiche tout de suite le dernier état connu, puis
// rafraîchit.

import "@island/sdk/ui.css";
import "./inbox.css";
import { connect } from "@island/sdk";
import { age, fetchSnapshot, type Failure, type Pr, type Snapshot } from "./github";

const island = await connect();
const app = document.getElementById("app")!;

let tab: "reviews" | "mine" = "reviews";
let snap: Snapshot | null = await island.storage.get<Snapshot>("snapshot");
let failure: Failure | null = null;
let loading = false;

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

function ciBadge(pr: Pr): string {
  if (!pr.ci) return "";
  const cls = { success: "green", failure: "red", pending: "blue" }[pr.ci];
  return `<span class="badge ${cls}">${esc(island.t(`ci.${pr.ci}`))}</span>`;
}

function row(pr: Pr): string {
  return `<div class="item" data-url="${esc(pr.url)}">
    <div class="grow">
      <div class="truncate"><b>${esc(pr.title)}</b></div>
      <div class="caption truncate">${esc(pr.repo)} #${pr.number} · ${esc(age(island, pr.updated))}</div>
    </div>
    ${pr.draft ? `<span class="badge">${esc(island.t("draft"))}</span>` : ""}
    ${ciBadge(pr)}
  </div>`;
}

function render() {
  const list = snap ? snap[tab] : [];
  const message =
    failure === "noToken" ? island.t("noToken") : failure === "badToken" ? island.t("badToken") : failure ? island.t("error", { msg: failure.error }) : null;
  app.innerHTML = `
    <header>
      <div class="seg">
        <button data-tab="reviews" class="${tab === "reviews" ? "on" : ""}">${esc(island.t("tab.reviews"))}${snap?.reviews.length ? ` · ${snap.reviews.length}` : ""}</button>
        <button data-tab="mine" class="${tab === "mine" ? "on" : ""}">${esc(island.t("tab.mine"))}${snap?.mine.length ? ` · ${snap.mine.length}` : ""}</button>
      </div>
      <span class="caption">${snap ? esc(island.t("updated", { age: age(island, snap.at) })) : ""}</span>
      <button class="btn" data-refresh ${loading ? "disabled" : ""}>${esc(loading ? island.t("loading") : island.t("refresh"))}</button>
    </header>
    <main>
      ${
        message && !list.length
          ? `<div class="empty">${esc(message)}</div>`
          : list.length
            ? `<div class="list">${list.map(row).join("")}</div>`
            : `<div class="empty">${esc(loading && !snap ? island.t("loading") : island.t(`empty.${tab}`))}</div>`
      }
    </main>`;
}

app.addEventListener("click", (e) => {
  const el = e.target as HTMLElement;
  const t = el.closest<HTMLElement>("[data-tab]");
  if (t) {
    tab = t.dataset.tab as typeof tab;
    return render();
  }
  if (el.closest("[data-refresh]")) return void refresh();
  const item = el.closest<HTMLElement>("[data-url]");
  if (item) void island.openUrl(item.dataset.url!);
});

async function refresh() {
  if (loading) return;
  loading = true;
  render();
  const res = await fetchSnapshot(island);
  loading = false;
  if (typeof res === "object" && "reviews" in res) {
    snap = res;
    failure = null;
  } else failure = res;
  render();
}

island.settings.onChange(() => void refresh());
render();
void refresh();
