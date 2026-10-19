-- Fiche du magasin : bannière (en plus de la vignette et des captures), liens et licence
-- lus dans le manifeste du paquet (convention `store/`, cf. island-formats::media).

ALTER TABLE assets DROP CONSTRAINT assets_kind_check;
ALTER TABLE assets ADD CONSTRAINT assets_kind_check CHECK (kind IN ('thumbnail', 'banner', 'screenshot'));

ALTER TABLE items ADD COLUMN homepage TEXT;
ALTER TABLE items ADD COLUMN repository TEXT;
ALTER TABLE items ADD COLUMN license TEXT;
