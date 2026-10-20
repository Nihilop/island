-- Réglages du magasin modifiables à chaud depuis le panneau (une ligne par réglage).
-- `publishing` : interrupteur d'urgence des publications (vague d'envois impossible à
-- modérer) : fermé, plus aucun envoi ni modification de fiche n'est accepté.

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by BIGINT REFERENCES users(id) ON DELETE SET NULL
);

INSERT INTO settings (key, value) VALUES ('publishing', '{"open": true, "message": ""}');
