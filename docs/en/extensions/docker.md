# Extension — Docker (planned)

A glanceable Docker control center in the island. Ambient status + actions.

## Goal
See container state without alt-tabbing, and act: list / start / stop / restart / kill /
logs / stats. Control **existing compose projects** (up/down/restart of a group) — not
reimplement `docker compose up` (build/depends_on): for that, run the CLI via the `pty`
service (see toolchain).

## Island slots
- **idle**: dot/badge = number of running containers (red if a key service is down).
- **view**: list of containers (name, image, status, ports) + CPU/RAM (poll stats).
- **window**: streamed logs of a container (xterm or text).
- **notify**: a container dies / restarts.
- **launcher**: quick actions (restart `<service>`, up/down a project).

## Accessing Docker
- **MVP (no host change)**: enable `tcp://localhost:2375` in Docker Desktop → `island.http`
  hits the **Engine API** (`GET /containers/json`, `POST /containers/{id}/start|stop|restart`,
  `GET /containers/{id}/logs?tail=`, `GET /containers/{id}/stats`).
- **Clean (host `docker` service)**: talk to the **named pipe** `//./pipe/docker_engine`
  (no insecure TCP toggle) + streaming logs/stats. ~150 lines of Rust. To evaluate after the
  MVP.

## Compose
Containers carry the `com.docker.compose.project` label → filter on it = up/down/restart a
whole project. The initial `up` (creation) = `docker compose up -d` via `pty`.

## Permission
`network` (MVP http); a possible host `docker` service would have its own permission.

## Open questions
- Exact view format (dense table vs cards).
- Logs: streaming (host service) vs `?tail=` poll (http).
- Multi-context Docker (WSL2 vs Windows engine).
