# Installing the Island store server

The store server hosts the Island catalog: live-wallpaper scenes today, extensions later.
It is a single Rust binary (Axum). It needs these services:

| Service | Used for | Required |
|---|---|---|
| PostgreSQL 15+ | accounts, catalog, versions, reports, audit log | yes |
| Cloudflare R2 (S3 API) | published files: public media, private packages | yes in production (a local folder works for development) |
| GitHub OAuth App | sign-in from Island (device flow) and the admin panel | yes, to publish or moderate |
| ClamAV (`clamd`) | antivirus scan of uploaded files | recommended |

The server keeps no state of its own, so you can redeploy or scale it freely. All state
lives in Postgres and R2. Database migrations are embedded in the binary and run on
startup.

---

## 1. Run it locally (development)

Requirements: Rust (stable), Node 22 with pnpm, Docker.

```bash
cd server
docker compose up -d                  # Postgres 17 on localhost:5433
cp .env.example .env                  # then fill it in (see section 3)
cd admin && pnpm install && pnpm build && cd ..
cargo run                             # http://127.0.0.1:8080, admin panel on /admin
```

`server/.env` is read at startup and is git-ignored, so never commit it. Without R2 settings,
files are stored in `./data/files` (`STORAGE=local:./data/files`).

Tests need the development Postgres (`docker compose up -d`):

```bash
DATABASE_URL=postgres://island:island@localhost:5433/island_store cargo test
```

Each test runs in its own temporary database.

---

## 2. Production image

The repository ships a Dockerfile. It builds the admin panel and the server, and produces a
small Debian image that runs as a non-root user. Build it **from the repository root**,
because the server depends on `crates/island-formats`:

```bash
docker build -f server/Dockerfile -t island-store .
docker run --env-file server/.env -p 8080:8080 island-store
```

The image listens on `0.0.0.0:8080` (`BIND`) and serves the admin panel from
`/app/admin/dist` (`ADMIN_DIST`).

---

## 3. Configuration

All settings are environment variables. `server/.env.example` lists them with comments.

| Variable | Example | |
|---|---|---|
| `DATABASE_URL` | `postgres://user:pass@host/island_store?sslmode=require` | Postgres connection string |
| `BIND` | `0.0.0.0:8080` | listen address |
| `PUBLIC_URL` | `https://store.example.com` | public address of the server; `https://` turns on secure cookies |
| `SESSION_KEY` | 48+ random characters | signs admin-panel cookies; if missing, a random key is used and sessions are lost on restart |
| `GITHUB_CLIENT_ID` / `GITHUB_CLIENT_SECRET` | | GitHub OAuth App (section 5) |
| `ADMIN_GITHUB_ID` | `40236901` | **numeric** GitHub id of the super admin |
| `STORAGE` | `r2` or `local:./data/files` | where published files go |
| `STORE_ENV` | `dev` | writes to `media-dev/` and `packages-dev/` instead of `media/` and `packages/` |
| `R2_ACCOUNT_ID` | | Cloudflare account id |
| `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` | | R2 API token (S3 credentials) |
| `R2_BUCKET` | `island-store` | **public** bucket for media |
| `R2_MEDIA_URL` | `https://cdn.example.com` | custom domain of the public bucket |
| `R2_PACKAGES_BUCKET` | `island-packages` | **private** bucket for packages |
| `STORE_SIGNING_KEY` | `/run/secrets/store-key.json` | store key (ed25519) that signs approved extensions: a file made by `island keygen`, its content, or a PKCS#8 PEM |
| `CLAMAV_URL` | `clamav:3310` | `host:port` of `clamd`; if unset, files are not scanned and are marked `skipped` |
| `TRUST_PROXY` | `1` | read the client IP from `X-Forwarded-For`; enable **only** behind a reverse proxy |
| `ADMIN_DIST` | `admin/dist` | built admin panel |
| `RUST_LOG` | `island_store=info,tower_http=info` | log filter |

Generate a session key:

```bash
openssl rand -base64 48
```

