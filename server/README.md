# Magasin d'Island — spécification (v1, en cours)

Serveur du magasin d'Island : catalogue de **scènes de fonds animés** et d'**extensions**,
publication par les auteurs, téléchargement et mises à jour depuis l'app. Statut :
**spécification**, rien n'est encore codé ici.

## Principes

- **Une API, pas de site (v1).** La vitrine est un écran *Magasin* dans Island ; les
  auteurs publient en ligne de commande (`island publish`, SDK). Un site public (pages
  partageables, tableau de bord auteur) viendra plus tard, sur la même API.
- **Mêmes règles que l'app.** Toute validation passe par `crates/island-formats` (scènes,
  `.islandscene`, paquets `.island`) : un fichier accepté par le serveur l'est par l'app, et
  inversement.
- **Deux contenus, deux modérations.**
  - *Scène* = des **données** : validée automatiquement, publiée aussitôt, signalable.
  - *Extension* = du **code** : relecture manuelle, puis **signée par le magasin** ; jamais
    publiée sans relecture.
- **Rien de secret dans le dépôt.** Clé de signature du magasin, secrets OAuth, accès au
  stockage : variables d'environnement / secrets de l'hébergeur. La clé officielle d'Island
  (`C:\Users\…\.island\island-official-key.json`) n'est **jamais** utilisée par le serveur.

## Architecture

| Rôle | Choix |
|---|---|
| API | Rust, **Axum** (+ `tower` : limites de taille, délais, limitation de débit) |
| Base | **SQLite** (`sqlx`, migrations dans `migrations/`), sauvegarde continue **Litestream** |
| Fichiers | **Cloudflare R2** (S3) en production ; dossier local en développement (trait `Storage`) |
| Validation | `island-formats` (dépendance par chemin : `../crates/island-formats`) |
| Hébergement | un binaire, un petit VPS ou Fly.io ; HTTPS par le proxy (Caddy / Fly) |
| Auth | **GitHub**, *device flow* (CLI et app sans navigateur intégré) → jeton d'API |

Un seul processus. Pas de workspace Cargo avec l'app (cf. *Dépôt*).

## Modèle de données

```sql
users     (id, github_id UNIQUE, login, avatar_url, role TEXT /* user|moderator|admin */,
           banned INTEGER, created_at)
tokens    (id, user_id, name, hash /* sha-256 du jeton, jamais le jeton */, created_at,
           last_used_at)
items     (id TEXT PRIMARY KEY /* scène : généré ; extension : id du manifeste */,
           kind TEXT /* scene|extension */, owner_id, name, summary, description,
           tags TEXT /* JSON */, status TEXT /* published|hidden|removed */,
           downloads INTEGER, created_at, updated_at)
versions  (id, item_id, version TEXT, status TEXT /* pending|approved|rejected|yanked */,
           file_key /* clé R2 du fichier publié */, size, sha256, permissions TEXT /* JSON,
           extensions */, changelog, reject_reason, reviewed_by, created_at, reviewed_at)
assets    (item_id, kind TEXT /* thumbnail|screenshot */, file_key, position)
reports   (id, item_id, version_id, user_id NULL, reason TEXT, details, created_at,
           resolved_by, resolved_at)
```

- **Identifiant d'une extension** = l'`id` de son manifeste (`com.exemple.ext`). Le premier
  auteur qui le publie le **réserve** ; seules ses versions suivantes sont acceptées.
- **Identifiant d'une scène** : généré par le serveur (`sc_…`) ; le nom n'est pas unique.
- **Version** : semver pour les extensions (doit augmenter) ; pour une scène, un compteur.

## API (JSON, préfixe `/v1`)

### Public (sans compte)

| | |
|---|---|
| `GET /items?kind=&q=&tag=&sort=popular\|recent&page=` | catalogue (cartes : id, type, nom, résumé, auteur, vignette, téléchargements) |
| `GET /items/{id}` | fiche : description, versions publiées, permissions (extension), captures |
| `GET /items/{id}/download?version=` | 302 vers le fichier (URL R2 signée, courte durée) ; compte un téléchargement |
| `GET /items/{id}/thumbnail` | vignette |
| `POST /updates` `{ extensions: [{ id, version }] }` | dernières versions approuvées + permissions **ajoutées** (pour l'alerte orange de l'app) |
| `POST /items/{id}/reports` `{ reason, details }` | signalement (anonyme possible, limité par IP) |

### Auteurs (jeton `Authorization: Bearer …`)

| | |
|---|---|
| `POST /auth/device` → `{ user_code, verification_uri, device_code }` | début de connexion GitHub |
| `POST /auth/token` `{ device_code }` | attend la validation, renvoie un **jeton d'API** (montré une seule fois) |
| `POST /items` (multipart : fichier + métadonnées) | nouvel élément **ou** nouvelle version |
| `PATCH /items/{id}` | nom, résumé, description, étiquettes, captures |
| `DELETE /items/{id}` | retire du catalogue (les installations existantes ne sont pas touchées) |
| `POST /items/{id}/versions/{v}/yank` | retire une version (bug grave) |
| `GET /me/items` | mes éléments et l'état de leurs versions |

### Modération (`moderator`, `admin`)

