# Magasin d'Island — spécification (v1, en cours)

Serveur du magasin d'Island : catalogue de **scènes de fonds animés** et d'**extensions**,
publication par les auteurs, téléchargement et mises à jour depuis l'app. Statut : étape 4
(**scènes** de bout en bout, **connexion GitHub** — device flow pour le CLI et l'app, OAuth
web pour le panneau —, rôles, jetons, **panneau d'administration**) ; l'écran Magasin dans
Island et les extensions suivent.

## Développement

```bash
cd server
docker compose up -d                       # Postgres de développement (port 5433)
export DATABASE_URL=postgres://island:island@localhost:5433/island_store
cargo run -- token moi --role admin        # compte + jeton d'API (affiché une seule fois)
cargo run                                  # API sur http://127.0.0.1:8080
cargo test                                 # tests (une base neuve par test)
cd admin && pnpm install && pnpm build     # panneau, servi ensuite sur http://127.0.0.1:8080/admin
```

Configuration locale : `server/.env` (lu au démarrage, jamais commité) — partir de
`server/.env.example`. Panneau en
développement avec rechargement à chaud : `cd admin && pnpm dev` (http://127.0.0.1:5180/admin/,
API relayée vers le serveur) — la connexion GitHub, elle, passe par le serveur (8080).

Connexion GitHub (facultative en développement) : créer une **OAuth App** GitHub
(*Settings › Developer settings › OAuth Apps*), cocher **Enable Device Flow**, URL de
retour `http://127.0.0.1:8080/admin/callback`, puis :

```bash
export GITHUB_CLIENT_ID=… GITHUB_CLIENT_SECRET=… ADMIN_GITHUB_ID=<ton id numérique>
# ton id : curl -s https://api.github.com/users/<login> | grep '"id"'
```

Publier une scène exportée depuis Island :

```bash
curl -H "Authorization: Bearer $TOKEN" -F file=@ma-scene.islandscene      -F summary="Une nuit de neige" -F 'tags=["neige","nuit"]' http://127.0.0.1:8080/v1/items
```

## Principes

- **Une API ; côté web, seulement l'administration (v1).** La vitrine est un écran
  *Magasin* dans Island ; les auteurs publient en ligne de commande (`island publish`,
  SDK). Un **panneau d'administration web** (`/admin`) sert à modérer et à gérer les
  comptes. Un site public (pages partageables, tableau de bord auteur) viendra plus tard,
  sur la même API et le même socle que le panneau.
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
| API | Rust, **Axum** (+ `tower` : limites de taille, délais, limitation de débit), **sans état** |
| Base | **Postgres hébergé : Neon** (`sqlx`, migrations dans `migrations/`) |
| Fichiers | **Cloudflare R2** (S3) en production ; dossier local en développement (trait `Storage`) |
| Validation | `island-formats` (dépendance par chemin : `../crates/island-formats`) |
| Analyse | **ClamAV** (`clamd`, à côté du serveur), optionnel — cf. *Sécurité des fichiers* |
| Administration | **Vue + Vite** dans `server/admin/`, construit et servi par Axum sur `/admin` |
| Hébergement | Fly.io ou Railway, sans disque ; HTTPS par la plateforme |
| Auth | **GitHub** : *device flow* (CLI, app) → jeton d'API ; OAuth web (panneau admin) → cookie |

Le serveur ne garde aucun état : tout est dans Postgres et R2, on le redéploie ou on en
lance plusieurs sans rien gérer. Pas de workspace Cargo avec l'app (cf. *Dépôt*).

**Pourquoi Neon (et pas Supabase)** : sur la formule gratuite, Supabase met le projet en
**pause après une semaine sans activité** (relance manuelle) — pour un magasin qui démarre,
c'est la panne assurée. Neon **s'endort et se réveille seul** à la première requête (quelques
centaines de ms) et offre des **branches** de base (copie pour le développement ou les tests,
sans toucher à la production). Le serveur ne connaît qu'une `DATABASE_URL` : changer
d'hébergeur Postgres plus tard ne coûte rien. Supabase Storage et Supabase Auth ne
conviendraient de toute façon pas (sortie de bande passante payante pour des fichiers jusqu'à
256 Mo ; pas de *device flow*).

## Modèle de données

```sql
users     (id, github_id BIGINT UNIQUE, login, avatar_url,
           role TEXT /* user|moderator|admin */, banned BOOLEAN, created_at, last_seen_at)
tokens    (id, user_id, name, hash /* sha-256 du jeton, jamais le jeton */, created_at,
           last_used_at)
items     (id TEXT PRIMARY KEY /* scène : généré ; extension : id du manifeste */,
           kind TEXT /* scene|extension */, owner_id, name, summary, description,
           tags JSONB, search TSVECTOR /* nom + résumé + description, 'french' + unaccent */,
           status TEXT /* published|hidden|removed */,
           downloads INTEGER, created_at, updated_at)
versions  (id, item_id, version TEXT, status TEXT /* pending|approved|rejected|yanked */,
           file_key /* clé R2 du fichier publié */, size, sha256, permissions JSONB /*
           extensions */, scan TEXT /* clean|skipped|… */, changelog, reject_reason,
           reviewed_by, created_at, reviewed_at)
assets    (item_id, kind TEXT /* thumbnail|screenshot */, file_key, position)
reports   (id, item_id, version_id, user_id NULL, reason TEXT, details, created_at,
           resolved_by, resolved_at)
audit     (id, actor_id, action TEXT, target TEXT, details JSONB, created_at)
           /* toute action d'administration : rôle changé, bannissement, approbation… */
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

### Rôles

- **admin** : le compte GitHub qui héberge le projet, identifié par son **identifiant
  numérique** (`ADMIN_GITHUB_ID`) — pas par son nom, qui peut changer et être repris par
  quelqu'un d'autre. Le rôle ne peut ni être retiré ni être donné par l'API.
- **moderator** : ajouté (ou retiré) par l'admin, à partir d'un nom GitHub — converti tout
  de suite en identifiant via l'API GitHub. Modère, ne gère pas les rôles.
- **user** : tout compte connecté ; publie et gère ses propres éléments.

### Modération (`moderator`, `admin`)

| | |
|---|---|
| `GET /mod/queue` | versions d'extensions en attente, signalements ouverts |
| `POST /mod/versions/{id}/approve` | **signe** le paquet avec la clé du magasin et le publie |
| `POST /mod/versions/{id}/reject` `{ reason }` | refus motivé (visible par l'auteur) |
| `POST /mod/items/{id}/hide` / `restore` | masquer / rétablir un élément |
| `POST /mod/reports/{id}/resolve` | clore un signalement |
| `GET /mod/versions/{id}/files` / `diff?against=` | fichiers d'un paquet, différence avec la version précédente |

### Administration (`admin`)

| | |
|---|---|
| `GET /admin/users?q=&role=&page=` | comptes (login, avatar, rôle, éléments publiés, dernière activité) |
| `POST /admin/moderators` `{ login }` / `DELETE /admin/moderators/{id}` | ajouter / retirer un modérateur |
| `POST /admin/users/{id}/ban` / `unban` | bannir (jetons révoqués, éléments masqués) / rétablir |
| `GET /admin/audit` | journal des actions d'administration |

## Panneau d'administration (`/admin`)

Petite application Vue (`server/admin/`), construite et servie par Axum. Connexion GitHub
(OAuth web), réservée aux rôles `moderator` et `admin` ; cookie de session `HttpOnly`,
`Secure`, `SameSite=Strict`, propre au panneau (les jetons du CLI et de l'app ne servent pas
ici).

- **File de modération** : versions d'extensions en attente — fiche, permissions (les
  nouvelles en orange), explorateur de fichiers avec coloration, **différence avec la
  version précédente**, résultat de l'analyse ; *Approuver* / *Refuser* (motif obligatoire).
- **Signalements** : élément concerné, motif, historique ; *Masquer* / *Clore*.
- **Éléments** : recherche, masquer / rétablir, versions retirées.
- **Comptes** (admin) : liste, rôles, ajout de modérateurs par nom GitHub, bannissement.
- **Journal** (admin) : qui a fait quoi, quand.

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

## Sécurité des fichiers

Rien n'est jamais envoyé directement dans R2 (pas d'URL d'envoi présignée) : tout fichier
passe par le serveur, qui le **vérifie avant de le stocker**, dans cet ordre.

1. **Validation de structure** (`island-formats`) — la vraie protection : tailles bornées,
   noms simples, chemins sûrs, scène / manifeste valides, chaque image décodée.
2. **Types de fichiers autorisés** dans un paquet d'extension : uniquement des ressources
   web (`html`, `js`, `mjs`, `css`, `json`, `wasm`, images, polices, `txt`, `md`) — tout
   exécutable ou script système (`exe`, `dll`, `ps1`, `bat`, `msi`…) est refusé.
3. **Métadonnées retirées des photos** des scènes (EXIF, dont la position GPS de l'auteur ;
   blocs texte des PNG), sans recompression.
4. **Analyse antivirus ClamAV** (`clamd`, protocole `INSTREAM`), si `CLAMAV_URL` est
   configuré : un fichier détecté est refusé et journalisé. Sans ClamAV (développement), la
   version est marquée `scan: skipped`.
5. **Relecture humaine** des extensions (aucun antivirus ne détecte un code qui abuse de ses
   permissions) — puis signature du magasin.

Côté app, rien ne change : une scène ne contient que des données, et une extension tourne
dans l'isolation habituelle (iframes sandboxées, CSP, permissions consenties).

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

## Stockage R2

Deux compartiments ; le développement (`STORE_ENV=dev`) écrit dans les dossiers suffixés
`-dev` :

```
island-store/      PUBLIC, domaine R2_MEDIA_URL (cdn.streemkit.com)
  media/<id>/thumbnail-<v>.jpg, banner-<v>.jpg, screenshot-<v>-<n>.jpg
  media-dev/
island-packages/   PRIVÉ : ni domaine personnalisé, ni URL r2.dev
  packages/<id>/<v>.islandscene
  packages-dev/
```

- **Versions** : chaque publication crée un fichier `packages/<id>/<v>…` neuf, jamais
  écrasé ; l'historique reste téléchargeable (`/download?version=`). La fiche montre les
  médias de la dernière version.
- **Médias** : la fiche renvoie directement les URL du CDN (cache d'un an, immuable : une
  version = de nouveaux noms) ; les routes `/v1/items/{id}/media/…` y redirigent.
- **Paquets** : `/download` compte le téléchargement puis redirige (303) vers une URL
  **signée de 5 minutes** sur l'API S3 du compartiment privé : sans lien valide, R2 répond
  403 ; masquer un élément coupe tout nouveau lien et les anciens expirent. Le serveur
  refuse de démarrer si les deux compartiments sont le même.
- Jeton R2 : *Object Read & Write*, limité aux deux compartiments. Le jeton de développement a donc
  aussi accès aux dossiers de production : en production, un jeton à part.

## Configuration (variables d'environnement)

| Variable | |
|---|---|
| `DATABASE_URL` | Postgres (Neon) ; en local, un Postgres de développement ou une branche Neon |
| `STORAGE` | `local:./data/files` (dev) ou `r2` |
| `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET` | stockage |
| `R2_MEDIA_URL` | domaine public du compartiment des médias (`https://cdn.streemkit.com`) |
| `R2_PACKAGES_BUCKET` | compartiment privé des paquets (`island-packages`) |
| `STORE_ENV` | `dev` : dossiers `media-dev/`, `packages-dev/` |
| `GITHUB_CLIENT_ID` | OAuth (le *device flow* n'a pas besoin du secret) |
| `GITHUB_CLIENT_SECRET` | OAuth web du panneau d'administration |
| `ADMIN_GITHUB_ID` | identifiant numérique GitHub du super admin |
| `SESSION_KEY` | clé de signature des cookies du panneau |
| `CLAMAV_URL` | `tcp://clamav:3310` (optionnel) |
| `STORE_SIGNING_KEY` | clé privée ed25519 du magasin (PEM) |
| `PUBLIC_URL` | `https://store.island…` |

## Étapes

1. ✅ `crates/island-formats` (formats partagés, tests).
2. ✅ Serveur : squelette Axum, Postgres + migrations, stockage local, **scènes** (catalogue,
   recherche, fiche, téléchargement, publication via jeton, signalement), tests d'API.
3. ✅ Auth GitHub (device flow, jetons ; OAuth web), rôles, admin par `ADMIN_GITHUB_ID`.
4. ✅ Panneau d'administration : comptes, modérateurs, signalements, journal ; médias de la
   fiche (convention `store/`) ; stockage R2 (CDN + liens signés).
5. Écran Magasin dans Island + `island publish` dans le SDK.
6. Extensions : types autorisés, file de modération (fichiers, différences), signature du
   magasin, état `Store` dans l'app, mises à jour.
7. Production : Neon, R2, ClamAV, déploiement, CI.
8. Plus tard : site public, tableau de bord auteur, liens `island://`.
