# 4. The `@island/sdk` SDK

> Source of truth for the types: `src/sdk/island.ts` in the Island repo.

## Two entry points

- **`defineExtension(def)`** — declares the extension (`surfaces`, `activate`, `deactivate`).
- **`useIsland(extId?)`** — inside a `.vue` component, returns the `IslandApi`. For
  **gated services** (guarded by a permission: `capture`, `system`, `media`, `network`,
  `terminal`…), pass your id: **`useIsland(EXT_ID)`** → the host checks your permission (see
  [Services & permissions](/en/guide/services)). Inside `activate(ctx)`, `ctx` is **already
  bound** to your id.

```ts
// inside a component
const EXT_ID = "com.island.myextension";
const island = useIsland(EXT_ID);
```

## The `activate(ctx)` context

`ctx` = the full API **+** `ctx.id` **+** `ctx.storage`. Key areas:

| Area           | Usage |
| -------------- | ----- |
| `ctx.launcher` | `register({label, icon, onActivate})` / `remove()` — launcher entry; `provider({onQuery})` / `removeProvider()` — feeds the launcher **search** (extensible palette) |
| `ctx.view`     | `open(component, opts)` / `close()` / `resize(size)` — mount/resize a view in the island |
| `ctx.drop`     | `open(component)` / `close()` — droplet (sub-slot of a view) |
| `ctx.window`   | `open(component, {title,icon,…}) → id` / `close(id?)` / `focus(id)` — draggable floating panel (minimizes to a sphere via `icon`) |
| `ctx.idle`     | `state(s)` (color), `center(component)` (custom viz), `action("left"\|"right", a)`, `tap(handler)` — contributes to the resting island |
| `ctx.notify`   | `notify(spec) → id` — banner + history; `notifications.dismiss/clear` |
| `ctx.media`    | `state` (reactive), `toggle/next/prev/seek/setVolume` — native media *(perm `media`)* |
| `ctx.capture`  | screenshot, recording, region selection… *(perm `capture`)* |
| `ctx.system`   | `stats()`, `battery()`, `online()`, `volume()/setVolume/setMuted`, `idleMs()/onUserIdle()` *(perm `system`)* |
| `ctx.windows`  | `foreground()`, `list()`, `focus(id)`, `onForegroundChanged(cb)` — desktop windows *(⚠ perm `windows`)* |
| `ctx.shortcuts`| `register(accel, handler)` / `unregister(accel)` — **global** shortcuts |
| `ctx.terminal` | `spawn/write/resize/kill/exec/onData/onExit` — PTY terminals (xterm) + one-shot exec *(⚠⚠ perm `terminal`, maximum trust)* |
| `ctx.storage`  | `get/set/delete/keys` — persistent key→value store, isolated per extension |
| `ctx.secrets`  | `get/set/delete` — **encrypted** vault (API tokens…), isolated per extension |
| `ctx.clipboard`| `readText/writeText/readImage/writeImage` — clipboard *(perm `clipboard`)* |
| `ctx.theme`    | `current()` / `onChange(cb)` — current theme (dark/light) |
| `ctx.bus`      | `emit(channel, payload)` / `on(channel, cb)` — pub/sub BETWEEN extensions |
| `ctx.speak`    | `speak(text)` — text-to-speech (reads out loud) |
| `ctx.input`    | `input.typeText(text)` — keystrokes into the active app *(⚠ perm `input`)* |
| `ctx.http`     | `request({extId, url, …})` — native cookie-jar HTTP *(perm `network`)* |
| `ctx.openExternal` | `(url)` — open an http(s) URL in the browser |
| `ctx.invoke` / `ctx.on` | raw access to host commands / events (escape hatch) |

## Surfaces in detail

### `view` — the main screen, inside the island

```ts
ctx.view.open(MyView, { width: 460, height: 320, radius: 26, persistent: true, safeZone: "absolute" });
ctx.view.resize({ width: 780, height: 560 });  // the island morphs smoothly, without remounting the view
ctx.view.close();
```

- **`persistent: true`**: the view stays open despite a click elsewhere / loss of focus
  (e.g. keeping stats visible). Otherwise, a click outside the island collapses it.
- **`safeZone`**: drives the top area (collapse handle + screen-edge margin):
  - `"relative"` *(default)* — the host reserves a top band (~14px) under the handle; your
    content starts below it. Keep normal padding and let the host manage the top.
  - `"absolute"` — your content goes to the top edge, the handle **floats over it** (+ a
    light scrim for legibility). Ideal for an **image banner**.
  - `"hidden"` — no handle nor reserve (internal use for notifications).

  > Legacy alias: `safeArea: true/false` is still accepted (`true → relative`, `false → absolute`).

