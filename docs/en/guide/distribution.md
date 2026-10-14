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

- drag the `.island` file onto the notch;
- double-click the file (the association is created on the first install);
- go through **Settings › Extensions › Install…**.

A sheet opens in the notch: name, author, signature status, permissions in plain language,
and what the extension adds. Once accepted, Island verifies the package again, installs it
in one go and records the consented permissions.

## Update

Same path as an install, with a newer version and the same `id`: the sheet shows the
installed version and highlights the **new** permissions. The extension's storage and
secrets are kept.

## Uninstall

Settings › Extensions › trash can (two clicks). Island removes the extension's folder,
permissions, storage and secrets.

## Limits

- At most 2,000 files and 64 MB uncompressed.
- Relative paths without `..`; every file referenced by the manifest must be present.
