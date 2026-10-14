# Introduction

Island is a notch for Windows: a bar at the top of the screen that opens on hover (or with
a shortcut) and hosts tools — playback, timer, clipboard, vault, launcher…
An **extension** adds its own tools and information to it.

## What an extension can add

An extension never creates a window: it plugs into **slots** of the notch, declared in its
manifest.

| Slot | What the user sees | Rendered by |
|---|---|---|
| **Tool** | a button in the dock, a view in the open notch | the extension (HTML/CSS/JS) |
| **Live activity** | an icon and a short text while the notch is closed | Island |
| **Launcher** | results in the launcher (with an optional prefix, e.g. `gh …`) | Island |
| **Notifications** | a banner while the notch is closed + the history | Island |
| **Commands** | an action with a global shortcut, searchable in the launcher | Island |
| **Settings** | a section in Settings › Extensions | Island |

Everything "rendered by Island" automatically follows the notch design (light and dark
themes included): the extension only provides data.

## How it runs

- An extension's code runs in **isolated iframes**: a hidden one for its logic (`main`),
  and one per open tool view.
- It has **no access** to the app, the network, the disk or other extensions. Everything goes
  through the **SDK** (`@island/sdk`), which talks to Island; Island checks permissions.
- It ships as a **signed `.island` package**; the user sees its permissions before
  installing it.

See [Permissions & security](./permissions) for details.

## Reference example

The **GitHub** extension (`examples/extensions/github` in the repository) uses every slot:
a "To review / My PRs" tool, the live activity (pending reviews, CI), notifications,
repositories in the launcher (`gh …`), a "Refresh" command and a settings page (token,
interval). It's the best starting point.

## Going further

- [Quick start](./demarrage) — from zero to an extension loaded in Island.
- [Manifest & build](./manifeste) — the manifest fields, the Vite config.
- [The SDK](./sdk) — every available function.
- [Slots](./emplacements) — each slot in detail.
- [Recipes](./recettes) — short examples.
- [Distribution](./distribution) — package, sign, install.
