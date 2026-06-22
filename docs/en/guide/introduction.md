# 1. Introduction & model

## The host is agnostic

Island (the app) only **presents** surfaces and **routes** events. It knows nothing about
any particular extension: no "hardcoded id", no Spotify/capture/etc. logic in the core.
Everything that shows up in the island comes from an extension via the SDK.

Practical consequence: **anything you can do, another extension can do too.** The
primitives are generic.

## What is an extension?

A **self-contained Vue/TypeScript mini-project** that:

- depends, at runtime, only on **`vue`** and **`@island/sdk`** (both provided by the host
  — see the build contract);
- compiles to a **single ESM module** `dist/index.mjs` (+ `dist/style.css`);
- default-exports a `defineExtension({...})` object.

The host reads **only** `manifest.json` + `dist/`. The source code sitting next to it is
**ignored** (that's your workspace; you rebuild it with your own `pnpm dev`).

## Surfaces: where your UI shows up

A surface is a Vue component the host mounts somewhere in the UI:

| Surface  | Where | For |
| -------- | ----- | --- |
| `view`   | **inside the island** (it morphs to the requested size) | the extension's main screen |
| `config` | in the centered **modal** | the extension's settings |
| `drop`   | **droplet** below a view (sub-slot) | a small side content (e.g. volume slider) |
| `window` | draggable **floating panel** | a free, movable tool (player, mini-window) |

You declare which surfaces exist in `manifest.json`, and wire the actual Vue component
in `index.ts`.

## Contributions: what you add to the island

Beyond surfaces, an extension **contributes** to the island without "owning" it:

- **`launcher`** — an entry (label + icon) in the island's launcher.
- **`idle`** — at the center: a simple state (`recording`… = color) **or** a custom
  component (`idle.center` — rich viz); and/or shortcuts at the edges while the island is
  at rest; a *tap* on the island opens your UI.
- **`notify`** — a notification (banner + history).

Several extensions coexist: their contributions are merged and cleaned up automatically on
deactivation.

## Lifecycle

```ts
export default defineExtension({
  surfaces: { view: MyView },
  activate(ctx) { /* the extension starts: register launcher, idle, watchers… */ },
  deactivate() { /* optional: manual cleanup */ },
});
```

`activate(ctx)` receives the **context**: the full Island API **+** `ctx.id` (your
identifier) **+** `ctx.storage` (your isolated storage). The host manages the scope:
`watch`/`watchEffect` created inside `activate` are stopped on deactivation, and your
contributions (launcher, idle, surfaces) are removed.

→ Next: [Quick start](/en/guide/demarrage).
