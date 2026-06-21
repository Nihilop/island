# 4. Le SDK `@island/sdk`

> Source de vérité du typage : [`src/sdk/island.ts`](../../src/sdk/island.ts) dans le repo Island.

## Deux entrées

- **`defineExtension(def)`** — déclare l'extension (`surfaces`, `activate`, `deactivate`).
- **`useIsland(extId?)`** — dans un composant `.vue`, renvoie l'`IslandApi`. Pour les
  **services gardés** par une permission (`capture`, `system`, `media`, `network`),
  passe ton id : **`useIsland(EXT_ID)`** → l'hôte vérifie ta permission (voir
  [Services & permissions](05-services-et-permissions.md)). Dans `activate(ctx)`, `ctx`
  est **déjà lié** à ton id.

```ts
// dans un composant
const EXT_ID = "com.island.monextension";
const island = useIsland(EXT_ID);
```

## Le contexte `activate(ctx)`

`ctx` = l'API complète **+** `ctx.id` **+** `ctx.storage`. Domaines clés :

| Domaine        | Usage |
| -------------- | ----- |
| `ctx.launcher` | `register({label, icon, onActivate})` / `remove()` — entrée dans le lanceur ; `provider({onQuery})` / `removeProvider()` — alimente la **recherche** du lanceur (palette extensible) |
| `ctx.view`     | `open(component, opts)` / `close()` / `resize(size)` — monte/redimensionne une view dans l'île |
| `ctx.drop`     | `open(component)` / `close()` — goutte (sous-slot d'une view) |
| `ctx.window`   | `open(component, opts) → id` / `close(id?)` / `focus(id)` — panneau flottant draggable |
| `ctx.idle`     | `state(s)`, `action("left"\|"right", a)`, `tap(handler)` — contribue à l'île au repos |
| `ctx.notify`   | `notify(spec) → id` — bannière + historique ; `notifications.dismiss/clear` |
| `ctx.media`    | `state` (réactif), `toggle/next/prev/seek/setVolume` — média natif *(perm `media`)* |
| `ctx.capture`  | screenshot, enregistrement, sélection de zone… *(perm `capture`)* |
| `ctx.system`   | `stats()`, `battery()`, `online()`, `volume()/setVolume/setMuted`, `idleMs()/onUserIdle()` *(perm `system`)* |
| `ctx.windows`  | `foreground()`, `list()`, `focus(id)`, `onForegroundChanged(cb)` — fenêtres du bureau *(⚠ perm `windows`)* |
| `ctx.shortcuts`| `register(accel, handler)` / `unregister(accel)` — raccourcis **globaux** |
| `ctx.storage`  | `get/set/delete/keys` — store clé→valeur persistant, isolé par extension |
| `ctx.secrets`  | `get/set/delete` — coffre **chiffré** (tokens d'API…), isolé par extension |
| `ctx.clipboard`| `readText/writeText/readImage/writeImage` — presse-papiers *(perm `clipboard`)* |
| `ctx.theme`    | `current()` / `onChange(cb)` — thème courant (dark/light) |
| `ctx.bus`      | `emit(channel, payload)` / `on(channel, cb)` — pub/sub ENTRE extensions |
| `ctx.speak`    | `speak(text)` — synthèse vocale (lit à voix haute) |
| `ctx.input`    | `input.typeText(text)` — frappe clavier dans l'app active *(⚠ perm `input`)* |
| `ctx.http`     | `request({extId, url, …})` — HTTP natif cookie-jar *(perm `network`)* |
| `ctx.openExternal` | `(url)` — ouvre une URL http(s) dans le navigateur |
| `ctx.invoke` / `ctx.on` | accès brut aux commandes / events de l'hôte (échappatoire) |

## Les surfaces en détail

### `view` — l'écran principal, dans l'île

```ts
ctx.view.open(MaView, { width: 460, height: 320, radius: 26, persistent: true, safeZone: "absolute" });
ctx.view.resize({ width: 780, height: 560 });  // l'île morphe en douceur, sans remonter la view
ctx.view.close();
```

- **`persistent: true`** : la view reste ouverte malgré un clic ailleurs / une perte de
  focus (ex. garder des stats visibles). Sinon, un clic hors de l'île la replie.
- **`safeZone`** : pilote la zone haute (poignée de collapse + marge au bord d'écran) :
  - `"relative"` *(défaut)* — l'hôte réserve une bande haute (~14px) sous la poignée ; ton
    contenu démarre dessous. Garde un padding normal et laisse l'hôte gérer le haut.
  - `"absolute"` — ton contenu va jusqu'au bord haut, la poignée **flotte par-dessus** (+ un
    léger scrim pour rester lisible). Idéal pour une **bannière image**.
  - `"hidden"` — aucune poignée ni réserve (usage interne notifs).

  > Ancien alias : `safeArea: true/false` reste accepté (`true → relative`, `false → absolute`).

### `drop` — goutte sous une view

Un sous-slot pour un mini-contenu annexe (ex. un slider de volume sous un lecteur).
`ctx.drop.open(component)` / `ctx.drop.close()`.

### `window` — panneau flottant draggable

```ts
const id = ctx.window.open(MonOutil, { title: "Lecteur", width: 480, height: 270, resizable: true });
ctx.window.focus(id);
ctx.window.close(id);
```

Un panneau libre (barre minimale + croix), déplaçable, indépendant de l'île — idéal
pour un lecteur ou un mini-outil.

## L'île au repos : `idle`

```ts
ctx.idle.state("playing");                                   // statut au centre (null = retire)
ctx.idle.action("right", { text: "00:12", color: "#ff453a" }); // raccourci à droite (icône OU texte)
ctx.idle.tap(() => ctx.view.open(MaView));                    // clic sur toute l'île → ouvre ton UI
```

- `state` : un enum géré par l'hôte (`idle | playing | busy | recording`), rendu
  joliment. `null` retire ta contribution.
- `action(slot, …)` : `slot` = `"left"` ou `"right"`. Une action **sans `onActivate`**
  est un affichage (ex. compteur) qui laisse passer le clic-île.
- `tap(handler)` : intercepte le clic sur l'île au repos (au lieu d'ouvrir le launcher).
  `null` retire.

## Notifications

```ts
const id = ctx.notify({
  title: "Capture enregistrée",
  body: "1280×720 · 4 s",
  icon: "<svg …>",
  color: "#30d158",
  source: "Capture",
  timeout: 4500,           // ms d'affichage ; 0 = historique seul
  onClick: () => { /* … */ },
});
```

Modèle en pile : les notifications récentes s'empilent (5 visibles), puis se rétractent.
Une cloche apparaît dans l'île tant qu'il reste des non-lues. `ctx.notifications.dismiss(id)`
/ `ctx.notifications.clear()`.

## Stockage

```ts
await ctx.storage.set("clef", valeur);          // valeur JSON quelconque
const v = await ctx.storage.get("clef", défaut);
await ctx.storage.delete("clef");
const keys = await ctx.storage.keys();
```

**Cloisonné par extension** : 1 fichier JSON par id, jamais partagé. Idéal pour les
réglages et l'état persistant.

## Secrets (chiffrés)

Pour les données **sensibles** (token d'API, mot de passe), n'utilise pas `storage`
(JSON en clair) : utilise `ctx.secrets`, qui stocke dans le coffre du système (Windows
Credential Manager / Keychain).

```ts
await ctx.secrets.set("apiToken", token);
const token = await ctx.secrets.get("apiToken"); // string | null
await ctx.secrets.delete("apiToken");
```

Cloisonné par id (comme `storage`) : une extension ne lit que ses propres secrets.

## Thème

Le HTML hérite automatiquement des tokens de l'hôte (voir le contrat Tailwind), mais un
rendu **canvas/SVG** doit connaître le thème côté JS :

```ts
const island = useIsland();          // non gardé : pas besoin d'extId
const t = island.theme.current();    // "dark" | "light"
const off = island.theme.onChange((t) => redessine(t));  // renvoie une fonction de désabonnement
// …à la destruction : off()
```

## Raccourcis globaux

```ts
const ok = await ctx.shortcuts.register("Ctrl+Shift+Space", () => { /* … */ });
// ok = false si la combinaison est déjà prise par une autre extension ou refusée par l'OS
await ctx.shortcuts.unregister("Ctrl+Shift+Space");
```

Rien n'est enregistré tant que tu n'appelles pas `register`. Tout est nettoyé à la
désactivation.

→ Pour les services natifs (capture, média, réseau, apps) et leurs permissions :
[Services & permissions](05-services-et-permissions.md). Pour des exemples concrets :
[Recettes](06-recettes.md).
