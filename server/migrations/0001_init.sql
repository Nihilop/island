-- Magasin d'Island : schéma initial (cf. server/README.md, « Modèle de données »).

-- Recherche insensible aux accents (« luciole » trouve « Lucioles », « ete » trouve « Été »).
CREATE EXTENSION IF NOT EXISTS unaccent;

-- `unaccent` n'est pas IMMUTABLE (dictionnaire modifiable) : enveloppe figée pour pouvoir
-- l'utiliser dans une colonne générée.
CREATE FUNCTION island_unaccent(text) RETURNS text
    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
    AS $$ SELECT public.unaccent('public.unaccent'::regdictionary, $1) $$;

CREATE TABLE users (
    id           BIGSERIAL PRIMARY KEY,
    -- NULL : compte créé par la commande de développement (pas encore de GitHub).
    github_id    BIGINT UNIQUE,
    login        TEXT NOT NULL UNIQUE,
    avatar_url   TEXT,
    role         TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'moderator', 'admin')),
    banned       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ
);

CREATE TABLE tokens (
    id           BIGSERIAL PRIMARY KEY,
    user_id      BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    -- SHA-256 du jeton : le jeton lui-même n'est jamais stocké.
    hash         TEXT NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

CREATE TABLE items (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('scene', 'extension')),
    owner_id    BIGINT NOT NULL REFERENCES users (id),
    name        TEXT NOT NULL,
    summary     TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    tags        JSONB NOT NULL DEFAULT '[]',
    status      TEXT NOT NULL DEFAULT 'published' CHECK (status IN ('published', 'hidden', 'removed')),
    downloads   BIGINT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    search      TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('french', island_unaccent(name)), 'A') ||
        setweight(to_tsvector('french', island_unaccent(summary)), 'B') ||
        setweight(to_tsvector('french', island_unaccent(description)), 'C')
    ) STORED
);
CREATE INDEX items_search ON items USING GIN (search);
CREATE INDEX items_recent ON items (kind, status, updated_at DESC);
CREATE INDEX items_popular ON items (kind, status, downloads DESC);
CREATE INDEX items_owner ON items (owner_id);

CREATE TABLE versions (
    id            BIGSERIAL PRIMARY KEY,
    item_id       TEXT NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    version       TEXT NOT NULL,
    status        TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'yanked')),
    file_key      TEXT NOT NULL,
    size          BIGINT NOT NULL,
    sha256        TEXT NOT NULL,
    permissions   JSONB,
    scan          TEXT NOT NULL,
    changelog     TEXT NOT NULL DEFAULT '',
    reject_reason TEXT,
    reviewed_by   BIGINT REFERENCES users (id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at   TIMESTAMPTZ,
    UNIQUE (item_id, version)
);

CREATE TABLE assets (
    item_id  TEXT NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    kind     TEXT NOT NULL CHECK (kind IN ('thumbnail', 'screenshot')),
    position INT NOT NULL DEFAULT 0,
    file_key TEXT NOT NULL,
    PRIMARY KEY (item_id, kind, position)
);

CREATE TABLE reports (
    id          BIGSERIAL PRIMARY KEY,
    item_id     TEXT NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    version_id  BIGINT REFERENCES versions (id) ON DELETE SET NULL,
    user_id     BIGINT REFERENCES users (id) ON DELETE SET NULL,
    -- Empreinte de l'IP (limitation des signalements anonymes), jamais l'IP elle-même.
    ip_hash     TEXT,
    reason      TEXT NOT NULL CHECK (reason IN ('illegal', 'malware', 'copyright', 'offensive', 'broken', 'other')),
    details     TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_by BIGINT REFERENCES users (id),
    resolved_at TIMESTAMPTZ
);
CREATE INDEX reports_open ON reports (created_at) WHERE resolved_at IS NULL;

CREATE TABLE audit (
    id         BIGSERIAL PRIMARY KEY,
    actor_id   BIGINT REFERENCES users (id),
    action     TEXT NOT NULL,
    target     TEXT NOT NULL,
    details    JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
