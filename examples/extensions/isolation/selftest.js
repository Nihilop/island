// Tentatives d'évasion : chacune DOIT échouer pour que l'isolation soit validée.
const TIMEOUT = Symbol("timeout");

// Une tentative qui ne répond pas en 2 s compte comme bloquée (Tauri ignore en silence
// une requête IPC d'origine opaque : la promesse ne se termine jamais).
async function expectFail(fn) {
  try {
    const r = await Promise.race([Promise.resolve().then(fn), new Promise((res) => setTimeout(() => res(TIMEOUT), 2000))]);
    if (r === TIMEOUT) return { pass: true, detail: "aucune réponse (ignorée)" };
    return { pass: false, detail: `a réussi : ${String(r).slice(0, 80)}` };
  } catch (e) {
    return { pass: true, detail: String(e?.message ?? e).slice(0, 80) };
  }
}

export async function selftest(api) {
  const r = [];
  const add = (name, res) => r.push({ name, ...res });

  add("Origine opaque", self.origin === "null" ? { pass: true, detail: "origin = null" } : { pass: false, detail: self.origin });
  add("DOM de l'app inaccessible", await expectFail(() => parent.document.title));
  // Tauri injecte son script dans tous les cadres et WebView2 fournit chrome.webview aux
  // iframes : ces objets sont INERTES ici (réseau bloqué par la CSP, origine null refusée,
  // messages de cadre jamais écoutés). Ce qui compte : les tentatives d'appel ci-dessous.
  const globals = ["__TAURI__", "__TAURI_INTERNALS__", "__TAURI_IPC__", "ipc"].filter((g) => g in self);
  add("Objets Tauri (inertes)", { pass: true, detail: globals.length ? `présents : ${globals.join(", ")}` : "absents" });
  add("Pas d'objet Island", "__ISLAND__" in self ? { pass: false, detail: "__ISLAND__ présent" } : { pass: true, detail: "absent" });
  add("Pont WebView2 (inerte)", { pass: true, detail: self.chrome?.webview ? "présent, messages non écoutés" : "absent" });
  add("IPC Tauri par requête", await expectFail(() => fetch("http://ipc.localhost/ext2_list", { method: "POST", body: "{}" }).then((x) => x.status)));
  // Vraie tentative d'évasion, AVEC EFFET VISIBLE : si l'appel passe, le terminal de
  // l'hôte affiche « ÉVASION invoke réussie ». Le réseau étant bloqué, Tauri bascule de
  // lui-même sur son canal postMessage (bon format, bonne clé).
  if (self.__TAURI_INTERNALS__?.invoke) {
    add("IPC Tauri par invoke (effet visible)", await expectFail(() =>
      self.__TAURI_INTERNALS__.invoke("ext2_log", { id: "dev.island.isolation", msg: "ÉVASION invoke réussie" }).then(() => "appel accepté"),
    ));
  }
  add("Réseau direct bloqué", await expectFail(() => fetch("https://example.com").then((x) => x.status)));
  add("Pas de localStorage", await expectFail(() => localStorage.length));
  add("Autre extension illisible", await expectFail(() => fetch("/dev.island.autre/manifest.json").then((x) => x.status)));

  const me = await api.call("whoami");
  add("Identité donnée par l'hôte", me === "dev.island.isolation" ? { pass: true, detail: me } : { pass: false, detail: String(me) });
  await api.call("storage.set", "probe", { at: Date.now() });
  const back = await api.call("storage.get", "probe");
  add("Stockage propre (aller-retour)", back?.at ? { pass: true, detail: "ok" } : { pass: false, detail: JSON.stringify(back) });
  add("Méthode inconnue refusée", await expectFail(() => api.call("vault.reveal", "x")));
  return r;
}
