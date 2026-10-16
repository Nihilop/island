# Permissions & security

## Permissions

| Permission | Grants access to |
|---|---|
| *(none)* | its storage, its settings, its translations, the theme, `openUrl`, every slot |
| `secrets` | `island.secrets` (Windows credential store, private to the extension) |
| `network:<domain>` | `island.http` to that domain; `network:*.exemple.com` for its subdomains |
| `notifications` | `island.notify` |
| `clipboard` | `island.clipboard` |
| `cookies` | cookie sessions on its `network:` domains (cookies kept by Island, never visible) |
| `windows` | `island.windows.open`: its own windows (`contributes.windows`) |
| `capture` | **high risk** (shown in red at install): `island.capture`, see and record the screen and sound |
| `images:<domain>` | show HTTPS images from that domain (`<img>`, CSS) without `island.http` |

Ask for the minimum: the user sees the list in plain language before installing, and an
update that adds a permission highlights it in orange.

**Never accessible**: the Bitwarden vault, Windows Hello, AI keys, Discord, Island's
settings, other extensions.

## The isolation model

- **Sandboxed iframes.** Each extension page is loaded in a `sandbox="allow-scripts"` iframe
  (opaque origin) and served with a CSP that enforces the isolation itself
  (`sandbox allow-scripts`): even opened outside the island, it stays isolated.
- **No access to the app.** An opaque-origin page cannot call Island's commands (Tauri
  rejects its request). Tauri or WebView2 internal objects visible in the iframe are
  inert — verified by the test extension `examples/extensions/isolation`.
- **No direct network, no other extensions' files.** The CSP forbids `fetch` and only allows
  loading the extension's own files; the one exception is images from consented `images:`
  domains (display only: no script, no cookie).
- **Forms.** A page's JavaScript receives the `submit` event (`allow-forms`), but a form is
  never sent anywhere (`form-action 'none'`).
- **Identity set by Island.** Island creates each iframe's channel and knows which extension
  is talking; the extension's code cannot impersonate another one.
- **Rust-side checks** wherever external data comes in:
  - network: allowed domain re-read from the installed manifest, HTTPS, no redirects,
    private and local addresses refused at connection time;
  - files: canonicalized paths, confined to the extension folder.

## Consented permissions

At install time, Island records the permissions the user accepted
(`extensions-v2-state/<id>.json`, outside the extension folder). **Those are what count**,
not the manifest: an extension copied by hand without going through installation has no
permissions. In developer mode, the manifest is authoritative.
