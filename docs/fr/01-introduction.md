# 1. Introduction & modèle

## L'hôte est agnostique

Island (l'application) ne fait que **présenter** des surfaces et **router** des
événements. Il n'a aucune connaissance d'une extension en particulier : pas d'« id en
dur », pas de logique Spotify/capture/etc. dans le cœur. Tout ce qui apparaît dans
l'île vient d'une extension via le SDK.

Conséquence pratique : **tout ce que tu peux faire, une autre extension peut le faire
aussi.** Les primitives sont génériques.

## Qu'est-ce qu'une extension ?

Un **mini-projet Vue/TypeScript autonome** qui :

- ne dépend, au runtime, que de **`vue`** et **`@island/sdk`** (tous deux fournis par
  l'hôte — voir le contrat de build) ;
- se compile en un **module ESM unique** `dist/index.mjs` (+ `dist/style.css`) ;
- exporte par défaut un objet `defineExtension({...})`.

L'hôte ne lit **que** `manifest.json` + `dist/`. Le code source présent à côté est
**ignoré** (c'est ton espace de travail ; tu le rebuild avec ton propre `pnpm dev`).

## Surfaces : où ton UI s'affiche

Une surface est un composant Vue que l'hôte monte à un endroit de l'interface :

| Surface  | Où | Pour quoi |
| -------- | -- | --------- |
| `view`   | **dans l'île** (elle morphe à la taille demandée) | l'écran principal de l'extension |
| `config` | dans la **modal** centrée | les réglages de l'extension |
| `drop`   | **goutte** sous une view (sous-slot) | un mini-contenu annexe (ex. slider de volume) |
| `window` | **panneau flottant** draggable | un outil libre, déplaçable (lecteur, mini-fenêtre) |

Tu déclares quelles surfaces existent dans le `manifest.json`, et tu branches le
composant Vue réel dans `index.ts`.

## Contributions : ce que tu ajoutes à l'île

Au-delà des surfaces, une extension **contribue** à l'île sans la « posséder » :

- **`launcher`** — une entrée (label + icône) dans le lanceur de l'île.
- **`idle`** — un statut au centre (`playing`, `recording`…) et/ou des raccourcis aux
  extrémités quand l'île est au repos ; un *tap* sur l'île ouvre ton UI.
- **`notify`** — une notification (bannière + historique).

Plusieurs extensions coexistent : leurs contributions sont cumulées et nettoyées
automatiquement à la désactivation.

## Le cycle de vie

```ts
export default defineExtension({
  surfaces: { view: MaView },
  activate(ctx) { /* l'extension démarre : enregistre launcher, idle, watchers… */ },
  deactivate() { /* optionnel : nettoyage manuel */ },
});
```

`activate(ctx)` reçoit le **contexte** : l'API Island complète **+** `ctx.id` (ton
identifiant) **+** `ctx.storage` (ton stockage isolé). L'hôte gère le scope : les
`watch`/`watchEffect` créés dans `activate` sont stoppés à la désactivation, et tes
contributions (launcher, idle, surfaces) sont retirées.

→ Place : [Démarrage rapide](02-demarrage-rapide.md).
