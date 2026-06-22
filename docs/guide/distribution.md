# 7. Distribution

## Le format `.island`

Un `.island` est une **archive zip** contenant `manifest.json` + `dist/` (sans le code
source). C'est tout ce dont l'hôte a besoin pour installer et charger une extension.

## Packager

Deux voies :

- **Dans l'app** (recommandé) : Réglages → Extensions → bouton **« Packager »** (visible
  sur les extensions marquées *dev*, c.-à-d. qui ont un `package.json`) → choisir la
  destination → un `.island` est créé.
  L'app **ne compile pas** : elle zippe le `dist/` que tu as déjà buildé. Build d'abord
  (`pnpm build`).
- **Manuellement** : zippe `manifest.json` + `dist/` à la racine de l'archive, extension
  `.island`.

## Installer

- **Double-clic** sur un `.island` (si l'association de fichiers est active) → une modal
  d'installation s'ouvre : CGU → permissions demandées (traduites, les paliers ⚠ mis en
  évidence) → progression → l'extension est copiée dans le dossier des extensions et
  activée.
- Depuis l'app : la même modal peut être déclenchée via **Réglages → Extensions →
  Parcourir…**.

## Association de fichiers

À la première installation, Island associe les `.island` à l'application (clé `HKCU`,
per-utilisateur, sans admin) → le double-clic fonctionne. Un bouton **« Associer les
.island »** est aussi disponible dans les Réglages.

## Checklist avant de publier

- [ ] `id` du `manifest.json` = nom du dossier (reverse-DNS).
- [ ] `vite.config.ts` externalise `["vue", "@island/sdk"]`.
- [ ] `tailwind.css` : `theme(reference)` + `@source not "./dist"` + `@theme inline`.
- [ ] `permissions` du manifeste = **exactement** les services backend utilisés (ni plus,
      ni moins) — chaque service est vérifié à l'appel.
- [ ] Les composants qui appellent un service gardé utilisent `useIsland(EXT_ID)`.
- [ ] Un binaire natif éventuel est **téléchargé au runtime** (pas dans le `.island`),
      `binaries/` est gitignore.
- [ ] `pnpm build` à jour → `dist/index.mjs` + `dist/style.css` présents.
- [ ] Testé activé/désactivé à chaud (Réglages → Extensions) sans erreur console.

## Et après

La traduction anglaise de cette documentation viendra dans `docs/en/`. La référence de
typage reste [`src/sdk/island.ts`](../../src/sdk/island.ts) (source de vérité de l'API).
