-- C7 signed network metadata; native conference numbers and messages remain local.
CREATE TABLE circuitnet_catalog_authority (
 network TEXT PRIMARY KEY REFERENCES circuitnet_profiles(network), authority TEXT NOT NULL
);
CREATE TABLE circuitnet_catalog_revisions (
 network TEXT NOT NULL REFERENCES circuitnet_catalog_authority(network), revision INTEGER NOT NULL CHECK(revision>0),
 hash TEXT NOT NULL, object TEXT NOT NULL, accepted_at INTEGER NOT NULL, PRIMARY KEY(network,revision)
);
CREATE TABLE circuitnet_catalog_choices (
 network TEXT NOT NULL, identity TEXT NOT NULL, decision TEXT NOT NULL CHECK(decision IN('mapped','ignored')),
 conference_id INTEGER REFERENCES message_conferences(conference_id), after_message INTEGER NOT NULL,
 PRIMARY KEY(network,identity), FOREIGN KEY(network) REFERENCES circuitnet_catalog_authority(network)
);
CREATE TABLE circuitnet_catalog_dossiers (
 network TEXT NOT NULL, neighbor TEXT NOT NULL, codename TEXT NOT NULL, identity TEXT NOT NULL,
 PRIMARY KEY(network,neighbor,codename,identity), FOREIGN KEY(network) REFERENCES circuitnet_catalog_authority(network)
);
CREATE TABLE circuitnet_catalog_health (
 network TEXT PRIMARY KEY REFERENCES circuitnet_catalog_authority(network), last_success INTEGER,
 rejected INTEGER NOT NULL DEFAULT 0, last_error TEXT, upstream_revision INTEGER NOT NULL DEFAULT 0
);
ALTER TABLE circuitnet_messages ADD COLUMN conference_identity TEXT;
CREATE TRIGGER circuitnet_catalog_revisions_update BEFORE UPDATE ON circuitnet_catalog_revisions BEGIN SELECT RAISE(ABORT,'retained catalog revision'); END;
CREATE TRIGGER circuitnet_catalog_revisions_delete BEFORE DELETE ON circuitnet_catalog_revisions BEGIN SELECT RAISE(ABORT,'retained catalog revision'); END;
CREATE TRIGGER circuitnet_catalog_authority_update BEFORE UPDATE ON circuitnet_catalog_authority BEGIN SELECT RAISE(ABORT,'pinned catalog authority'); END;
CREATE TRIGGER circuitnet_catalog_authority_delete BEFORE DELETE ON circuitnet_catalog_authority BEGIN SELECT RAISE(ABORT,'pinned catalog authority'); END;
CREATE TABLE circuitnet_catalog_peers (
 network TEXT NOT NULL REFERENCES circuitnet_catalog_authority(network), neighbor TEXT NOT NULL,
 revision INTEGER NOT NULL, hash TEXT, observed_at INTEGER NOT NULL, PRIMARY KEY(network,neighbor)
);
-- Reuse the generic Event wakeup generation; no catalog-specific timer.
CREATE TRIGGER circuitnet_catalog_activity AFTER INSERT ON circuitnet_catalog_revisions
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
