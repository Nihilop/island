# 5. Services & permissions

Certaines capacités passent par le **backend natif** d'Island (capture d'écran, stats
système, média, réseau, lancement d'apps). Chacune est un **service gardé par une
permission** que tu déclares au `manifest.json`.

## Permission = clé d'accès

L'hôte vérifie la permission **à chaque appel** (`ext_has_permission`, en plus du
consentement donné à l'installation — défense en profondeur). Appeler un service sans
sa permission est **refusé** : erreur, ou valeur neutre pour les services sans retour
(volume `-1`, stats à zéro, no-op).

## Lier ton identité : `useIsland(EXT_ID)`

Les services gardés ont besoin de savoir **qui** appelle, pour retrouver ton manifeste.
Le SDK joint ton id automatiquement **si tu le lies** :

```ts
const EXT_ID = "com.island.monextension";
const island = useIsland(EXT_ID);   // dans un composant
await island.system.stats();        // extId joint tout seul → l'hôte vérifie "system"
```

- Dans `activate(ctx)`, **`ctx` est déjà lié** — `ctx.capture`, `ctx.media`… marchent
  directement.
- Dans un composant, passe ton `EXT_ID` à `useIsland(EXT_ID)`.
- `useIsland()` sans id reste valable pour tout ce qui n'est **pas gardé** (`view`,
  `drop`, `window`, `notify`, `launcher`, `idle`, `openExternal`…).

## Catalogue

| Service | Permission | Confiance | OS |
| ------- | ---------- | --------- | -- |
| Capture d'écran | `capture` | standard | Windows ✓ |
| Encodeur natif (enregistrement) | `native-encoder` | ⚠ **élevé** | binaire fourni par l'ext |
| Stats système | `system` | standard | multi ✓ |
| Média (SMTC) | `media` | standard | Windows ✓ |
| Applications (launcher) | `apps` | ⚠ **élevé** | Windows ✓ |
| Presse-papiers | `clipboard` | standard | multi ✓ |
| Conscience des fenêtres | `windows` | ⚠ **élevé** | Windows ✓ |
| Automatisation clavier | `input` | ⚠ **élevé** | Windows ✓ |
| HTTP natif (cookie-jar) | `network` | standard | multi ✓ |
| Stockage par extension | `storage` | standard | multi ✓ |
| Raccourcis clavier globaux | `shortcuts` | standard | multi ✓ |

> **Multi-OS** : la couche commande + permission est cross-platform ; l'implémentation
> native vit dans `services/<svc>/windows.rs` (gated Windows). Porter sur macOS/Linux =
> ajouter un `macos.rs`/`linux.rs` au même contrat, sans toucher au SDK ni aux
> permissions.

## Par service

### `capture` — capture d'écran & enregistrement

```ts
const island = useIsland(EXT_ID);
const screens = await island.capture.listDisplays();
const png = await island.capture.screenshot({ display: 1, region });
const ok  = await island.capture.isRecording();
```

`listDisplays`, `screenshot`, `startRecording`, `stopRecording`, `isRecording`. Les
helpers UI `selectRegion`, `showRegionOutline`, `pickFolder` ne sont **pas** gardés.
C'est un accès écran : réservé aux extensions qui le justifient.

### `native-encoder` — exécuter l'encodeur de l'extension

