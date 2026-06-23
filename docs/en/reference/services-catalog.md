# SDK services & permissions

> Catalog of the **backend services** exposed to extensions via `@island/sdk`. For each
> service: the **permission** to declare, the **SDK methods**, the underlying **commands**,
> the **trust level** and the **multi-OS status**. Complements
> [Authoring an extension](/en/reference/authoring).

## Principle: permission = key to the backend

Each OS-specific service is gated by a permission **declared in the manifest**
(`permissions: [...]`). The host checks this declaration **on every call** (`ext_has_permission`,
in addition to the consent given at install — defense in depth). An extension that calls a
service without having declared its permission is **denied** by the host (an error, or a
neutral value for services without a return: volume `-1`, zeroed stats, no-op).

### How the host knows WHO is calling: `extId`

Gated services need the **extension's identity** to look up its manifest. The SDK attaches it
automatically **if you bind your id**:

```ts
const EXT_ID = "com.island.myextension";
const island = useIsland(EXT_ID);   // inside a component
await island.system.stats();        // extId attached automatically → the host checks "system"
```

- Inside `activate(ctx)`, **`ctx` is already bound** to your id: `ctx.capture`, `ctx.media`,
  etc. work without passing anything.
- Inside a **component** (`.vue`), pass your `EXT_ID` to `useIsland(EXT_ID)`.
- `useIsland()` without an id still works for everything that is **not gated** (`view`,
  `drop`, `window`, `notify`, `launcher`, `idle`, `openExternal`…).

## Catalog

| Service | Permission | Trust | OS |
|---|---|---|---|
| Screen capture | `capture` | standard | Windows ✓ |
| Native encoder (recording) | `native-encoder` | ⚠ **high** | multi (binary provided by the ext) |
| System stats | `system` | standard | multi ✓ (sysinfo) |
| Media (SMTC) | `media` | standard | Windows ✓ |
| Applications (launcher) | `apps` | ⚠ **high** | Windows ✓ |
| Native HTTP (cookie-jar) | `network` | standard | multi ✓ (ureq) |
| Clipboard (text + image) | `clipboard` | standard | multi ✓ (arboard) |
| Window awareness | `windows` | ⚠ **high** | Windows ✓ |
| Per-extension storage | `storage` | standard | multi ✓ |
| Per-extension encrypted secrets | — (scoped by id) | standard | multi ✓ (keyring) |
| Current theme (dark/light) | — (read-only) | standard | multi ✓ (front) |
| Global keyboard shortcuts | `shortcuts` | standard | multi ✓ |
| PTY terminals / exec | `terminal` | ⚠⚠ **maximum** | multi ✓ (portable-pty) |

> **Multi-OS**: the command + permission layer is cross-platform; the native implementation
> lives in `services/<svc>/windows.rs` (gated `#[cfg(target_os = "windows")]`). Porting to
> macOS/Linux = adding a `macos.rs`/`linux.rs` fulfilling the same contract, **without
> touching** the commands, the permissions or the SDK.

---

### `capture` — Screen capture & recording

```ts
const island = useIsland(EXT_ID);
const screens = await island.capture.listDisplays();
const png = await island.capture.screenshot({ display: 1, region });
const ok = await island.capture.isRecording();
```

- **Methods**: `listDisplays`, `screenshot`, `startRecording`, `stopRecording`,
  `isRecording`, `selectRegion`, `showRegionOutline`, `pickFolder` (the last 3 are host UI
  helpers, ungated).
- **Commands**: `capture_list_displays`, `capture_screenshot`, `capture_start_recording`,
  `capture_stop_recording`, `capture_is_recording`.
- **Trust**: standard, but it's **screen access** — reserved for extensions that justify it.
  `stopRecording` is not gated (always safe to stop).
- **OS**: Windows (Windows Graphics Capture, anti-cheat safe).

### `native-encoder` — Run the extension's encoder

Recording **delegates encoding to a binary provided by the extension** (typically ffmpeg).
The host stays agnostic: it captures the frames and pipes them; the ext provides the binary +
the encoding arguments.

```ts
await island.capture.fetchBinary({ extId: EXT_ID, url, dest: "binaries/ffmpeg.exe", zipEntry });
await island.capture.startRecording({ ...opts, encoder: { extId: EXT_ID, bin, args, audioArgs } });
```