### `drop` — droplet under a view

A sub-slot for a small side content (e.g. a volume slider under a player).
`ctx.drop.open(component)` / `ctx.drop.close()`.

### `window` — draggable floating panel

```ts
const ICON = "<svg …>…</svg>"; // SVG/lucide
const id = ctx.window.open(MyTool, { title: "Player", icon: ICON, width: 480, height: 270, resizable: true });
ctx.window.focus(id);
ctx.window.close(id);
```

A free panel (minimal bar: **minimize − / close ✕**), movable, independent of the island —
ideal for a player, a terminal or a mini-tool.

- **`icon`**: shown in the **sphere** when the user **minimizes** the window. Minimized
  windows appear as small spheres **to the right of the island**; clicking a sphere
  **restores** the window.

### `terminal` — PTY terminals (perm `terminal`)

```ts
const id = await ctx.terminal.spawn({ cwd: "/home/me/project", cols: 80, rows: 24 });
ctx.terminal.onData(({ id: i, b64 }) => { if (i === id) term.write(atob(b64)); }); // → xterm.js
term.onData((d) => ctx.terminal.write(id, d));
const { stdout } = await ctx.terminal.exec({ cmd: "git", args: ["status"], cwd }); // captured one-shot
```

⚠⚠ **Maximum trust** (runs arbitrary processes). Render it with **xterm.js** (bundled in
the extension). Full details in the [Services catalog](/en/reference/services-catalog)
(`terminal` section).

## The resting island: `idle`

```ts
ctx.idle.state("recording");                                 // color of the center circle (null = remove)
ctx.idle.center(MyViz);                                       // OR mount a component at the center (null = remove)
ctx.idle.action("right", { text: "00:12", color: "#ff453a" }); // right-edge shortcut (icon OR text)
ctx.idle.tap(() => ctx.view.open(MyView));                    // tap on the whole island → open your UI
```

- `state`: a **simple state** managed by the host (`idle | recording`) = the **color of the
  center circle**. `null` removes your contribution.
- `center(component)`: mounts a **custom component** at the center (rich viz — audio wave,
  3D sphere of a voice AI…). **Takes precedence** over the colored circle. `null` removes.
  Works like `view.open` (a Vue component from your extension mounted in the island).
- `action(slot, …)`: `slot` = `"left"` or `"right"`. An action **without `onActivate`** is
  a display (e.g. a counter) that lets the island-click through.
- `tap(handler)`: intercepts the click on the resting island (instead of opening the
  launcher). `null` removes.

## Notifications

```ts
const id = ctx.notify({
  title: "Capture saved",
  body: "1280×720 · 4 s",
  icon: "<svg …>",
  color: "#30d158",
  source: "Capture",
  timeout: 4500,           // ms on screen; 0 = history only
  onClick: () => { /* … */ },
});
```

Stack model: recent notifications stack up (5 visible), then retract. A bell appears in the
island while there are unread ones. `ctx.notifications.dismiss(id)` / `ctx.notifications.clear()`.

## Storage

```ts
await ctx.storage.set("key", value);          // any JSON value
const v = await ctx.storage.get("key", fallback);
await ctx.storage.delete("key");
const keys = await ctx.storage.keys();
```

**Isolated per extension**: 1 JSON file per id, never shared. Ideal for settings and
persistent state.

## Secrets (encrypted)

For **sensitive** data (API token, password), don't use `storage` (plaintext JSON): use
`ctx.secrets`, which stores in the system vault (Windows Credential Manager / Keychain).

```ts
await ctx.secrets.set("apiToken", token);
const token = await ctx.secrets.get("apiToken"); // string | null
await ctx.secrets.delete("apiToken");
```

Isolated per id (like `storage`): an extension only reads its own secrets.

## Theme

HTML automatically inherits the host's tokens (see the Tailwind contract), but **canvas/SVG**
rendering needs to know the theme on the JS side:

```ts
const island = useIsland();          // ungated: no extId needed
const t = island.theme.current();    // "dark" | "light"
const off = island.theme.onChange((t) => redraw(t));  // returns an unsubscribe function
// …on teardown: off()
```

## Global shortcuts

```ts
const ok = await ctx.shortcuts.register("Ctrl+Shift+Space", () => { /* … */ });
// ok = false if the combo is already taken by another extension or refused by the OS
await ctx.shortcuts.unregister("Ctrl+Shift+Space");
```

Nothing is registered until you call `register`. Everything is cleaned up on deactivation.

→ For native services (capture, media, network, apps) and their permissions:
[Services & permissions](/en/guide/services). For concrete examples:
[Recipes](/en/guide/recettes).
