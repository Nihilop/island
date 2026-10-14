# The SDK

`@island/sdk` (the `packages/sdk` folder in the repository) is an extension's only gateway to
Island. It is used the same way in the logic (`main`) and in views.

```ts
import { connect } from "@island/sdk";
const island = await connect();
```

`connect()` waits for Island's handshake, then returns the `island` object. Every method
returns a promise; a missing permission rejects with `permission:<name>`.

## Identity, language, theme

| | |
|---|---|
| `island.id` | Extension identifier (provided by Island, not declared by the code) |
| `island.locale` | Island's language at startup (`fr`, `en`…) |
| `island.theme` | `"dark"` or `"light"`; the SDK sets the `light` class on `<html>` |
| `island.onTheme(cb)` | Live theme changes |
| `island.t(key, params?)` | Translated text (`locales` files), `{param}` replaced |
| `island.log(...args)` | Logs to Island's development console |

## Data

| | Permission |
|---|---|
| `island.storage.get(key)` / `set(key, value)` | none — JSON storage private to the extension |
| `island.secrets.get(key)` / `set(key, value)` / `delete(key)` | `secrets` — Windows credential store, private to the extension |
| `island.settings.get(key)` | none — value of a declared setting (manifest default otherwise) |
| `island.settings.onChange(cb)` | none — called with the changed key |

## Network

```ts
const r = await island.http.request({ url: "https://api.github.com/user", method: "GET", headers: { Authorization: `Bearer ${token}` } });
// r = { status, headers: [name, value][], body: string }

const user = await island.http.json<{ login: string }>("https://api.github.com/user", { headers });
// throws HttpError(status, body) if status ≥ 400
```

Permission `network:<domain>` (`network:*.exemple.com` for subdomains). **HTTPS only**, no
redirects followed, response capped at 5 MB, local and private addresses refused.

## Notch

| | Permission |
|---|---|
| `island.live.set(id, spec \| null)` | none — [live activity](./emplacements#live-activity) |
| `island.launcher.provide(id, search)` | none — [launcher results](./emplacements#launcher) |
| `island.launcher.onRun(id, action)` | none — result picked |
| `island.commands.on(id, action)` | none — [command](./emplacements#commands) triggered |
| `island.notify({ title, body?, banner? })` | `notifications` |
| `island.openUrl(url)` | none — http(s) only, default browser |
| `island.clipboard.readText()` / `writeText(text)` | `clipboard` |

## UI kit

```ts
import "@island/sdk/ui.css";
```

The notch's colors and typography (variables `--label`, `--label-2`, `--fill-1`…, `--green`,
`--orange`, `--red`, `--blue`), a transparent background, and a few classes: `.list`, `.item`,
`.btn` (`.primary`), `.badge` (`.green`, `.orange`…), `.seg` (tabs), `.empty`, `.caption`,
`.muted`, `.truncate`. The light theme is handled automatically.
