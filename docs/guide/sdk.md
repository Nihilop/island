# Le SDK

`@island/sdk` (dossier `packages/sdk` du dépôt) est la seule porte d'une extension vers
Island. Il s'utilise de la même façon dans la logique (`main`) et dans les vues.

```ts
import { connect } from "@island/sdk";
const island = await connect();
```

`connect()` attend la poignée de main d'Island puis renvoie l'objet `island`. Toutes les
méthodes renvoient des promesses ; une permission manquante rejette avec `permission:<nom>`.

## Identité, langue, thème

| | |
|---|---|
| `island.id` | Identifiant de l'extension (fourni par Island, pas déclaré par le code) |
| `island.locale` | Langue d'Island au démarrage (`fr`, `en`…) |
| `island.theme` | `"dark"` ou `"light"` ; le SDK pose la classe `light` sur `<html>` |
| `island.onTheme(cb)` | Changement de thème en direct |
| `island.t(clé, params?)` | Texte traduit (fichiers `locales`), `{param}` remplacés |
| `island.log(...args)` | Trace dans la console de développement d'Island |

## Données

| | Permission |
|---|---|
| `island.storage.get(clé)` / `set(clé, valeur)` | aucune — stockage JSON propre à l'extension |
| `island.secrets.get(clé)` / `set(clé, valeur)` / `delete(clé)` | `secrets` — trousseau Windows, propre à l'extension |
| `island.settings.get(clé)` | aucune — valeur d'un réglage déclaré (défaut du manifeste sinon) |
| `island.settings.onChange(cb)` | aucune — appelé avec la clé modifiée |

## Réseau

```ts
const r = await island.http.request({ url: "https://api.github.com/user", method: "GET", headers: { Authorization: `Bearer ${token}` } });
// r = { status, headers: [nom, valeur][], body: string }

const user = await island.http.json<{ login: string }>("https://api.github.com/user", { headers });
// lève HttpError(status, body) si status ≥ 400
```

Permission `network:<domaine>` (`network:*.exemple.com` pour les sous-domaines). **HTTPS
seulement**, aucune redirection suivie, réponse limitée à 5 Mo, adresses locales et privées
refusées.

## Notch

| | Permission |
|---|---|
| `island.live.set(id, spec \| null)` | aucune — [activité en direct](./emplacements#activite-en-direct) |
| `island.launcher.provide(id, recherche)` | aucune — [résultats du launcher](./emplacements#launcher) |
| `island.launcher.onRun(id, action)` | aucune — résultat choisi |
| `island.commands.on(id, action)` | aucune — [commande](./emplacements#commandes) déclenchée |
| `island.notify({ title, body?, banner? })` | `notifications` |
| `island.openUrl(url)` | aucune — http(s) seulement, navigateur par défaut |
| `island.clipboard.readText()` / `writeText(texte)` | `clipboard` |

## Kit UI

```ts
import "@island/sdk/ui.css";
```

Couleurs et typographie du notch (variables `--label`, `--label-2`, `--fill-1`…, `--green`,
`--orange`, `--red`, `--blue`), fond transparent, et quelques classes : `.list`, `.item`,
`.btn` (`.primary`), `.badge` (`.green`, `.orange`…), `.seg` (onglets), `.empty`, `.caption`,
`.muted`, `.truncate`. Le thème clair est géré automatiquement.
