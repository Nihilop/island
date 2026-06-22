---
layout: home
hero:
  name: Island
  text: La Dynamic Island pour Windows
  tagline: Des slots agnostiques + des extensions. Vue 3 · Tauri 2 · WebView2.
  actions:
    - theme: brand
      text: Démarrer
      link: /guide/introduction
    - theme: alt
      text: Écrire une extension
      link: /reference/authoring
    - theme: alt
      text: GitHub
      link: https://github.com/Nihilop/island
features:
  - icon: 🏝️
    title: Host agnostique
    details: L'île ne connaît aucune extension. Elle expose des slots (idle, view, modal, window, notif, launcher) que les extensions remplissent via @island/sdk.
  - icon: 🧩
    title: Extensions Vue + TS
    details: Une extension est un mini-projet Vue/Vite buildé en ESM. Dev == prod (même artefact), runtime Vue/SDK partagé avec l'hôte.
  - icon: 🔐
    title: Permissions explicites
    details: Chaque service backend (capture, media, network, terminal…) est gardé par une permission déclarée au manifeste et vérifiée à chaque appel.
  - icon: ⚡
    title: Natif & léger
    details: Capture WGC, média SMTC, HTTP cookie-jar, terminaux PTY, raccourcis globaux — exposés proprement au front, sans alourdir l'hôte.
---
