---
layout: home
hero:
  name: Island
  text: The island for Windows
  tagline: Your tools in an island at the top of the screen. Extensible, isolated, signed.
  actions:
    - theme: brand
      text: Write an extension
      link: /en/guide/introduction
    - theme: alt
      text: Quick start
      link: /en/guide/demarrage
    - theme: alt
      text: GitHub
      link: https://github.com/Nihilop/island
features:
  - icon: 🏝️
    title: Slots, not windows
    details: An extension plugs into specific places in the island — dock tool, live activity, launcher results, commands, settings. Everything stays in the island.
  - icon: 🛡️
    title: Isolated by design
    details: Each extension runs in a sandboxed iframe, with no access to the app, the network or other extensions. It only talks to Island, which knows its identity.
  - icon: 🔐
    title: Consented permissions
    details: Network limited to declared domains, secrets, notifications, clipboard… The user accepts the list at install time, and Island checks it on every call.
  - icon: ✍️
    title: Signed packages
    details: An extension ships as a signed .island package (ed25519). Developer mode loads a folder and hot-reloads it.
---
