---
layout: home
hero:
  name: Island
  text: L'île de Windows
  tagline: Vos outils dans une île, en haut de l'écran. Extensible, isolé, signé.
  actions:
    - theme: brand
      text: Écrire une extension
      link: /guide/introduction
    - theme: alt
      text: Démarrage rapide
      link: /guide/demarrage
    - theme: alt
      text: GitHub
      link: https://github.com/Nihilop/island
features:
  - icon: 🏝️
    title: Des emplacements, pas des fenêtres
    details: Une extension s'ajoute à des endroits précis de l'île — outil du dock, activité en direct, résultats du launcher, commandes, réglages. Tout reste dans l'île.
  - icon: 🛡️
    title: Isolée par construction
    details: Chaque extension tourne dans une iframe sandboxée, sans accès à l'app, au réseau ni aux autres extensions. Elle ne parle qu'à Island, qui connaît son identité.
  - icon: 🔐
    title: Permissions consenties
    details: Réseau limité aux domaines déclarés, secrets, notifications, presse-papiers… L'utilisateur accepte la liste à l'installation, Island la vérifie à chaque appel.
  - icon: ✍️
    title: Paquets signés
    details: Une extension se distribue en paquet .island signé (ed25519). Le mode développeur charge un dossier et recharge à chaud.
---
