# Permissions & sécurité

## Les permissions

| Permission | Donne accès à |
|---|---|
| *(aucune)* | son stockage, ses réglages, ses traductions, le thème, `openUrl`, tous les emplacements |
| `secrets` | `island.secrets` (trousseau Windows, propre à l'extension) |
| `network:<domaine>` | `island.http` vers ce domaine ; `network:*.exemple.com` pour ses sous-domaines |
| `notifications` | `island.notify` |
| `clipboard` | `island.clipboard` |
| `cookies` | session par cookie sur ses domaines `network:` (cookies tenus par Island, jamais visibles) |
| `windows` | `island.windows.open` : ses propres fenêtres (`contributes.windows`) |
| `capture` | **à haut risque** (en rouge à l'installation) : `island.capture`, voir et enregistrer l'écran et le son |
| `images:<domaine>` | afficher des images HTTPS de ce domaine (`<img>`, CSS) sans passer par `island.http` |

Demandez le minimum : l'utilisateur voit la liste en clair avant d'installer, et une mise à
jour qui ajoute une permission la signale en orange.

**Jamais accessibles** : le coffre Bitwarden, Windows Hello, les clés d'IA, Discord, les
réglages d'Island, les autres extensions.

## Le modèle d'isolation

- **Iframes sandboxées.** Chaque page d'extension est chargée dans une iframe
  `sandbox="allow-scripts"` (origine opaque) et servie avec une CSP qui impose elle-même
  l'isolation (`sandbox allow-scripts`) : même ouverte hors de l'île, elle reste isolée.
- **Pas d'accès à l'app.** Une page d'origine opaque ne peut pas appeler les commandes
  d'Island (Tauri refuse sa requête). Les objets internes de Tauri ou de WebView2 visibles
  dans l'iframe sont inertes — vérifié par l'extension de test
  `examples/extensions/isolation`.
- **Pas de réseau direct, pas de fichiers d'autrui.** La CSP interdit `fetch`, et ne permet
  de charger que les fichiers de l'extension elle-même ; seule exception, les images des
  domaines `images:` consentis (affichage seulement : ni script, ni cookie).
- **Formulaires.** Le JavaScript d'une page voit l'événement `submit` (`allow-forms`), mais
  un formulaire n'est jamais envoyé nulle part (`form-action 'none'`).
- **Identité fixée par Island.** Island crée le canal de chaque iframe et sait quelle
  extension parle ; le code de l'extension ne peut pas se faire passer pour une autre.
- **Contrôles côté Rust** là où une donnée externe entre :
  - réseau : domaine autorisé relu dans le manifeste installé, HTTPS, aucune redirection,
    adresses privées et locales refusées au moment même de la connexion ;
  - fichiers : chemins canonicalisés et confinés au dossier de l'extension.

## Permissions consenties

À l'installation, Island enregistre les permissions acceptées par l'utilisateur
(`extensions-v2-state/<id>.json`, hors du dossier de l'extension). **Ce sont elles qui
comptent**, pas le manifeste : une extension copiée à la main sans passer par
l'installation n'a aucune permission. En mode développeur, le manifeste fait foi.
