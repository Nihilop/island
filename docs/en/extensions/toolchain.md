# Extension — Toolchain (spec)

A dev cockpit in the island: open IDEs, manage Git, and launch/supervise the processes
(`pnpm dev`, `artisan serve`…) of the **current project** — with real terminals.

Everything lives in **one view** with a **tab system** (at the component level):
**IDE · Git · Process**.

## Host prerequisites (new)
- **`pty` service** (in progress): interactive terminals (ConPTY/portable-pty) + one-shot
  `exec` (captures stdout). Permission **`terminal`** (max trust). → processes & git.
- **VSCode companion extension**: pushes the **project context** (path, scripts, open
  files/terminals) via `island.serve`, and receives commands (open, run a task). It's the
  only reliable way to know "which project/IDE is focused" among several windows. Light
  fallback: parse the **foreground window title** (`island.windows`) for the focused project
  name.

## Tab 1 — IDE launcher
- List of projects; click → opens the IDE: `code <path>` (Windows) **or** `code` on the
  **WSL** side (`code` from `\\wsl$\…` / `wsl code .`). **Handle both cases** (Windows path vs
  WSL distro) — detect via the path.
- **Green dot when the IDE is open** on that project → via **`island.windows`** (window
  presence: title contains the project name).
- Launch via `pty.exec` (or `apps`/ShellExecute for `code`).

## Tab 2 — Git manager
- Projects in **collapsibles**; in each: **10 latest branches**, the **active branch**
  highlighted, **select** a branch (checkout).
- Actions: **add + commit + push**, **create a PR** (gh CLI or GitHub API).
- **Per-line diff** shown (style `+582 −54`) → `git diff --numstat` / `--shortstat`.
- Structured data via `pty.exec`: `git branch`, `git status`, `git diff --numstat`,
  `git log`. (No PTY here — parsed one-shot capture.)

## Tab 3 — Process launcher (per project)
- **Automatic analysis of the project's `package.json`** → offers the **scripts** (dev,
  build, test…) as buttons. (Composer/artisan possible too.)
- A launched process is **physically visible**: in **idle** (icon/counter via `idle.action`)
  AND in the view → quick access. Terminal icons that **increment** with the number of open
  processes/terminals.
- Click → opens the process in an **`island.window()`**: **xterm.js** (perfect PTY output) +
  control commands **restart / stop / kill**.

### Process window UI detail (key idea)
- The **control commands are detached** from the window: a small **"container" just below the
  window**, offset by a few px, that **follows the window** as it moves (drag) → floating look.
- The **window is resizable** → xterm `fit` on resize → `pty.resize`.
- (Implies extending `island.window`: real `resizable` support + a detached `footer` slot that
  follows the window. To be specified host-side.)

## Permissions
`terminal` (pty/exec), `windows` (IDE presence), `apps` (open `code`), `network` (create PR
via API), `storage` (saved projects). `serve` if a VSCode companion.

## Build order
1. **Host `pty` service** (+ `exec`, `terminal` permission) ← we start here.
2. **Process** tab (xterm + window + restart/stop/kill) = the "wow".
3. **Git** tab (exec + diff + PR).
4. **IDE** tab + **VSCode companion** (project context + presence).
5. Window polish: resize + detached control footer that follows.

## To decide
- `island.window`: add `resizable` + a detached following footer (host change).
- Project context: VSCode companion (rich) vs title parsing (light) — probably both (title as
  fallback).
- PR: `gh` CLI (via `exec`) vs GitHub API (via `http` + `secrets` token).