Find a numeric GitHub id:

```bash
curl -s https://api.github.com/users/<login> | grep '"id"'
```

---

## 4. Cloudflare R2

The server uses **two buckets**:

```
<R2_BUCKET>            PUBLIC, custom domain R2_MEDIA_URL
  media/<id>/thumbnail-<v>.jpg, banner-<v>.jpg, screenshot-<v>-<n>.jpg
<R2_PACKAGES_BUCKET>   PRIVATE: no custom domain, r2.dev URL disabled
  packages/<id>/<v>.islandscene
```

- **Media** are re-encoded JPEGs, and the API returns their CDN URLs directly. Every
  version uses new file names, so they are cached for a year (`immutable`).
- **Packages** are never public. `GET /v1/items/{id}/download` counts the download, then
  redirects (303) to a pre-signed URL that is valid for **5 minutes**. Hiding an item stops
  new links from being issued, and existing links expire.
- Each published version is a new file and is never overwritten, so older versions stay
  downloadable with `?version=`.
- The server refuses to start if both variables name the same bucket.

Setup:

1. Create the public bucket. In *Settings › Custom Domains*, connect your domain, with a
   minimum TLS version of 1.2.
2. Create the private bucket. Keep it private: no custom domain, and the *Public Development
   URL* left disabled.
3. In *R2 › Manage API tokens*, create an **Object Read & Write** token scoped to **both**
   buckets. Copy the Access Key ID and the Secret Access Key: the secret is shown only once.
4. Use a separate token in production. A token shared with development can also reach
   production folders.

No CORS rules are needed: Island downloads files natively, not from a browser page.

---

## 5. Store signing key

Extensions are reviewed by moderators before they are published. On approval, the server
**replaces** the author's signature with the store's, so Island shows them as
“verified by the store”.

```bash
node packages/sdk/bin/island.mjs keygen store-key.json   # keep it OUT of the repository
```

- Give the file to the server through `STORE_SIGNING_KEY`, as a secret.
- The public key is printed at startup and exposed by `GET /v1/store/status` (`signingKey`).
  Island must list it among its trusted store keys.
- Back the key up. Losing it means publishing a new key in an Island update.
- Without it, reviewing works but approving fails with `noSigningKey`.
- Extension ids starting with `dev.island.` are reserved for the admin (official
  extensions).

## 6. GitHub OAuth App

In GitHub, open *Settings › Developer settings › OAuth Apps › New OAuth App*:

- **Homepage URL**: your `PUBLIC_URL`.
- **Authorization callback URL**: `<PUBLIC_URL>/admin/callback`.
- Tick **Enable Device Flow**. The Island app signs in with it.

Copy the Client ID, then generate a client secret. Use one OAuth App per environment
(development and production), because the callback URL differs.

Roles:

- `ADMIN_GITHUB_ID` is the super admin. This account cannot be demoted or banned.
- The admin adds moderators in the panel by GitHub login. The login is converted to the
  numeric id, so later renames do not matter.

---

## 7. Behind a reverse proxy or tunnel

- Set `PUBLIC_URL` to the public `https://` address, and `TRUST_PROXY=1`.
- Do **not** expose the server port directly when `TRUST_PROXY=1`: clients could forge
  their IP address. Bind to a private network instead, for example a Docker network or
  `127.0.0.1`.
- The upload limit is 256 MB. Some proxies cap request bodies lower; Cloudflare's free plan,
  for example, caps them at 100 MB.

---

## 8. Operations

- **Health check**: `GET /health` returns `ok`.
- **Emergency stop for uploads**: moderators can pause all publishing from the admin panel
  (*Suspend publishing*). Uploads and listing edits are then rejected with
  `503 publishingPaused`. The catalog, downloads and reports keep working.
  `GET /v1/store/status` exposes the current state.
- **Development tokens**, without GitHub:
  `island-store token <login> [--role user|moderator|admin] [--name <label>]`.
- **Backups**: back up Postgres (the R2 file keys are stored there) and both buckets.
