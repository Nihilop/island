# Island — Architecture & SDK des extensions

> Référence de l'API et du modèle d'extension. Statut : ✅ fait · 🟡 partiel · 🔜 à venir.

## Vision

**Island = des slots + un gros SDK.** L'app fournit un widget (l'île), une modal, et une
API riche (`@island/sdk`). Les extensions **consomment le SDK** pour créer des trucs stylés ;
elles n'accèdent **jamais** aux process Windows/Linux directement — elles **demandent au SDK**,
qui est le **gardien** des capacités natives (SMTC, volume, stockage…).

En front, on expose donc **`island` + `modal` agnostiques** (ils ne savent rien des extensions)
et un **gros SDK** livré AVEC Island (version-locké, pas de npm).

## 1. L'Island (le widget haut-centre)

Plusieurs états, pilotables par l'API : **idle**, **view**, **dnd**.

### Idle ✅ (API en place)
- **Centre** = indicateur d'état. Défaut : **point vert**. Une extension peut le remplacer
  (ex. Spotify → **wave audio** quand un son joue) via `idle.set`.
- **Extrémités gauche/droite** = 2 raccourcis **conditionnels**, visibles seulement si utiles
  (ex. gauche = note de musique quand ça joue → rouvrir le player ; droite = centre de notifs).
- **Clic au centre → launcher** (toujours).
- Garde-fou anti-race : registre clé-par-contributeur (borné), dédoublonnage, coalescing/frame.

### Launcher ✅
Liste les **extensions** détectées + des **raccourcis natifs** (Réglages…). Générique : aucune
extension n'est codée en dur dans l'île.

### View 🔜
Slot **générique** où monte la **vue d'une extension** (ex. le **player Spotify** = vue de
l'extension, consommant l'API média). L'île gère l'animation/morph ; l'extension rend le contenu.

### DND ✅
Repli en sphère noire ; idle au survol ; supprime indicateurs/notifs.

## 2. La Modal ✅ (slot agnostique)

Espace centré (« grand island »), proposé **si besoin** à une extension via `openModal`. Rend
soit un **composant d'extension** (vue), soit un **spec UI déclaratif** (kit). Sert p.ex. de
**réglages** au player Spotify.

## 3. Le SDK (`@island/sdk`)

Livré avec Island. Une extension importe `@island/sdk` ; au runtime tout passe par le pont
`window.__ISLAND__` (marche entre instances Vue).

```ts
export default defineExtension({
  surfaces: { view: Player, config: Settings }, // vues montées par l'hôte
  activate(ctx) { /* logique, abonnements */ },
  deactivate() {},
});
```

### `useIsland()` — surface d'API

| API | Rôle | Statut |
|-----|------|--------|
| `media.state` + `toggle/next/prev/seek/setVolume` | lecture (SMTC, **capacité hôte**) | ✅ |
| `idle.set(key, indicator)` / `clear(key)` | contribue aux indicateurs idle (garde-fou) | ✅ |
| `openModal(req)` / `closeModal()` | espace modal | ✅ |
| `volume()` | ouvre le **tiroir volume en goutte** (primitive hôte, metaball) | 🔜 |
| `storage.get/set(key, value)` | **persiste les settings de l'extension** (par extension) | 🔜 |
| `invoke(cmd, args)` / `on(event, cb)` | accès bas niveau gated | ✅ |
| UI kit : `Button`, `Toggle`, `Slider`… | composants Vue stylés | ✅ |

> Le **média** (`media.rs` / SMTC) est une **API hôte** : Spotify la consomme aujourd'hui, une
> future extension Deezer consommera la même. La **goutte de volume** est une **primitive hôte**
> réutilisable par toute extension (animation metaball intriquée à la fenêtre).

## 4. Modèle d'extension

```
extensions/<id>/            (≈ %APPDATA%/island/extensions en prod)
  manifest.json             # id, name, version, entry, permissions, surfaces
  index.ts                  # defineExtension({ surfaces, activate })
  *.vue                     # vues (n'importent QUE @island/sdk)
```

- **Surfaces** : `view` (montée dans l'île), `config` (montée dans la modal), etc.
- **Permissions** déclarées dans le manifeste (consenties à l'install — P6).
- **Sécurité** : l'extension ne fait que ce que le SDK expose. Zéro accès OS direct.

### Chargement
- **Dev** (`pnpm tauri dev`) : `import.meta.glob('../../extensions/*')` → partage du Vue +
  hot-reload natifs. ✅
- **Prod** (Island installé) : import dynamique du `dist/index.js` autonome de l'extension. 🔜

## 5. Sécurité / capacités natives

Toute capacité native (SMTC, volume, stockage, futur overlay/capture) est **dans le cœur Rust**,
exposée via une **commande Tauri** → relayée par le SDK. L'extension demande, le SDK arbitre
(permissions). C'est le point d'ancrage du futur consentement + signing (P6).