| | |
|---|---|
| `GET /mod/queue` | versions d'extensions en attente, signalements ouverts |
| `POST /mod/versions/{id}/approve` | **signe** le paquet avec la clé du magasin et le publie |
| `POST /mod/versions/{id}/reject` `{ reason }` | refus motivé (visible par l'auteur) |
| `POST /mod/items/{id}/hide` / `restore` | masquer / rétablir un élément |
| `POST /mod/reports/{id}/resolve` | clore un signalement |

Erreurs : `{ "error": "<code>" }` (codes stables : ceux de `island-formats` —
`corrupt`, `tooLarge`, `sceneTooNew`, `missingFile:…`, `invalidImage:…`, `badPath:…`,
`manifest:…` — plus `unauthorized`, `forbidden`, `notFound`, `idTaken`,
`versionNotGreater`, `rateLimited`).

## Publication

1. **Scène** (`.islandscene`) : `islandscene::read` (tailles, noms simples, images décodées,
   scène validée). La **vignette est régénérée** par le serveur (`islandscene::thumbnail`) :
   on ne fait pas confiance à celle du fichier. Publiée aussitôt.
2. **Extension** (`.island`) : `package::read` (chemins sûrs, tailles, manifeste valide,
   fichiers présents). L'id doit être libre ou à l'auteur, la version doit augmenter. Statut
   `pending` → file de modération.
3. **Relecture d'une extension** : code lisible (pas de minification obscure pour la v1),
   permissions justifiées par la description, pas d'appel réseau hors domaines déclarés
   (déjà imposé par Island), comportement conforme à la fiche.

## Signature des extensions

- À l'approbation, le serveur **remplace** `ISLAND-SIGNATURE.json` par une signature de la
  **clé du magasin** sur la même empreinte (`package::digest`, qui ignore ce fichier). Le
  paquet publié est donc signé par le magasin, quel que soit l'auteur.
- **Côté app** : la clé publique du magasin s'ajoute aux clés de confiance, avec un état
  distinct `Store` (« Vérifiée par le magasin ») à côté d'`Official` (« Extension
  officielle ») ; `Untrusted` / `Unsigned` restent réservés au mode développeur.
- Clé du magasin : ed25519, générée hors du dépôt (`island keygen`), fournie au serveur par
  un secret (`STORE_SIGNING_KEY`). Rotation : l'app accepte une liste de clés.

## L'app

- **Écran Magasin** (outil ou page des Réglages) : catalogue, recherche, fiche, bouton
  *Installer* / *Appliquer*. Les appels passent par le Rust (`ureq`, déjà présent).
- **Installation** : téléchargement dans un fichier temporaire, puis les chemins existants —
  extension : `ext2_inspect` / `ext2_install` (consentement des permissions inchangé) ;
  scène : `wallpaper::share::import`.
- **Mises à jour** : `POST /updates` au démarrage puis toutes les 24 h, mêmes alertes
  qu'aujourd'hui (permission ajoutée = orange).
- **Plus tard** : lien `island://store/<id>` (pages web partageables qui ouvrent l'app).

## Sécurité et limites

- Tailles : celles d'`island-formats` (`.islandscene` 256 Mo, `.island` 64 Mo décompressé),
  et un quota par auteur. Corps de requête borné avant toute lecture.
- Limitation de débit : par jeton (publication) et par IP (catalogue, signalements).
- Jetons stockés **hachés** ; jeton révocable ; un compte banni perd ses jetons.
- Pas de HTML rendu par le serveur ; les descriptions sont du texte (Markdown limité, rendu
  et assaini par l'app).
- Vie privée : compteur de téléchargements agrégé, pas de journal d'IP au-delà de la
  limitation de débit.
- Droits : l'auteur garde ses droits et accorde au magasin la redistribution ; procédure de
  retrait sur demande (contenu protégé, signalement fondé).

## Dépôt

- `crates/island-formats` : formats partagés, dépendance **par chemin** de `src-tauri` et de
  `server` — pas de workspace Cargo (le `target/` de l'app et la CI de release ne bougent
  pas).
- `server/` : ce serveur. Workflow CI séparé (`server/**`, `crates/**` → `cargo test`), sans
  lien avec la release de l'app (tags `v*`).

## Configuration (variables d'environnement)

| Variable | |
|---|---|
| `DATABASE_URL` | `sqlite://data/store.db` |
| `STORAGE` | `local:./data/files` (dev) ou `r2` |
| `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET` | stockage |
| `GITHUB_CLIENT_ID` | OAuth (device flow : pas de secret client nécessaire) |
| `STORE_SIGNING_KEY` | clé privée ed25519 du magasin (PEM) |
| `PUBLIC_URL` | `https://store.island…` |

## Étapes

1. ✅ `crates/island-formats` (formats partagés, tests).
2. Serveur : squelette Axum, SQLite + migrations, stockage local, **scènes** (catalogue,
   fiche, téléchargement, publication via jeton, signalement), tests d'API.
3. Auth GitHub (device flow), jetons.
4. Écran Magasin dans Island + `island publish` dans le SDK.
5. Extensions : file de modération, signature du magasin, état `Store` dans l'app, mises à
   jour.
6. Production : R2, Litestream, déploiement, CI.
7. Plus tard : site public, tableau de bord auteur, liens `island://`.
