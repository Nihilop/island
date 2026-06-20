# Island — Spec & Roadmap (v2)

> Source de vérité de l'architecture après les décisions du 2026-06-16.
> Remplace le modèle de plugin décrit dans `api.md` (qui sera révisé au fil de l'implémentation).

## Décisions actées

1. **Extension = module Vue/TS** chargé dans le webview (modèle Obsidian / VS Code webview),
   **pas** un isolate deno_core headless. Vrais composants `.vue`, hot-reload, DX maximale.
2. **Fenêtre overlay unique** : un seul webview transparent plein écran. L'île, la modal, le
   launcher et les surfaces d'extension sont des **composants** d'une même app Vue.
3. Confiance = **consentement des permissions à l'install + signing** (pas de sandbox runtime
   de l'UI en v1). Modèle « extensions de confiance », comme Obsidian.

## Ce qu'on garde / retravaille / archive

| Élément | Sort |
|---|---|
| Moteur d'animation île/modal (`exit→morph→enter`, `fit`) | ✅ gardé (à extraire en `useMorph`) |
| Média natif SMTC + volume WASAPI | ✅ gardé (exposé via `ctx.media`) |
| Formats d'île, launcher, DND, goutte volume | ✅ gardés |
| 3 fenêtres séparées (island/settings/modal) | 🔄 fusionnées en **1 overlay** |
| Kit UI déclaratif JSON (`UiNode.vue`) | 🔄 devient une **librairie de composants Vue** dans le SDK |
| Runtime deno_core + ops + `op_modal_open` async | 🗄️ archivé (réservé à un éventuel sandbox P7) |

## Architecture cible

### Fenêtre overlay unique
- 1 fenêtre transparente, plein écran, `always-on-top`, sans déco, `skip-taskbar`, sans ombre.
- Une app Vue : `<Island>` (haut-centre), `<Modal>` (centre + backdrop), `<Launcher>`, +
  un hôte de surfaces d'extension.
- **Click-through** : la fenêtre couvre tout l'écran mais ne capte les clics que sur les
  zones interactives. Mécanisme : hook souris bas-niveau Win32 (`WH_MOUSE_LL`) qui bascule
  `set_ignore_cursor_events(true/false)` selon que le curseur est dans une **région
  interactive** (le front publie les bounding-rects courants au Rust). C'est le gros morceau natif.

### Modèle d'extension
```
spotify/
  manifest.json            # id, name, version, permissions, surfaces, default-enabled:false
  package.json             # pnpm dev (Vite watch) -> dist/index.js  (ESM, vue + sdk externes)
  src/
    index.ts               # export default defineExtension({ surfaces, activate, deactivate })
    Tile.vue               # surface "tile"  -> île
    Config.vue             # surface "config"-> modal
```
- `defineExtension({ surfaces: { tile, config }, activate(ctx), deactivate() })`.
- Forme impérative équivalente dispo : `ctx.register(id, Component)`.
- L'`id` de surface (`spotify.tile`, `spotify.config`) sert de **route** de navigation.

### Chargement (partage du runtime Vue — critique)
- Le bundle d'extension **externalise** `vue` et `@island/sdk` ; l'hôte les fournit au runtime
  (import-map / scope partagé) → le composant monte dans **l'instance Vue de l'hôte**, bundle léger.
- Activation : `const mod = await import(pluginUrl)` (servi via protocole asset Tauri en prod,
  dev-server en dev) → `mod.default` → enregistrement des surfaces dans un registre réactif.

### SDK (`@island/sdk`)
- `defineExtension`, `useIsland()` (ctx réactif : `media`, `storage`, `navigate`, `openModal`,
  `close`, `onCommand`…), types.
- `@island/sdk/ui` : composants Vue stylés (`Button`, `Toggle`, `Slider`, `Segmented`, `Input`,
  `Row`, `List`, `Text`, `Progress`). Pas de lib externe.

### Cycle de vie
1. **Presence detector** : scan de `%APPDATA%/island/plugins/*` (installées) + dossier dev (watch).
2. Manifeste valide → extension **détectée** (inerte).
3. **Statut d'activation possédé par l'app** (settings `enabled: [...]`). Aucune auto-activation ;
   le module n'est importé qu'à l'activation.
4. Activation → import module → `activate(ctx)` (ctx gated par permissions) → enregistre surfaces.
5. Désactivation → `deactivate()` → désenregistre.
6. **L'île ne connaît l'UI d'une extension qu'après activation.**

## Roadmap

- **P0 — Overlay & click-through** : fenêtre overlay unique ; migrer île/modal/launcher dans un
  seul webview ; hook souris Win32 + publication des régions interactives. *(gros, natif)*
- **P1 — Loader d'extensions** : presence detector + manifeste + statut d'activation local +
  import dynamique + registre de surfaces (sans self-activation).
- **P2 — SDK** : `defineExtension`, `useIsland`, partage du runtime Vue (externals + import-map),
  librairie UI Vue, types ; hot-reload dev (watch `dist` + ré-import).
- **P3 — Surfaces & navigation** : monter les composants d'extension dans l'île et la modal via
  route id ; launcher = surfaces des extensions activées.
- **P4 — Capacités (`ctx`)** : `ctx.media` (SMTC réactif), `ctx.storage` (config persistée),
  gating des permissions.
- **P5 — Gestion** : UI Réglages pour activer/désactiver les extensions détectées.
- **P6 — Distribution** : packaging `.island`, modal d'installation (consentement permissions),
  dossier user, signing. *(plus tard)*
- **P7 — Sandbox avancé** (optionnel) : iframe/worker ou deno_core pour extensions non-fiables.

## État des lieux (2026-06-16, après clarif. de spec)

**Conforme :** overlay unique + click-through (P0) · idle = registre d'indicateurs + raccourcis
conditionnels + clic-centre→launcher (avec garde-fou) · launcher générique · modal = slot
agnostique · SDK de base (`useIsland`: media/idle/modal/invoke) + pont `window.__ISLAND__` ·
click-outside = perte de focus · extensions hors de `src/` (`./extensions/*`) · hôte dé-brandé.

**À corriger (le widget doit être 100 % agnostique) :**
- ❌ Pas de slot **`view`** générique → des formats codés en dur dans `Island.vue` :
  `media` (le **player Spotify**) + `call`/`timer`/`transfer`/`dialog` (vestiges démo).
- ❌ Le **player** (transport, progression, goutte volume) est dans l'hôte → doit être une **vue d'extension**.
- 🟡 L'indicateur idle "now playing" est contribué par l'hôte → doit l'être par **l'extension**.

## Plan d'attaque (validé)

1. **Slot `view` générique** : `Island` ne gère plus que `idle | launcher | view | dnd`. `view`
   monte une surface d'extension. **Supprimer** les formats démo (call/timer/transfer/dialog).
2. **Extraire le player Spotify** → surface `view` de l'extension (consomme `useIsland().media`).
   La modal `config` existante = ses réglages (démontre la modal).
3. **Indicateur idle → l'extension** (`island.idle.set` : wave + note, `onActivate` → ouvre sa vue).
4. **SDK `storage`** : `ctx.storage.get/set` pour persister les settings **par extension** (le besoin du player).
5. **Goutte de volume = primitive hôte** exposée au SDK (`island.volume()`), réutilisable par toute extension.
6. **`media.rs` = API hôte** (SMTC) consommée par les extensions média (Spotify, futur Deezer…).
7. Puis **P5** (gestion activer/désactiver dans Réglages) et **P6** (packaging `.island`).

> Philosophie de maintenance : les mises à jour = surtout des **fix** + **enrichissement du SDK
> in-app** pour permettre de meilleures extensions.
