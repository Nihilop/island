# Island

A **Dynamic Island for Windows** — a floating, shape‑shifting bar that sits at the top of
your screen and morphs to fit whatever you're doing. Island itself is **100% agnostic**:
every feature lives in an **extension** (a small Vue/TS module). The host only renders
surfaces and routes events.

> Built with Tauri 2 (Rust + WebView2), Vue 3, Pinia and Tailwind.

## Highlights

- 🏝️ **Morphing island** — idle pill, launcher, views, modal and floating windows, with
  smooth transitions; click‑through everywhere except the interactive island.
- 🧩 **Extensions, not a monolith** — media, screen capture/recording, an app launcher,
  a system monitor, an anime client… all are extensions consuming only `@island/sdk`.
- 🔔 **Notification center** — stacked banners, history, do‑not‑disturb.
- 🔌 **Rich SDK** — surfaces (`view`/`drop`/`window`/`modal`), launcher entries &
  **search providers**, idle indicators, notifications, persistent **storage** and
  encrypted **secrets**, an inter‑extension **bus**, plus permission‑gated native
  **services**: capture, system sensors, media (SMTC), clipboard, native HTTP with a
  **persistent cookie‑jar**, app launching, window awareness, keyboard automation, TTS.
- 🔐 **Per‑extension permissions** — each service is gated by a permission declared in the
  extension manifest and verified on every call.
- ⬆️ **Auto‑update** — signed releases via GitHub; installed apps update on next launch.

## Install

Grab the latest **`Island_*_x64-setup.exe`** (or the `.msi`) from the
[**Releases**](https://github.com/Nihilop/island/releases) page and run it. The app keeps
itself up to date automatically.

## Develop

```bash
pnpm install
pnpm tauri dev        # run the app
pnpm build            # type-check + build the frontend
```

Requirements: Node 20+, pnpm, the Rust toolchain, and the Tauri prerequisites for Windows.

### Writing an extension

An extension is a standalone Vue/TS mini‑project that only depends on `vue` and
`@island/sdk`, builds to `dist/index.mjs` + `dist/style.css`, and is packaged as a
`.island` file. The fastest way to start: in the app, **tray → Settings → Extensions →
“Create an extension”**.

Developer documentation lives in [`docs/fr/`](docs/fr/) (French first, English translation
to come) — see the [SDK guide](docs/fr/04-le-sdk.md) and the
[services & permissions reference](docs/fr/05-services-et-permissions.md).

## Releasing

Push a tag and CI builds, signs and publishes a release with installers and the
auto‑updater manifest:

```bash
git tag v0.6.0 && git push origin v0.6.0
```

See [`docs/release-and-updates.md`](docs/release-and-updates.md) for the full runbook.
