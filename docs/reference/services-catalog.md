# Services du SDK & permissions

> Catalogue des **services backend** exposés aux extensions via `@island/sdk`.
> Pour chaque service : la **permission** à déclarer, les **méthodes SDK**, les
> **commandes** sous-jacentes, le **niveau de confiance** et le **statut multi-OS**.
> Complète [authoring-extensions.md](authoring-extensions.md).

## Principe : permission = clé d'accès au backend

Chaque service spécifique à l'OS est gardé par une permission **déclarée au
manifeste** (`permissions: [...]`). L'hôte vérifie cette déclaration **à chaque
appel** (`ext_has_permission`, en plus du consentement donné à l'installation —
défense en profondeur). Une extension qui appelle un service sans avoir déclaré sa
permission est **refusée** côté hôte (erreur, ou valeur neutre pour les services
sans retour : volume `-1`, stats à zéro, no-op).

### Comment l'hôte sait QUI appelle : `extId`

Les services gardés ont besoin de l'**identité de l'extension** pour retrouver son
manifeste. Le SDK la joint automatiquement **si tu lies ton id** :

```ts
const EXT_ID = "com.island.monextension";
const island = useIsland(EXT_ID);   // dans un composant
await island.system.stats();        // extId joint tout seul → l'hôte vérifie "system"
```

- Dans `activate(ctx)`, **`ctx` est déjà lié** à ton id : `ctx.capture`, `ctx.media`,
  etc. fonctionnent sans rien passer.
- Dans un **composant** (`.vue`), passe ton `EXT_ID` à `useIsland(EXT_ID)`.
- `useIsland()` sans id reste valable pour tout ce qui n'est **pas gardé**
  (`view`, `drop`, `window`, `notify`, `launcher`, `idle`, `openExternal`…).

## Catalogue

| Service | Permission | Confiance | OS |
|---|---|---|---|
| Capture d'écran | `capture` | standard | Windows ✓ |
| Encodeur natif (enregistrement) | `native-encoder` | ⚠ **élevé** | multi (binaire fourni par l'ext) |
| Stats système | `system` | standard | multi ✓ (sysinfo) |
| Média (SMTC) | `media` | standard | Windows ✓ |
| Applications (launcher) | `apps` | ⚠ **élevé** | Windows ✓ |
| HTTP natif (cookie-jar) | `network` | standard | multi ✓ (ureq) |
| Presse-papiers (texte + image) | `clipboard` | standard | multi ✓ (arboard) |
| Conscience des fenêtres | `windows` | ⚠ **élevé** | Windows ✓ |
| Stockage par extension | `storage` | standard | multi ✓ |
| Secrets chiffrés par extension | — (cloisonné par id) | standard | multi ✓ (keyring) |
| Thème courant (dark/light) | — (lecture seule) | standard | multi ✓ (front) |
| Raccourcis clavier globaux | `shortcuts` | standard | multi ✓ |

> **Multi-OS** : la couche commande + permission est cross-platform ; l'implémentation
> native vit dans `services/<svc>/windows.rs` (gated `#[cfg(target_os = "windows")]`).
> Porter sur macOS/Linux = ajouter un `macos.rs`/`linux.rs` remplissant le même contrat,
> **sans toucher** aux commandes, aux permissions ni au SDK.

---

### `capture` — Capture d'écran & enregistrement

```ts
const island = useIsland(EXT_ID);
const screens = await island.capture.listDisplays();
const png = await island.capture.screenshot({ display: 1, region });
const ok = await island.capture.isRecording();
```

- **Méthodes** : `listDisplays`, `screenshot`, `startRecording`, `stopRecording`,
  `isRecording`, `selectRegion`, `showRegionOutline`, `pickFolder` (les 3 dernières
  sont des helpers UI de l'hôte, non gardés).
- **Commandes** : `capture_list_displays`, `capture_screenshot`,
  `capture_start_recording`, `capture_stop_recording`, `capture_is_recording`.
- **Confiance** : standard, mais c'est un **accès écran** — réservé aux extensions
  qui le justifient. `stopRecording` n'est pas gardé (toujours sûr d'arrêter).
- **OS** : Windows (Windows Graphics Capture, anti-cheat safe).

### `native-encoder` — Exécuter l'encodeur de l'extension

L'enregistrement **délègue l'encodage à un binaire fourni par l'extension** (ffmpeg
typiquement). L'hôte reste agnostique : il capture les frames et les pipe ; l'ext
fournit le binaire + les arguments d'encodage.

```ts
await island.capture.fetchBinary({ extId: EXT_ID, url, dest: "binaries/ffmpeg.exe", zipEntry });
await island.capture.startRecording({ ...opts, encoder: { extId: EXT_ID, bin, args, audioArgs } });
```

