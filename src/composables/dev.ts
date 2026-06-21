// Harness de DÉVELOPPEMENT (gated `import.meta.env.DEV`) — zéro impact en prod.
// But : coder les surfaces *core* (notifs, modal, window…) sans se battre avec le
// click-outside, et les retrouver telles quelles après chaque rechargement HMR.
//  - PIN : fige l'île ouverte (autoDismiss devient no-op).
//  - FORCE : ré-ouvre la dernière surface forcée au montage (après HMR).
//  - SEED : injecte de fausses notifs pour itérer sur leur UI.
// L'état (pin + surface forcée) est persisté en localStorage → il SURVIT au HMR : on
// épingle/ouvre une fois, et chaque save remet la surface en place.
import { ref, h } from "vue";
import { post, clearUnread } from "./notifications";
import {
  openModal, closeModal, modalSpec,
  openWindow, closeWindow, floatWindows,
  type ModalSpec,
} from "./overlay";

export const DEV = import.meta.env.DEV;

const LS_PIN = "island.dev.pin";
const LS_FORCE = "island.dev.force";

// Empêche l'île de se refermer (clic dehors / perte de focus). Persisté.
export const devPin = ref(DEV && localStorage.getItem(LS_PIN) === "1");
// Surface à ré-ouvrir au montage (après HMR) : "" | "notifcenter" | "modal" | "window".
export const devForce = ref<string>(DEV ? localStorage.getItem(LS_FORCE) || "" : "");

function persist() {
  localStorage.setItem(LS_PIN, devPin.value ? "1" : "0");
  localStorage.setItem(LS_FORCE, devForce.value);
}

export function devSetPin(v: boolean) { devPin.value = v; persist(); }
export function devSetForce(f: string) { devForce.value = f; persist(); }

// --- Contenu de démo (surfaces de design) ---------------------------------------
const ICON = "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M12 2a6 6 0 0 0-6 6v3.4l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.6L18 11.4V8a6 6 0 0 0-6-6z'/></svg>";
export function devSeedNotifs() {
  clearUnread();
  post({ title: "Spotify", body: "Bohemian Rhapsody — Queen", icon: ICON, color: "#1db954", source: "now", timeout: 0 });
  post({ title: "Capture", body: "Enregistrement de zone terminé", icon: ICON, color: "#ff453a", source: "2 min", timeout: 0 });
  post({ title: "Aniplex", body: "Nouvel épisode disponible", icon: ICON, color: "#6366f1", source: "1 h", timeout: 0 });
}

// Modal de démo : exerce le kit UI déclaratif (UiNode) → représentatif du chrome réel.
const DEMO_MODAL: ModalSpec = {
  title: "Modal de démo",
  subtitle: "Surface de design (Ctrl+Alt+M)",
  ui: [
    { type: "text", label: "Aperçu des composants du kit déclaratif." },
    { type: "toggle", id: "t1", label: "Activer l'option", value: true },
    { type: "segmented", id: "s1", label: "Mode", options: ["Auto", "Clair", "Sombre"], value: "Auto" },
    { type: "slider", id: "v1", label: "Volume", min: 0, max: 100, value: 60 },
    { type: "input", id: "i1", label: "Nom", placeholder: "Saisis quelque chose…" },
    { type: "progress", value: 45 },
    { type: "row", children: [
      { type: "button", id: "cancel", label: "Annuler", variant: "ghost" },
      { type: "button", id: "ok", label: "Valider", variant: "primary" },
    ] },
  ],
};

// Fenêtre flottante de démo (composant inline via render() → pas de fichier .vue dédié).
const DemoWindow = {
  name: "DevWindow",
  render() {
    return h("div", { style: "padding:20px;color:#fff;font-size:13px;display:flex;flex-direction:column;gap:12px;height:100%;box-sizing:border-box" }, [
      h("div", { style: "font-size:15px;font-weight:600" }, "Fenêtre de démo"),
      h("div", { style: "color:rgba(255,255,255,.6);line-height:1.5" }, "Panneau flottant draggable (Ctrl+Alt+W). Déplace-le par la barre, ferme via la croix."),
      h("div", { style: "margin-top:auto;height:90px;border-radius:12px;background:rgba(255,255,255,.06);display:grid;place-items:center;color:rgba(255,255,255,.4)" }, "zone de contenu"),
    ]);
  },
};
const DEMO_WIN_ID = "dev:window";

function openDemoModal() { openModal(DEMO_MODAL); devSetForce("modal"); }
function openDemoWindow() {
  openWindow(DemoWindow, { id: DEMO_WIN_ID, title: "Démo", width: 420, height: 300, resizable: true });
  devSetForce("window");
}
function resetAll(setFormat: (f: string) => void) {
  devSetForce(""); devSetPin(false);
  closeModal(); closeWindow(); clearUnread();
  setFormat("idle");
}

// Ré-ouvre la surface forcée au montage (après HMR) — re-sème les notifs au besoin.
export function devRestore(setFormat: (f: string) => void) {
  if (!DEV || !devForce.value) return;
  if (devForce.value === "modal") openDemoModal();
  else if (devForce.value === "window") openDemoWindow();
  else if (devForce.value === "notifcenter") { devSeedNotifs(); setFormat("notifcenter"); }
  else setFormat(devForce.value);
}

// Raccourcis globaux (Ctrl+Alt+…). `setFormat` est fourni par Island.vue.
export function installDevShortcuts(setFormat: (f: string) => void) {
  if (!DEV) return () => {};
  const onKey = (e: KeyboardEvent) => {
    if (!(e.ctrlKey && e.altKey)) return;
    const k = e.key.toLowerCase();
    if (k === "p") { e.preventDefault(); devSetPin(!devPin.value); }
    else if (k === "n") { e.preventDefault(); devSetPin(true); devSeedNotifs(); devSetForce("notifcenter"); setFormat("notifcenter"); }
    else if (k === "m") { e.preventDefault(); modalSpec.value ? (closeModal(), devSetForce("")) : openDemoModal(); }
    else if (k === "w") { e.preventDefault(); floatWindows.value.length ? (closeWindow(), devSetForce("")) : openDemoWindow(); }
    else if (k === "0") { e.preventDefault(); resetAll(setFormat); }
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
