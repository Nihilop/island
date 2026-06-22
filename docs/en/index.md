---
layout: home
hero:
  name: Island
  text: The Dynamic Island for Windows
  tagline: Agnostic slots + extensions. Vue 3 · Tauri 2 · WebView2.
  actions:
    - theme: brand
      text: Get started
      link: /en/guide/introduction
    - theme: alt
      text: GitHub
      link: https://github.com/Nihilop/island
features:
  - icon: 🏝️
    title: Agnostic host
    details: The island knows about no extension. It exposes slots (idle, view, modal, window, notification, launcher) that extensions fill through @island/sdk.
  - icon: 🧩
    title: Vue + TS extensions
    details: An extension is a small Vue/Vite project built to ESM. Dev == prod (same artifact), with the Vue/SDK runtime shared with the host.
  - icon: 🔐
    title: Explicit permissions
    details: Every backend service (capture, media, network, terminal…) is gated by a permission declared in the manifest and checked on every call.
  - icon: ⚡
    title: Native & light
    details: WGC capture, SMTC media, cookie-jar HTTP, PTY terminals, global shortcuts — exposed cleanly to the front-end without bloating the host.
---
