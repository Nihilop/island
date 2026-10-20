-- Extensions au magasin : une version déposée attend la relecture (`pending`). Sa fiche
-- (nom, résumé, liens, médias) n'est appliquée à l'élément qu'à l'approbation : `meta`.
-- `submitted_key` = paquet tel qu'envoyé par l'auteur (gardé pour la relecture) ;
-- `file_key` = paquet distribué (re-signé par le magasin à l'approbation).

ALTER TABLE versions ADD COLUMN meta JSONB;
ALTER TABLE versions ADD COLUMN author_key TEXT;
ALTER TABLE versions ADD COLUMN submitted_key TEXT;

CREATE INDEX versions_pending ON versions (created_at) WHERE status = 'pending';