- **Commands**: `ext_fetch_binary`, `capture_start_recording` (also checks `capture`).
- **Trust**: ⚠ **high** — runs a native program. Safeguard: the binary is **confined to the
  extension's folder** (`resolve_in_ext` forbids `..`, absolute paths, drive letters). An ext
  can therefore only launch ITS OWN files.
- **OS**: cross-platform host logic; the binary is provided by the ext per OS.

### `system` — Stats & sensors

```ts
const sys = useIsland(EXT_ID).system;
await sys.stats();    // { cpu, cores[], memUsed, memTotal }
await sys.battery();  // { percent, charging } | null
await sys.online();   // boolean
await sys.volume();   // { level, muted } | null  (MASTER volume, ≠ media)
await sys.setVolume(0.3); await sys.setMuted(true);
sys.onUserIdle(60_000, onIdle, onActive);  // idle helper (polls idleMs)
```

- **Commands**: `system_stats`, `system_battery`, `system_online`, `system_volume`,
  `system_set_volume`, `system_set_muted`, `system_idle_ms`.
- **Trust**: standard. Stats/battery/network/idle are read-only; the master volume is read
  **and write** (default device output).
- **OS**: `stats` cross-platform (`sysinfo`); sensors Windows (`GetSystemPowerStatus`,
  `InternetGetConnectedState`, `GetLastInputInfo`, WASAPI `IAudioEndpointVolume`).

### `media` — Native media control

```ts
const island = useIsland(EXT_ID);
island.media.toggle(); island.media.next(); island.media.setVolume(0.5);
const m = island.media.state; // reactive (title/artist/playback) — FREE TO READ, ungated
```

- **Methods**: `toggle`, `next`, `prev`, `seek`, `setVolume` (+ reactive `media.state`,
  ungated: just an event stream).
- **Commands**: `media_toggle`, `media_next`, `media_prev`, `media_seek`, `media_get_volume`,
  `media_set_volume`.
- **Trust**: standard. Without the permission: no-op (volume read = `-1`).
- **OS**: Windows (SMTC + WASAPI volume).

### `apps` — Launch applications & search files

```ts
const apps  = await island.invoke("list_apps",    { extId: EXT_ID });               // Win32 + UWP + Steam
await         island.invoke("launch_path",  { extId: EXT_ID, path });               // ShellExecute (open)
await         island.invoke("launch_admin", { extId: EXT_ID, path });               // UAC elevation (runas)
const icons = await island.invoke("app_icons",    { extId: EXT_ID, paths });         // PNG icons (data-URL)
const files = await island.invoke("search_files", { extId: EXT_ID, query, roots, limit }); // [{ name, path, isDir }]
const hasEverything = await island.invoke("files_engine", { extId: EXT_ID });        // Everything detected?
```

- **Commands**: `list_apps`, `launch_path`, `launch_admin`, `app_icons`, `search_files`, `files_engine`.
- **`list_apps`**: **Win32** apps (Start Menu) **+ UWP/Store + Steam games** (shell *AppsFolder* enumeration + Steam manifests).
- **`search_files`**: file/folder search, **hybrid engine** — *Everything* (voidtools) if running → instant whole-disk; otherwise a **home-grown index** of the given `roots` (default: Desktop / Documents / Downloads).
- **Trust**: ⚠ **high** — can launch installed programs (and **elevated** via `launch_admin`).
- **OS**: Windows.

### `network` — Native HTTP with a cookie-jar

Consumes a third-party API with a **cookie session**, free of a browser `fetch`'s CORS/SameSite
restrictions (the request is native, outside the webview). One cookie-jar **per extension**,
**persisted on disk** (the session survives restarts).

```ts
const r = await useIsland().http.request({ extId: EXT_ID, url, method, body, headers });
```

- **Command**: `http_fetch`.
- **Trust**: standard, but the ext provides the **full URL** → an SSRF surface. The permission
  + consent bound the capability.
- **OS**: cross-platform (`ureq`). `http.request` takes the `extId` **explicitly**.

### `clipboard` — Clipboard (text + image)