L'enregistrement **délègue l'encodage à un binaire fourni par l'extension** (ffmpeg
typiquement) ; l'hôte reste agnostique. ⚠ **Confiance élevée** (exécute un natif) →
affichée en évidence à l'install. Garde-fou : le binaire est **cantonné au dossier de
l'extension** (`..`, chemins absolus, lettres de lecteur interdits). Voir la recette
[Embarquer un binaire natif](06-recettes.md#embarquer-un-binaire-natif).

### `system` — statistiques & capteurs

```ts
const sys = useIsland(EXT_ID).system;
const s = await sys.stats();        // { cpu, cores[], memUsed, memTotal }
const bat = await sys.battery();    // { percent, charging } | null
const net = await sys.online();     // boolean
const vol = await sys.volume();     // { level, muted } | null  (volume MAÎTRE de la sortie)
await sys.setVolume(0.3);
await sys.setMuted(true);
const off = sys.onUserIdle(60_000, () => pause(), () => reprise()); // inactivité > 60 s
```

`stats` (sysinfo) est cross-platform ; les capteurs (batterie, réseau, volume maître,
inactivité) sont Windows pour l'instant (`GetSystemPowerStatus`, `InternetGetConnectedState`,
WASAPI, `GetLastInputInfo`). Le **volume maître** est distinct de `media.setVolume` (qui
pilote l'app média). Sur un OS sans impl : valeurs neutres (`null` / `0` / `false`).

### `media` — contrôle du média natif

```ts
const island = useIsland(EXT_ID);
island.media.toggle(); island.media.next(); island.media.setVolume(0.5);
const m = island.media.state; // réactif (titre/artiste/lecture) — LECTURE LIBRE, non gardée
```

Suit la **session média active de l'OS** (aucune app en dur). `media.state` (le flux
d'événements) n'est pas gardé ; seules les **actions** le sont. Windows (SMTC + volume
WASAPI).

### `apps` — lister & lancer des applications

```ts
const apps  = await island.invoke("list_apps", { extId: EXT_ID });
await island.invoke("launch_path", { extId: EXT_ID, path });
const icons = await island.invoke("app_icons", { extId: EXT_ID, paths });
```

⚠ **Confiance élevée** (peut lancer des programmes installés). Windows (raccourcis
`.lnk` du menu Démarrer + ShellExecute).

### `network` — HTTP natif avec cookie-jar

```ts
const r = await useIsland().http.request({ extId: EXT_ID, url, method, body, headers });
```

Consomme une API tierce avec **session par cookie**, hors restrictions CORS/SameSite
d'un `fetch` navigateur (un cookie-jar par extension, **persisté** → session conservée
entre redémarrages). L'extension fournit
l'URL complète → surface SSRF encadrée par la permission. Voir la recette
[Appeler une API avec session](06-recettes.md#appeler-une-api-avec-session-cookie).

### `clipboard` — presse-papiers

```ts
const island = useIsland(EXT_ID);
await island.clipboard.writeText("copié !");
const txt = await island.clipboard.readText();
const png = await island.clipboard.readImage();      // PNG data URL, ou null
await island.clipboard.writeImage(canvas.toDataURL());
```

Texte **et image**. Gardé car lire le presse-papiers expose des données potentiellement
sensibles. Cross-platform (`arboard`).

### `windows` — conscience des fenêtres

```ts
const w = await useIsland(EXT_ID).windows;
const fg = await w.foreground();      // { id, title, app } | null
const all = await w.list();           // fenêtres top-level visibles
await w.focus(fg!.id);                // met une fenêtre au premier plan
const off = w.onForegroundChanged((win) => maj(win));  // au changement d'app active
```

- **Commandes** : `window_foreground`, `window_list`, `window_focus`.
- **Confiance** : ⚠ **élevée** — révèle l'activité (apps et **titres** des fenêtres
  ouvertes = données potentiellement sensibles) et peut activer une fenêtre.
- **OS** : Windows (`GetForegroundWindow`/`EnumWindows`/`SetForegroundWindow`).

### `input` — automatisation clavier

```ts
await useIsland(EXT_ID).input.typeText("texte tapé dans l'app active");
```

- **Commande** : `input_type_text` (SendInput Unicode).
- **Confiance** : ⚠ **élevée** — écrit dans n'importe quelle application au premier plan
  (text-expander, collage automatique…). Affichée en évidence à l'install.
- **OS** : Windows.

### `bus`, `speak` (sans permission backend)

- **`bus`** (`island.bus.emit/on`) : pub/sub **entre extensions** (composition). Choisis
  des canaux préfixés (`"nowplaying:update"`). Abonnements nettoyés à la désactivation.
- **`speak`** (`island.speak(text)`) : synthèse vocale (SAPI). Sortie audio, non gardée
  comme `notify`/`openExternal`.

### `storage`, `secrets` & `shortcuts` (sans permission backend)

Voir [Le SDK](04-le-sdk.md) : `storage` (clé→valeur en clair) et `secrets` (coffre
**chiffré**) sont cloisonnés par id — une extension n'accède qu'à ses propres données,
donc **aucune permission** n'est requise. `shortcuts` (raccourcis globaux) non plus.
Tous cross-platform.

## Récapitulatif « quelle permission pour quel besoin »

- Lire l'écran / enregistrer → **`capture`** (+ **`native-encoder`** si tu embarques un
  encodeur).
- CPU/RAM → **`system`**. · Lecture/volume du média → **`media`**.
- Lister/lancer des apps → **`apps`** (⚠). · API web avec session cookie → **`network`**.
- Persister des réglages → **`storage`**. · Raccourci global → **`shortcuts`**.

Les contributions purement UI (`launcher`, `idle`, `view`, `drop`, `window`, `notify`)
ne demandent **aucune** permission backend.
