# Le SDK

`@island/sdk` (dossier `packages/sdk` du dépôt) est la seule porte d'une extension vers
Island. Il s'utilise de la même façon dans la logique (`main`) et dans les vues.

```ts
import { connect } from "@island/sdk";
const island = await connect();
```

`connect()` attend la poignée de main d'Island puis renvoie l'objet `island`. Toutes les
méthodes renvoient des promesses ; une permission manquante rejette avec `permission:<nom>`.

## Identité, langue, thème

| | |
|---|---|
| `island.id` | Identifiant de l'extension (fourni par Island, pas déclaré par le code) |
| `island.locale` | Langue d'Island au démarrage (`fr`, `en`…) |
| `island.theme` | `"dark"` ou `"light"` ; le SDK pose la classe `light` sur `<html>` |
| `island.onTheme(cb)` | Changement de thème en direct |
| `island.t(clé, params?)` | Texte traduit (fichiers `locales`), `{param}` remplacés |
| `island.log(...args)` | Trace dans la console de développement d'Island |

## Données

| | Permission |
|---|---|
| `island.storage.get(clé)` / `set(clé, valeur)` | aucune — stockage JSON propre à l'extension |
| `island.secrets.get(clé)` / `set(clé, valeur)` / `delete(clé)` | `secrets` — trousseau Windows, propre à l'extension |
| `island.settings.get(clé)` | aucune — valeur d'un réglage déclaré (défaut du manifeste sinon) |
| `island.settings.onChange(cb)` | aucune — appelé avec la clé modifiée |

## Réseau

```ts
const r = await island.http.request({ url: "https://api.github.com/user", method: "GET", headers: { Authorization: `Bearer ${token}` } });
// r = { status, headers: [nom, valeur][], body: string }

const user = await island.http.json<{ login: string }>("https://api.github.com/user", { headers });
// lève HttpError(status, body) si status ≥ 400
```

Permission `network:<domaine>` (`network:*.exemple.com` pour les sous-domaines). **HTTPS
seulement**, aucune redirection suivie, réponse limitée à 5 Mo, adresses locales et privées
refusées. `timeout` (secondes, 1 à 120, 20 par défaut) allonge le délai d'une requête.

```ts
// Corps binaire (vidéo, images, fichiers ; 64 Mo max), transféré sans copie.
const r = await island.http.bytes({ url: "https://exemple.com/video/seg0001.ts", timeout: 90 });
// r = { status, headers, body: ArrayBuffer }
```

**Session par cookie** (permission `cookies`) : Island retient les cookies posés par les
domaines déclarés et les renvoie aux requêtes suivantes, comme un navigateur. Ils restent
invisibles pour l'extension (ni `Set-Cookie` dans les réponses, ni `Cookie` accepté en
entrée), sont gardés dans le trousseau Windows et effacés à la désinstallation.
`island.http.clearCookies()` les oublie tous (déconnexion).

## Fenêtres et événements

```ts
// Ouvre la fenêtre déclarée `player` (ou la ramène au premier plan) avec des arguments.
await island.windows.open("player", { episode: 12 }, { title: "Lecteur" });

// Dans la page de la fenêtre :
const args = await island.windows.args<{ episode: number }>();
island.windows.onArgs((a) => …); // rouverte avec d'autres arguments
await island.windows.setTitle("Épisode 12");

// Entre les pages de l'extension (île, fenêtres, logique) :
island.events.on("progress", (p) => …);
await island.events.emit("progress", { episode: 12, position: 340 });
```

| | Permission |
|---|---|
| `island.windows.open(id, args?, { title? })` | `windows` — fenêtre de [`contributes.windows`](./emplacements#fenetre) |
| `island.windows.args()` / `onArgs(cb)` | aucune — dans la page d'une fenêtre |
| `island.windows.setTitle(titre)` / `close()` | aucune — dans la page d'une fenêtre |
| `island.events.emit(nom, données)` / `on(nom, cb)` | aucune — ne sort jamais de l'extension |

## Capture

Permission `capture`. **Island** affiche la sélection (zone dessinée, fenêtre survolée ou
écran), capture et encode — l'extension pilote seulement. L'île n'apparaît jamais dans les
captures.

```ts
const target = await island.capture.select("region"); // null si annulé (Échap)
if (target) {
  const shot = await island.capture.screenshot(target, { clipboard: true }); // PNG dans Images\Island
  await island.capture.startRecording(target, { systemAudio: true, mic: false, quality: "medium", fps: 30, codec: "h264" });
  const video = await island.capture.stopRecording(); // { path, durationMs } — MP4 dans Vidéos\Island
}
island.capture.onRecording((s) => …); // début / fin (activité en direct « REC »…)
await island.capture.reveal(path);     // afficher une capture dans l'Explorateur
```

Vidéo : encodeur matériel de Windows (H.264 ou H.265), son du PC et micro mélangés. Un seul
enregistrement à la fois ; pendant l'enregistrement, le contour de la zone est visible (pas
dans la vidéo) et le voyant rouge de l'île est allumé.

## Fond d'écran

Permission `wallpaper`. Pose un fond d'écran **statique** Windows, sur un écran ou tous.
L'extension fournit l'image elle-même (un `Blob`, un `ArrayBuffer` ou un `Uint8Array`) :
Island vérifie que c'est une vraie image (30 Mo max), la range dans son propre dossier et
la pose. Aucun chemin de fichier n'est accepté.

```ts
const screens = await island.wallpaper.displays(); // [{ id, x, y, width, height, work, scale, primary }]
const { body } = await island.http.bytes({ url: "https://exemple.com/photo-du-jour.jpg" }); // network:exemple.com
await island.wallpaper.setImage(body, { display: screens[0].id, fit: "fill" });
```

`display` absent : tous les écrans. `fit` (`fill`, `fit`, `stretch`, `center`, `tile`,
`span`) est un réglage global de Windows : il s'applique à tous les écrans. Les fonds
**animés** ne sont pas exposés aux extensions : c'est l'outil « Fonds animés » d'Island.

## Dans l'île

| | Permission |
|---|---|
| `island.live.set(id, spec \| null)` | aucune — [activité en direct](./emplacements#activite-en-direct) |
| `island.launcher.provide(id, recherche)` | aucune — [résultats du launcher](./emplacements#launcher) |
| `island.launcher.onRun(id, action)` | aucune — résultat choisi |
| `island.commands.on(id, action)` | aucune — [commande](./emplacements#commandes) déclenchée |
| `island.notify({ title, body?, banner? })` | `notifications` |
| `island.openUrl(url)` | aucune — http(s) seulement, navigateur par défaut |
| `island.clipboard.readText()` / `writeText(texte)` | `clipboard` |

## Kit UI

```ts
import "@island/sdk/ui.css";
```

Couleurs et typographie de l'île (variables `--label`, `--label-2`, `--fill-1`…, `--green`,
`--orange`, `--red`, `--blue`), fond transparent, et quelques classes : `.list`, `.item`,
`.btn` (`.primary`), `.badge` (`.green`, `.orange`…), `.seg` (onglets), `.empty`, `.caption`,
`.muted`, `.truncate`. Le thème clair est géré automatiquement.
