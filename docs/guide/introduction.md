# Introduction

Island est un notch pour Windows : une barre en haut de l'écran qui s'ouvre au survol (ou
au raccourci) et accueille des outils — lecture, minuteur, presse-papiers, coffre, launcher…
Une **extension** y ajoute ses propres outils et informations.

## Ce qu'une extension peut ajouter

Une extension ne crée jamais de fenêtre : elle s'ajoute à des **emplacements** du notch,
déclarés dans son manifeste.

| Emplacement | Ce que voit l'utilisateur | Rendu par |
|---|---|---|
| **Outil** | un bouton dans le dock, une vue dans le notch ouvert | l'extension (HTML/CSS/JS) |
| **Activité en direct** | une icône et un texte court quand le notch est fermé | Island |
| **Launcher** | des résultats dans le launcher (avec un préfixe optionnel, ex. `gh …`) | Island |
| **Notifications** | une bannière notch fermé + l'historique | Island |
| **Commandes** | une action avec raccourci global, cherchable dans le launcher | Island |
| **Réglages** | une section dans Réglages › Extensions | Island |

Tout ce qui est « rendu par Island » reprend automatiquement le design du notch (thème clair
ou sombre compris) : l'extension ne fournit que des données.

## Comment elle tourne

- Le code d'une extension s'exécute dans des **iframes isolées** : une cachée pour sa
  logique (`main`), une par vue d'outil ouverte.
- Elle n'a **aucun accès** à l'app, au réseau, au disque ni aux autres extensions. Tout passe
  par le **SDK** (`@island/sdk`), qui parle à Island ; Island vérifie les permissions.
- Elle est livrée en **paquet `.island` signé** ; l'utilisateur voit ses permissions avant de
  l'installer.

Voir [Permissions & sécurité](./permissions) pour le détail.

## Exemple de référence

L'extension **GitHub** (`examples/extensions/github` dans le dépôt) utilise tous les
emplacements : un outil « À revoir / Mes PR », l'activité en direct (revues en attente, CI),
des notifications, les dépôts dans le launcher (`gh …`), une commande « Rafraîchir » et une
page de réglages (jeton, fréquence). C'est le meilleur point de départ.

## Pour aller plus loin

- [Démarrage rapide](./demarrage) — de zéro à une extension chargée dans Island.
- [Manifeste & build](./manifeste) — les champs du manifeste, la config Vite.
- [Le SDK](./sdk) — toutes les fonctions disponibles.
- [Emplacements](./emplacements) — chaque emplacement en détail.
- [Recettes](./recettes) — des exemples courts.
- [Distribution](./distribution) — empaqueter, signer, installer.