```ts
const island = useIsland(EXT_ID);
await island.clipboard.writeText("copied!");
const txt = await island.clipboard.readText();
const png = await island.clipboard.readImage();   // PNG data URL, or null
await island.clipboard.writeImage(canvas.toDataURL());
```

- **Commands**: `clipboard_read_text`, `clipboard_write_text`, `clipboard_read_image`,
  `clipboard_write_image`.
- **Trust**: standard, gated by `clipboard` (reading the clipboard exposes potentially
  sensitive data).
- **OS**: cross-platform (`arboard`).

### `storage` — Per-extension persistent key→value

```ts
await ctx.storage.set("key", value);
const v = await ctx.storage.get("key", fallback);
```

- **Commands**: `storage_get`, `storage_set`, `storage_delete`, `storage_keys`.
- **Trust**: standard. **Scoped by id**: 1 JSON file per extension, an ext never reads
  another's store.
- **OS**: cross-platform (`std::fs`).

### `secrets` — Per-extension encrypted storage

```ts
await ctx.secrets.set("apiToken", token);
const token = await ctx.secrets.get("apiToken"); // string | null
await ctx.secrets.delete("apiToken");
```

- **Commands**: `secret_get`, `secret_set`, `secret_delete`.
- **Trust**: standard. **Scoped by id** (no dedicated permission); values ENCRYPTED in the
  system vault (vs plaintext `storage`). Reserve for sensitive data (tokens, passwords).
- **OS**: Windows (Credential Manager via `keyring`); Keychain/Secret Service when ported.

### `theme` — Current theme (dark/light)

```ts
const island = useIsland();          // ungated
island.theme.current();              // "dark" | "light"
const off = island.theme.onChange((t) => redraw(t));
```

- **Trust**: standard, read-only (no permission). Pure front-end (observes the
  `documentElement` class). Useful for canvas/SVG rendering that doesn't follow CSS tokens.
- **OS**: cross-platform.

### `shortcuts` — Global keyboard shortcuts

```ts
const ok = await ctx.shortcuts.register("Ctrl+Shift+Space", () => { /* … */ });
await ctx.shortcuts.unregister("Ctrl+Shift+Space");
```

- **Trust**: standard. **OS**: cross-platform (global-shortcut plugin).

---

### `terminal` — PTY terminals & exec

```ts
// Interactive terminal (wire to xterm.js):
const id = await ctx.terminal.spawn({ cwd: "C:/dev/project", cols: 80, rows: 24 });
const off = await ctx.terminal.onData(({ id: i, b64 }) => { if (i === id) term.write(atob(b64)); });
term.onData((d) => ctx.terminal.write(id, d));   // keystrokes → stdin
ctx.terminal.resize(id, cols, rows);             // on resize / xterm fit
ctx.terminal.kill(id);                           // kill the process
await ctx.terminal.onExit(({ id }) => { /* process finished */ });

// Captured one-shot command (git, docker…):
const { code, stdout, stderr } = await ctx.terminal.exec({ cmd: "git", args: ["-C", path, "status"] });
```

- **Methods**: `spawn(opts)→id`, `write(id,data)`, `resize(id,cols,rows)`, `kill(id)`,
  `exec({cmd,args?,cwd?})→{code,stdout,stderr}`, `onData(cb)`, `onExit(cb)`. PTY output arrives
  as **base64** (binary/ANSI safe) via `onData`; decode it for xterm.
- **Trust**: ⚠⚠ **MAXIMUM** — runs **arbitrary processes** (equivalent to code execution).
  Only grant to **trusted** extensions (dev tools). Shown prominently at install.
- **OS**: cross-platform (crate `portable-pty` → ConPTY on Windows).

---

## Recap "which permission for which need"

- Read the screen / record → **`capture`** (+ **`native-encoder`** if you bundle an encoder
  for recording).
- CPU/RAM → **`system`**.
- Media play/pause/volume → **`media`**.
- List/launch apps → **`apps`** (⚠).
- Call a web API with a cookie session → **`network`**.
- Persist settings → **`storage`**.
- Global keyboard shortcut → **`shortcuts`**.
- Run processes / interactive terminal → **`terminal`** (⚠⚠ maximum trust).

Purely-UI contributions (`launcher`, `idle`, `view`, `drop`, `window`, `notify`) require **no**
backend permission.
