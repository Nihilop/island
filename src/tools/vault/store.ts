// Coffre (pont Bitwarden), côté front : état, écrans, actions. Les secrets restent dans
// Rust ; ici on ne manipule que des métadonnées. Une action qui reçoit « locked » affiche
// le déverrouillage puis est REJOUÉE automatiquement.

import { reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { clearDrafts } from "../../stores/drafts";
import { prompt } from "../../notch/hello";

export type VaultState = "missing" | "loggedOut" | "locked" | "unlocked";

export interface Meta {
  id: string;
  name: string;
  username: string | null;
  uri: string | null;
  hasPassword: boolean;
  hasTotp: boolean;
}

export interface Code {
  id: string;
  code: string;
  remaining: number;
  period: number;
}

export type Screen = "loading" | "install" | "login" | "unlock" | "list" | "passwordForm" | "totpForm";

export const vault = reactive({
  installed: false,
  state: "missing" as VaultState,
  email: null as string | null,
  server: null as string | null,
  /** Connexion en cours qui attend un code (gardée côté Rust). */
  pending: null as "twoFactor" | "newDevice" | null,
  /** Déverrouillage par Windows Hello possible (session scellée côté Rust). */
  hello: false,
  items: [] as Meta[],
  codes: new Map<string, Code>(),
  /** Installation du client : % de téléchargement, puis 100 pendant vérif + extraction. */
  installing: null as number | null,
});

export const screen = ref<Screen>("loading");

/** Action qui occupe toute la vue (voile + indicateur) ; null = rien en cours. */
export type BusyAction = "lock" | "logout" | "sync" | "save" | "delete";
export const busy = ref<BusyAction | null>(null);

/** Exécute `fn` comme action de vue : ignorée si une autre est déjà en cours. */
async function asBusy<T>(name: BusyAction, fn: () => Promise<T>): Promise<T | undefined> {
  if (busy.value) return undefined;
  busy.value = name;
  try {
    return await fn();
  } finally {
    busy.value = null;
  }
}
export { asBusy };

/** Brouillon de connexion : vit dans le module (pas dans l'écran) pour survivre à une
 *  fermeture du notch. Le mot de passe n'y reste que jusqu'à l'envoi (jamais sur disque). */
export const loginDraft = reactive({
  server: "com" as "com" | "eu" | "custom",
  customUrl: "",
  email: "",
  password: "",
  /** Plusieurs méthodes 2FA : choisir avant de relancer. */
  chooseMethod: false,
});
/** Segment de la liste (mémorisé tant que l'app tourne). */
export const segment = ref<"passwords" | "totp">("passwords");
/** Élément en cours d'édition (formulaire mot de passe). */
export const editing = ref<Meta | null>(null);
/** Retour de copie (global : survit à la reconstruction de la liste). */
export const copied = reactive<{ id: string | null; what: string }>({ id: null, what: "" });
/** Toast d'erreur (2,6 s). */
export const toast = ref<string | null>(null);

let pending: (() => Promise<unknown>) | null = null;
let toastTimer: number | undefined;
let copiedTimer: number | undefined;
let codesTimer: number | undefined;

/** Message lisible pour un code d'erreur du coffre (code inconnu → texte brut). */
export function errorText(t: (k: string, p?: Record<string, unknown>) => string, code: string): string {
  if (code.startsWith("bw:")) return t("vault.err.bw", { msg: code.slice(3) });
  const key = `vault.err.${code}`;
  const text = t(key);
  return text === key ? code : text;
}

export function showError(code: string) {
  window.clearTimeout(toastTimer);
  toast.value = code;
  toastTimer = window.setTimeout(() => (toast.value = null), 2600);
}

function errCode(e: unknown): string {
  return typeof e === "string" ? e : String((e as Error)?.message ?? e);
}

/** Exécute une action ; « locked » → écran de déverrouillage puis rejeu. */
export async function guarded<T>(fn: () => Promise<T>): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    const code = errCode(e);
    if (code === "locked") {
      pending = fn;
      vault.state = "locked";
      screen.value = "unlock";
      return undefined;
    }
    showError(code);
    return undefined;
  }
}

export async function refreshStatus() {
  const s = await invoke<{
    installed: boolean;
    state: VaultState;
    email: string | null;
    server: string | null;
    pending: "twoFactor" | "newDevice" | null;
    hello: boolean;
  }>("vault_status").catch(() => null);
  if (!s) return;
  Object.assign(vault, s);
  if (s.state === "unlocked") await loadItems();
  else {
    vault.items = [];
    vault.codes = new Map();
  }
  route();
}

