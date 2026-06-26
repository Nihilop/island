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

## Signer une extension (optionnel, recommandé)

À l'installation, l'écran de consentement affiche un **badge de confiance** : 🟢 *Signée par
un éditeur de confiance*, 🟠 *Non signée*, ou 🔴 *Signature invalide*. La signature est
**advisory** : une extension non signée s'installe quand même (à la responsabilité de
l'utilisateur), mais une extension signée rassure.

Island utilise des **signatures détachées [minisign](https://jedisct1.github.io/minisign/)**
(même mécanisme que l'updater).

```bash
# 1. Générer ta paire de clés UNE FOIS (garde la clé privée SECRÈTE, hors du dépôt).
minisign -G -p island-ext.pub -s island-ext.key

# 2. Signer le paquet → produit `mon-extension.island.minisig`.
minisign -S -s island-ext.key -m mon-extension.island

# 3. Distribue le `.minisig` À CÔTÉ du `.island` (même dossier, même nom de base).
#    Island cherche automatiquement `<paquet>.island.minisig` à l'installation.
```

> **Badge « de confiance »** : le statut 🟢 n'apparaît que si la **clé publique** qui a signé
> est celle **embarquée dans le build d'Island** (`EXT_TRUSTED_PUBKEY`). Pour les extensions
> first-party, signe avec la clé du mainteneur. Une signature d'une clé inconnue → 🔴 *invalide* ;
> pas de `.minisig` → 🟠 *non signée*. Dans les deux cas l'install reste possible (advisory).

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
- [ ] (optionnel) Paquet **signé** : `<paquet>.island.minisig` joint au `.island`.

## Et après

La traduction anglaise de cette documentation viendra dans `docs/en/`. La référence de
typage reste [`src/sdk/island.ts`](../../src/sdk/island.ts) (source de vérité de l'API).
