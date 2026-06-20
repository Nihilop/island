# Documentation Island — Développer une extension

> Base de la documentation officielle (FR d'abord ; une traduction EN suivra).
> Public visé : **développeurs d'extensions**. Pour les notes d'architecture internes,
> voir le dossier [`docs/`](../) à la racine.

**Island** est une « Dynamic Island » pour Windows : une barre flottante en haut de
l'écran qui morphe selon le contexte. L'hôte est **100 % agnostique** — il ne contient
aucune logique métier. Toute fonctionnalité (média, capture, monitoring, launcher…)
vit dans une **extension** : un mini-projet Vue/TS qui ne consomme que `@island/sdk`.

## Sommaire

1. [Introduction & modèle](01-introduction.md) — l'hôte agnostique, les surfaces, ce que l'hôte lit.
2. [Démarrage rapide](02-demarrage-rapide.md) — générer depuis un template, coder, activer.
3. [Anatomie & contrat de build](03-anatomie-et-build.md) — `manifest.json`, `vite.config.ts`, Tailwind, `index.ts`.
4. [Le SDK `@island/sdk`](04-le-sdk.md) — `defineExtension`, `useIsland`, surfaces, idle, launcher, notify, storage.
5. [Services & permissions](05-services-et-permissions.md) — capture, système, média, réseau, apps : quelle permission pour quoi.
6. [Recettes](06-recettes.md) — saisie clavier, lancer des apps, HTTP cookie-jar, binaire natif, fenêtre flottante.
7. [Distribution](07-distribution.md) — packager un `.island`, installer, associer.

## Parcours conseillé

Premier contact → lis **1** puis **2** (tu auras une extension qui tourne en quelques
minutes). Pour coder pour de bon → **3** et **4**. Dès que tu touches un service natif
(capture, média, réseau…) → **5**. Pour publier → **7**.

## En une phrase

Une extension déclare des **surfaces** (vues montées dans l'île) et des
**contributions** (entrée de launcher, statut idle, notifications), code en Vue + le
SDK, se build en `dist/index.mjs` + `dist/style.css`, et se distribue en `.island`.