function route() {
  if (screen.value === "passwordForm" || screen.value === "totpForm") {
    if (vault.state === "unlocked") return;
  }
  screen.value = !vault.installed
    ? "install"
    : vault.state === "loggedOut" || vault.pending
      ? "login"
      : vault.state === "locked"
        ? "unlock"
        : "list";
}

async function loadItems() {
  vault.items = await invoke<Meta[]>("vault_list").catch(() => []);
  await loadCodes();
}

/** Codes rechargés au renouvellement de la période la plus courte (+60 ms). */
export async function loadCodes() {
  window.clearTimeout(codesTimer);
  if (vault.state !== "unlocked") return;
  const list = await invoke<Code[]>("vault_codes").catch(() => [] as Code[]);
  vault.codes = new Map(list.map((c) => [c.id, c]));
  const next = Math.min(...list.map((c) => c.remaining), 30);
  codesTimer = window.setTimeout(loadCodes, next * 1000 + 60);
}

export async function install() {
  if (vault.installing !== null) return;
  vault.installing = 0;
  try {
    await invoke("vault_install");
  } catch (e) {
    if (errCode(e) !== "installing") showError(errCode(e));
  }
  // On garde l'état « installation » jusqu'à ce que le statut confirme le client : pas de
  // bouton qui réapparaît entre la fin du téléchargement et l'extraction.
  await refreshStatus();
  vault.installing = null;
}

export type LoginStep = "done" | "twoFactor" | "newDevice" | "method";
export interface LoginResult {
  step: LoginStep;
  /** Le code précédent était faux : Bitwarden en a envoyé un nouveau. */
  retry: boolean;
}

/** Lance la connexion ; renvoie l'étape suivante (code demandé, choix de méthode…). Le mot
 *  de passe n'est effacé du brouillon qu'une fois la connexion aboutie ou relayée à Rust. */
export async function login(method?: number): Promise<LoginResult> {
  const { email, password } = loginDraft;
  try {
    const r = await invoke<LoginResult>("vault_login", { email, password, method });
    loginDraft.chooseMethod = r.step === "method";
    if (r.step !== "method") loginDraft.password = "";
    return r;
  } finally {
    await refreshStatus();
  }
}

export async function submitCode(code: string): Promise<LoginResult> {
  try {
    return await invoke<LoginResult>("vault_login_code", { code });
  } finally {
    await refreshStatus();
  }
}

export async function cancelLogin() {
  await invoke("vault_login_cancel").catch(() => {});
  loginDraft.password = "";
  loginDraft.chooseMethod = false;
  await refreshStatus();
}

export async function unlock(password: string) {
  await invoke("vault_unlock", { password });
  await refreshStatus();
  const replay = pending;
  pending = null;
  if (replay) await guarded(replay);
}

/** Déverrouillage par Windows Hello (invite système), puis rejeu de l'action en attente. */
export async function helloUnlock() {
  try {
    await prompt(() => invoke("vault_hello_unlock"));
  } finally {
    await refreshStatus();
  }
  const replay = pending;
  pending = null;
  if (replay) await guarded(replay);
}

export function lock() {
  return asBusy("lock", async () => {
    pending = null;
    clearDrafts("vault.");
    await invoke("vault_lock").catch(() => {});
    await refreshStatus();
  });
}

export function logout() {
  return asBusy("logout", async () => {
    pending = null;
    clearDrafts("vault.");
    await invoke("vault_logout").catch(() => {});
    await refreshStatus();
  });
}

export const sync = () => asBusy("sync", () => guarded(() => invoke("vault_sync").then(loadItems)));

export function copy(id: string, field: "username" | "password" | "totp") {
  return guarded(async () => {
    await invoke("vault_copy", { id, field });
    window.clearTimeout(copiedTimer);
    copied.id = id;
    copied.what = field;
    copiedTimer = window.setTimeout(() => (copied.id = null), 1400);
  });
}

export const reveal = (id: string) => guarded(() => invoke<string>("vault_reveal", { id }));

export function initVault() {
  listen("vault://changed", () => void refreshStatus());
  listen<number>("vault://install", (e) => {
    if (vault.installing !== null) vault.installing = e.payload;
  });
}
