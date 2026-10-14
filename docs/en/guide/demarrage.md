# Quick start

Goal: a minimal extension, loaded in Island in developer mode.

## 1. Start from the example

The easiest way is to copy `examples/extensions/github` from the repository, then trim it
down. A minimal extension contains:

```
my-extension/
├─ manifest.json
├─ icons/icon.svg          ← monochrome icon (tinted by Island)
├─ hello.html              ← tool view
├─ src/main.ts             ← logic (runs in the background)
├─ src/hello.ts            ← view script
└─ vite.config.ts
```

## 2. The manifest

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

The `id` is a reverse domain name (`a-z`, `0-9`, `-`, at least two segments). Details in
[Manifest & build](./manifeste).

## 3. The logic

```ts
// src/main.ts
import { connect } from "@island/sdk";

const island = await connect();
await island.live.set("status", { text: "Hi", color: "green" });
await island.notify({ title: "Hello is ready" });
```

## 4. The view

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
document.getElementById("app")!.innerHTML = `<div class="empty">Hello from ${island.id}</div>`;
```

## 5. Build

```bash
node ../../../node_modules/vite/bin/vite.js build
```

(from the extension folder, with the example's Vite config — see
[Manifest & build](./manifeste#build-with-vite)).

## 6. Load it in Island

1. **Settings › Extensions › Developer mode**: turn it on.
2. **Add a folder**: pick the extension folder (the one containing `manifest.json`).
3. The extension shows up with the **Dev** badge; its tool is added at the end of the dock.
4. After a new build: the **Reload** button next to the extension.

In developer mode, permissions are those of the manifest (no consent prompt).
