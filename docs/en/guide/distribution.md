# Distribution

An extension ships as an **`.island` package**: a zip containing the manifest, the built
files (`dist/`, `icons/`, `locales/`, `assets/`) and a signature.

## Package

The `packages/sdk/bin/island.mjs` tool only requires Node:

```bash
node packages/sdk/bin/island.mjs pack examples/extensions/github --out github.island
```

Without a key, the package is **not signed**: it can only be installed in developer mode,
after a warning.

## Sign

```bash
node packages/sdk/bin/island.mjs keygen ma-cle.json
node packages/sdk/bin/island.mjs pack . --key ma-cle.json
```

- The signature (ed25519) covers the hash of every file in the package: a package modified
  after signing is rejected, even in developer mode.
- **Keep the private key secret and backed up**: without it, no more signed updates.
- Only keys recognized by Island grant the "Signed by Island" status. A package signed with
  another key is treated as unofficial (developer mode).

## Install

The user can:

- drag the `.island` file onto the island;
- double-click the file (the association is created on the first install);
- go through **Settings › Extensions › Install…**.

A sheet opens in the island: name, author, signature status, permissions in plain language,
and what the extension adds. Once accepted, Island verifies the package again, installs it
in one go and records the consented permissions.

## Update

Same path as an install, with a newer version and the same `id`: the sheet shows the
installed version and highlights the **new** permissions. The extension's storage and
secrets are kept.

## Uninstall

Settings › Extensions › trash can (two clicks). Island removes the extension's folder,
permissions, storage and secrets.

## Publish to the store

The same tool publishes to the Island store. It accepts **live-wallpaper scenes**
(`.islandscene`, exported from the editor with “Share”) and **extensions** (`.island`).

```bash
node packages/sdk/bin/island.mjs login      # GitHub sign-in: enter a code on github.com
node packages/sdk/bin/island.mjs whoami
node packages/sdk/bin/island.mjs publish aurora.islandscene --summary "Northern dawn" --tags night,sky
node packages/sdk/bin/island.mjs publish aurora.islandscene --changelog "Finer stars"
node packages/sdk/bin/island.mjs logout
```

- The first publish creates an item. Publishing **the same file** again sends a **new
  version** of that item; `--new` creates another item, and `--item <id>` targets a given
  item.
- The summary, tags and links can also come from the package (manifest and `README`).
  Command options take precedence: `--summary`, `--description`, `--tags`, `--homepage`,
  `--repository`, `--license`, `--changelog`.
- An **extension** (`.island`) goes to **review**: moderators read its code and
  permissions, then publish it signed by the store. Its id is the manifest's, and its
  version must increase. In developer mode, Settings › Extensions also offers “Package”
  and “Publish” (with a window for the listing).
- The store token is kept in `~/.island/credentials.json`, readable only by you. `logout`
  revokes it on the store.
- `--store <url>` (or the `ISLAND_STORE_URL` variable) targets another store, for example
  a development server: `--store http://127.0.0.1:8080`.
- If moderators have paused publishing, the command says so and sends nothing. Try again
  later.

In the app, the wallpaper editor's **Publish** button does the same without going through
a file.

## Limits

- At most 2,000 files and 64 MB uncompressed.
- Relative paths without `..`; every file referenced by the manifest must be present.