- **Commandes** : `ext_fetch_binary`, `capture_start_recording` (vérifie **aussi**
  `capture`).
- **Confiance** : ⚠ **élevée** — exécute un programme natif. Garde-fou : le binaire
  est **cantonné au dossier de l'extension** (`resolve_in_ext` interdit `..`, chemins
  absolus, lettres de lecteur). Une ext ne peut donc lancer QUE ses propres fichiers.
- **OS** : logique hôte cross-platform ; le binaire est fourni par l'ext par OS.

### `system` — Statistiques & capteurs

```ts
const sys = useIsland(EXT_ID).system;
await sys.stats();    // { cpu, cores[], memUsed, memTotal }
await sys.battery();  // { percent, charging } | null
await sys.online();   // boolean
await sys.volume();   // { level, muted } | null  (volume MAÎTRE, ≠ media)
await sys.setVolume(0.3); await sys.setMuted(true);
sys.onUserIdle(60_000, onIdle, onActive);  // helper d'inactivité (sonde idleMs)
```

- **Commandes** : `system_stats`, `system_battery`, `system_online`, `system_volume`,
  `system_set_volume`, `system_set_muted`, `system_idle_ms`.
- **Confiance** : standard. Stats/batterie/réseau/inactivité en lecture ; le volume
  maître est en lecture **et écriture** (sortie du périphérique par défaut).
- **OS** : `stats` cross-platform (`sysinfo`) ; capteurs Windows (`GetSystemPowerStatus`,
  `InternetGetConnectedState`, `GetLastInputInfo`, WASAPI `IAudioEndpointVolume`).

### `media` — Contrôle du média natif

```ts
const island = useIsland(EXT_ID);
island.media.toggle(); island.media.next(); island.media.setVolume(0.5);
const m = island.media.state; // réactif (titre/artiste/lecture) — LECTURE LIBRE, non gardée
```

