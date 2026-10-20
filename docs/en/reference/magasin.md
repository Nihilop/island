# The store

The Island store distributes **live wallpapers** (`.islandscene` scenes) and **extensions**
(`.island`). The app reaches it through its Store window; authors publish from the app or
with the SDK CLI (`island publish`, see [Distribution](/en/guide/distribution)).

- **Scenes** go live right away, once the file is checked.
- **Extensions** go through **human review**: moderators read the code and the permissions,
  then the store **signs them with its own key**. Island then installs them as “Verified by
  the store”.
- Installed items update themselves: checked at startup, then daily.

## Host your own store

The server is open source, like the app: a fork of Island can have its own store. The full
guide is [`server/INSTALL.md`](https://github.com/Nihilop/island/blob/main/server/INSTALL.md).
In short:

| Piece | Role | Example |
|---|---|---|
| Server (Docker image) | API and the `/admin` moderation panel | `ghcr.io/<you>/island-store` |
| Postgres | accounts, catalog, versions, audit log | the compose Postgres, or Neon |
| Cloudflare R2 | files: **public** media (CDN), **private** packages (signed links) | two buckets |
| GitHub OAuth App | sign-in for authors and moderators | *Enable Device Flow* ticked |
| Store key | signs approved extensions | `island keygen` |

1. **Files**: a public R2 bucket connected to a domain (media), a second **private** one with
   no domain (packages), and an *Object Read & Write* token scoped to both.
2. **Store key**: `node packages/sdk/bin/island.mjs keygen secrets/store-signing-key.json`.
   Keep it **out of the repository**, and back it up.
3. **Server**: the `server/deploy/` folder holds a ready-made `docker-compose.yml` (optional
   profiles: Cloudflare Tunnel, ClamAV, local Postgres) and a commented `.env.example`. Run
   `docker compose up -d`; `/health` should answer `ok`.
4. **App**: the store address and key are **build settings**. In your fork, open *Settings ›
   Secrets and variables › Actions › Variables*:
   - `ISLAND_STORE_URL`: the server's public address;
   - `ISLAND_STORE_KEYS`: its public key, printed at server startup and returned by
     `GET /v1/store/status` (`signingKey`).

   For a local build, put the same lines in `.env` at the repository root (see
   `.env.example`).

## Security

- **Never commit** `.env`, `secrets/` or a private key: the repository ignores them, keep it
  that way. The Docker image holds no secret; the `.env` and the key are mounted at run time.
- **Use a separate production R2 token**: the development one also reaches the production
  folders.
- **The store key is precious**: Island only trusts keys compiled into the app. Losing it
  means shipping a new Island release with a new key.
- If a wave of uploads cannot be moderated, the panel offers **Suspend publishing**: the
  catalog and downloads keep working, and nothing new is accepted.
