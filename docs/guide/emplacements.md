# Emplacements

Les emplacements sont déclarés dans `contributes` du manifeste : Island sait ce qu'une
extension ajoute **sans exécuter son code** (et l'affiche à l'installation).

## Outil

```json
"tools": [{ "id": "inbox", "title": "GitHub", "icon": "icons/github.svg", "view": "dist/inbox.html", "size": [560, 340] }]
```

- Un bouton dans le dock (ordre et activation dans Réglages › Outils, comme les outils
  intégrés). Trop d'outils : les derniers passent dans le menu « ••• ».
- `view` : page HTML affichée dans le notch ouvert, sous l'en-tête et au-dessus du dock.
- `size` : largeur × hauteur du notch ouvert, en-tête et dock compris (max. 740 × 540).
- La vue est rechargée à chaque ouverture : gardez l'état utile dans `island.storage`.

## Activité en direct

```json
"live": [{ "id": "status", "tool": "inbox" }]
```

```ts
await island.live.set("status", { icon: "icons/ci.svg", text: "3", color: "orange" });
await island.live.set("status", null); // retirer
```

| Champ | |
|---|---|
| `icon` | Chemin d'un SVG monochrome de l'extension, teinté par `color` |
| `text` | Texte court (une douzaine de caractères visibles) |
| `color` | `label`, `green`, `orange`, `red`, `blue` |
| `priority` | `normal` (après la musique) ou `high` (avant le minuteur, l'appel et la musique) |

Visible notch fermé ; un clic ouvre l'outil désigné par `tool`. Une seule activité est
affichée à la fois : les alertes d'Island (minuteur terminé, notification) passent devant.

## Launcher

```json
"launcher": [{ "id": "repos", "prefix": "gh", "title": "GitHub" }]
```

```ts
island.launcher.provide("repos", async (q) =>
  repos.filter((r) => r.name.includes(q)).map((r) => ({ id: r.url, title: r.name, subtitle: r.description })),
);
island.launcher.onRun("repos", (id) => island.openUrl(id));
```

- Sans préfixe : la recherche est envoyée à partir de 2 caractères ; 5 résultats maximum
  sous ceux d'Island.
- Avec préfixe (`gh island`) : seul ce fournisseur répond, jusqu'à 40 résultats.
- Les réponses périmées (l'utilisateur a continué à taper) sont ignorées par Island.

## Notifications

Permission `notifications`. `island.notify({ title, body })` passe par l'API unique
d'Island : bannière **minimale** ou **complète** selon le réglage de l'utilisateur quand le
notch est fermé, rien s'il est ouvert, historique dans l'outil Notifications.
`banner: false` = historique seulement.

## Commandes

```json
"commands": [{ "id": "refresh", "title": "GitHub : rafraîchir", "shortcut": "Control+Alt+G" }]
```

```ts
island.commands.on("refresh", () => refresh());
```

Le raccourci est global (format des accélérateurs Tauri). S'il est déjà pris, la commande
reste accessible dans le launcher (recherche par titre).

## Réglages

```json
"settings": [
  { "key": "token", "type": "secret", "label": "Jeton d'accès", "help": "…" },
  { "key": "interval", "type": "select", "label": "Rafraîchissement", "default": "5",
    "options": [{ "value": "2", "label": "2 min" }, { "value": "5", "label": "5 min" }] },
  { "key": "notify", "type": "switch", "label": "Notifications", "default": true }
]
```

Types : `text`, `secret` (rangé dans le trousseau, jamais réaffiché), `switch`, `select`,
`number`, `list` (une valeur par ligne → tableau). Island génère la page dans Réglages ›
Extensions ; l'extension lit avec `island.settings.get(clé)` et suit `onChange`.