- **Méthodes** : `toggle`, `next`, `prev`, `seek`, `setVolume` (+ `media.state`
  réactif, non gardé : juste un flux d'événements).
- **Commandes** : `media_toggle`, `media_next`, `media_prev`, `media_seek`,
  `media_get_volume`, `media_set_volume`.
- **Confiance** : standard. Sans permission : no-op (volume lu = `-1`).
- **OS** : Windows (SMTC + volume WASAPI).

### `apps` — Lancer des applications & chercher des fichiers

```ts
const apps  = await island.invoke("list_apps",    { extId: EXT_ID });               // Win32 + UWP + Steam
await         island.invoke("launch_path",  { extId: EXT_ID, path });               // ShellExecute (open)
await         island.invoke("launch_admin", { extId: EXT_ID, path });               // élévation UAC (runas)
const icons = await island.invoke("app_icons",    { extId: EXT_ID, paths });         // icônes PNG (data-URL)
const files = await island.invoke("search_files", { extId: EXT_ID, query, roots, limit }); // [{ name, path, isDir }]
const hasEverything = await island.invoke("files_engine", { extId: EXT_ID });        // Everything détecté ?
```

- **Commandes** : `list_apps`, `launch_path`, `launch_admin`, `app_icons`, `search_files`, `files_engine`.
- **`list_apps`** : applications **Win32** (menu Démarrer) **+ UWP/Store + jeux Steam** (énumération du dossier shell *AppsFolder* + manifestes Steam).
- **`search_files`** : recherche fichiers/dossiers, **moteur hybride** — *Everything* (voidtools) s'il tourne → tout le disque instantané ; sinon **index maison** des `roots` fournies (défaut : Bureau / Documents / Téléchargements).
- **Confiance** : ⚠ **élevée** — peut lancer des programmes installés (et avec **élévation** via `launch_admin`).
- **OS** : Windows.

### `network` — HTTP natif avec cookie-jar

Consomme une API tierce avec **session par cookie**, hors restrictions
CORS/SameSite d'un `fetch` navigateur (la requête est native, hors webview). Un
cookie-jar **par extension**, **persisté sur disque** (la session survit aux redémarrages).

```ts
const r = await useIsland().http.request({ extId: EXT_ID, url, method, body, headers });
```

- **Commande** : `http_fetch`.
- **Confiance** : standard, mais l'ext fournit l'**URL complète** → surface SSRF.
  La permission + le consentement encadrent la capacité.
- **OS** : cross-platform (`ureq`). `http.request` prend l'`extId` **explicitement**.

### `clipboard` — Presse-papiers (texte + image)

```ts
const island = useIsland(EXT_ID);
await island.clipboard.writeText("copié !");
const txt = await island.clipboard.readText();
const png = await island.clipboard.readImage();   // PNG data URL, ou null
await island.clipboard.writeImage(canvas.toDataURL());
```

- **Commandes** : `clipboard_read_text`, `clipboard_write_text`, `clipboard_read_image`,
  `clipboard_write_image`.
- **Confiance** : standard, gardé `clipboard` (lire le presse-papiers expose des données
  potentiellement sensibles).
- **OS** : cross-platform (`arboard`).

### `storage` — Clé→valeur persistant par extension

```ts
await ctx.storage.set("clef", valeur);
const v = await ctx.storage.get("clef", défaut);
```

- **Commandes** : `storage_get`, `storage_set`, `storage_delete`, `storage_keys`.
- **Confiance** : standard. **Cloisonné par id** : 1 fichier JSON par extension,
  une ext ne lit jamais le store d'une autre.
- **OS** : cross-platform (`std::fs`).

### `secrets` — Stockage chiffré par extension

```ts
await ctx.secrets.set("apiToken", token);
const token = await ctx.secrets.get("apiToken"); // string | null
await ctx.secrets.delete("apiToken");
```

- **Commandes** : `secret_get`, `secret_set`, `secret_delete`.
- **Confiance** : standard. **Cloisonné par id** (pas de permission dédiée) ; valeurs
  CHIFFRÉES dans le coffre du système (vs `storage` en clair). À réserver aux données
  sensibles (tokens, mots de passe).
- **OS** : Windows (Credential Manager via `keyring`) ; Keychain/Secret Service au portage.

### `theme` — Thème courant (dark/light)

```ts
const island = useIsland();          // non gardé
island.theme.current();              // "dark" | "light"
const off = island.theme.onChange((t) => redessine(t));
```

- **Confiance** : standard, lecture seule (pas de permission). Front pur (observe la
  classe de `documentElement`). Utile au rendu canvas/SVG qui ne suit pas les tokens CSS.
- **OS** : cross-platform.

### `shortcuts` — Raccourcis clavier globaux

```ts
const ok = await ctx.shortcuts.register("Ctrl+Shift+Space", () => { /* … */ });
await ctx.shortcuts.unregister("Ctrl+Shift+Space");
```

- **Confiance** : standard. **OS** : cross-platform (plugin global-shortcut).

---

### `terminal` — Terminaux PTY & exec

```ts
// Terminal interactif (à brancher sur xterm.js) :
const id = await ctx.terminal.spawn({ cwd: "C:/dev/projet", cols: 80, rows: 24 });
const off = await ctx.terminal.onData(({ id: i, b64 }) => { if (i === id) term.write(atob(b64)); });
term.onData((d) => ctx.terminal.write(id, d));   // frappes → stdin
ctx.terminal.resize(id, cols, rows);             // au resize / xterm fit
ctx.terminal.kill(id);                           // tue le process
await ctx.terminal.onExit(({ id }) => { /* process terminé */ });

// Commande one-shot CAPTURÉE (git, docker…) :
const { code, stdout, stderr } = await ctx.terminal.exec({ cmd: "git", args: ["-C", path, "status"] });
```

- **Méthodes** : `spawn(opts)→id`, `write(id,data)`, `resize(id,cols,rows)`, `kill(id)`,
  `exec({cmd,args?,cwd?})→{code,stdout,stderr}`, `onData(cb)`, `onExit(cb)`. La sortie
  PTY arrive en **base64** (binaire/ANSI safe) via `onData` ; à décoder pour xterm.
- **Confiance** : ⚠⚠ **MAXIMALE** — exécute des **processus arbitraires** (équivalent
  exécution de code). À n'accorder qu'à des extensions de **confiance** (outils de dev).
  Affichée en évidence à l'install.
- **OS** : cross-platform (crate `portable-pty` → ConPTY sous Windows).

---

## Récapitulatif « quelle permission pour quel besoin »

- Lire l'écran / enregistrer → **`capture`** (+ **`native-encoder`** si tu embarques
  un encodeur pour l'enregistrement).
- CPU/RAM → **`system`**.
- Lecture/pause/volume du média → **`media`**.
- Lister/lancer des apps → **`apps`** (⚠).
- Appeler une API web avec session cookie → **`network`**.
- Persister des réglages → **`storage`**.
- Raccourci clavier global → **`shortcuts`**.
- Lancer des processus / terminal interactif → **`terminal`** (⚠⚠ confiance maximale).

Les contributions purement UI (`launcher`, `idle`, `view`, `drop`, `window`, `notify`)
ne demandent **aucune** permission backend.
