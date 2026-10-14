# Démarrage rapide

Objectif : une extension minimale, chargée dans Island en mode développeur.

## 1. Partir de l'exemple

Le plus simple est de copier `examples/extensions/github` du dépôt, puis de l'alléger. Une
extension minimale contient :

```
mon-extension/
├─ manifest.json
├─ icons/icon.svg          ← icône monochrome (teintée par Island)
├─ hello.html              ← vue d'outil
├─ src/main.ts             ← logique (tourne en fond)
├─ src/hello.ts            ← script de la vue
└─ vite.config.ts
```

## 2. Le manifeste

```json
{
  "id": "dev.moi.hello",
  "name": "Hello",
  "version": "0.1.0",
  "icon": "icons/icon.svg",
  "main": "dist/main.js",
  "permissions": ["notifications"],
  "contributes": {
    "tools": [{ "id": "hello", "title": "Hello", "view": "dist/hello.html", "size": [480, 240] }],
    "live": [{ "id": "status", "tool": "hello" }]
  }
}
```

L'`id` est un nom de domaine inversé (`a-z`, `0-9`, `-`, au moins deux segments). Détails
dans [Manifeste & build](./manifeste).

## 3. La logique

```ts
// src/main.ts
import { connect } from "@island/sdk";

const island = await connect();
await island.live.set("status", { text: "Hi", color: "green" });
await island.notify({ title: "Hello est prêt" });
```

## 4. La vue

```html
<!-- hello.html -->
<!doctype html>
<meta charset="utf-8" />
<div id="app"></div>
<script type="module" src="./src/hello.ts"></script>
```

```ts
// src/hello.ts
import "@island/sdk/ui.css";
import { connect } from "@island/sdk";

const island = await connect();
document.getElementById("app")!.innerHTML = `<div class="empty">Bonjour depuis ${island.id}</div>`;
```

## 5. Build

```bash
node ../../../node_modules/vite/bin/vite.js build
```

(depuis le dossier de l'extension, avec la config Vite de l'exemple — voir
[Manifeste & build](./manifeste#build-avec-vite)).

## 6. Charger dans Island

1. **Réglages › Extensions › Mode développeur** : activer.
2. **Ajouter un dossier** : choisir le dossier de l'extension (celui qui contient
   `manifest.json`).
3. L'extension apparaît avec le badge **Dév.** ; son outil rejoint la fin du dock.
4. Après un nouveau build : bouton **Recharger** à côté de l'extension.

En mode développeur, les permissions sont celles du manifeste (pas de consentement).
