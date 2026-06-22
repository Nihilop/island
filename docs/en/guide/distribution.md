# 7. Distribution

## The `.island` format

An `.island` is a **zip archive** containing `manifest.json` + `dist/` (without the source
code). That's all the host needs to install and load an extension.

## Packaging

Two ways:

- **In the app** (recommended): Settings → Extensions → **"Pack"** button (visible on
  extensions marked *dev*, i.e. that have a `package.json`) → choose the destination → an
  `.island` is created. The app **does not compile**: it zips the `dist/` you already built.
  Build first (`pnpm build`).
- **Manually**: zip `manifest.json` + `dist/` at the root of the archive, with an `.island`
  extension.

## Installing

- **Double-click** an `.island` (if the file association is enabled) → an install modal
  opens: terms → requested permissions (translated, the ⚠ levels highlighted) → progress →
  the extension is copied to the extensions folder and enabled.
- From the app: the same modal can be triggered via **Settings → Extensions → Browse…**.

## File association

On first install, Island associates `.island` files with the app (`HKCU` key, per-user, no
admin) → double-click works. An **"Associate .island files"** button is also available in
Settings.

## Checklist before publishing

- [ ] `manifest.json` `id` = folder name (reverse-DNS).
- [ ] `vite.config.ts` externalizes `["vue", "@island/sdk"]`.
- [ ] `tailwind.css`: `theme(reference)` + `@source not "./dist"` + `@theme inline`.
- [ ] The manifest `permissions` = **exactly** the backend services used (no more, no less) —
      each service is checked on call.
- [ ] Components calling a gated service use `useIsland(EXT_ID)`.
- [ ] Any native binary is **downloaded at runtime** (not in the `.island`), `binaries/` is
      gitignored.
- [ ] `pnpm build` up to date → `dist/index.mjs` + `dist/style.css` present.
- [ ] Tested enabling/disabling at runtime (Settings → Extensions) with no console error.

## Next

The type reference remains `src/sdk/island.ts` (source of truth for the API).
