# 2. Démarrage rapide

## Le plus rapide : générer depuis un template

Dans Island : **tray → Réglages → Extensions → « Créer une extension »**. Choisis un
template, donne un nom → un projet prêt à coder est généré dans le dossier des
extensions.

- **Minimal** : une seule `view`, le strict nécessaire.
- **Complet** : `view` + `config` (modal), compteur persistant (`storage`),
  contribution idle et notification — une vitrine du SDK.

Les placeholders `__EXT_ID__` / `__EXT_NAME__` / `__EXT_SLUG__` sont substitués à la
génération. Les templates de référence vivent dans `src-tauri/templates/{minimal,complete}/`.

## Où vit une extension

Le dossier des extensions installées **EST** l'espace de dev (on code en place) :

```
%APPDATA%\com.nihil.island\extensions\<id>\
```

soit `C:\Users\<toi>\AppData\Roaming\com.nihil.island\extensions\<id>\`.

> ⚠️ Le dossier de config utilise l'**identifier** Tauri (`com.nihil.island`), pas
> « island ». C'est un piège classique.

Island scanne ce dossier au démarrage. Une extension qui contient un `package.json`
est marquée **dev** (badge + bouton « Packager » dans les Réglages). Un `.island`
packagé ne contient que `manifest.json` + `dist/`.

## Le cycle de dev

1. Ouvre un terminal dans le dossier de l'extension.
2. `pnpm install` puis **`pnpm dev`** (= `vite build --watch` → reconstruit `dist/` à
   chaque sauvegarde).
3. Dans Island : **Réglages → Extensions** → active l'extension.
4. Ouvre le launcher sur l'île → clique ton entrée → ta `view` s'affiche.

### Live-reload

- En **dev** (Island lancé via `pnpm tauri dev`) : un watcher de l'hôte surveille les
  `dist/`. Dès que `vite build --watch` réécrit ton build, l'extension est **rechargée
  à chaud** — pas de redémarrage.
- En **prod** (Island installé) : le `dist/` est lu et importé via une Blob URL. Pas de
  Vite requis.

> Une extension ajoutée pendant qu'Island tourne : rouvre les Réglages (re-scan) pour
> la voir apparaître, puis active-la. La (dés)activation est gérée à chaud
> (reconciliation runtime), sans redémarrage.

## Et ensuite

Tu as une extension qui tourne. Pour comprendre chaque fichier et le contrat de build
(externals `vue`/`@island/sdk`, Tailwind) → [Anatomie & contrat de build](03-anatomie-et-build.md).
Pour l'API → [Le SDK](04-le-sdk.md).
