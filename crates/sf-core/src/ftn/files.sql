-- Schema 26: file-network adapter state references the native file catalog.
CREATE TABLE ftn_file_policy (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 freq INTEGER NOT NULL CHECK(freq IN (0,1)),
 max_payload INTEGER NOT NULL CHECK(max_payload BETWEEN 1 AND 16777216),
 staging_bytes INTEGER NOT NULL CHECK(staging_bytes BETWEEN 32768 AND 134217728),
 staging_count INTEGER NOT NULL CHECK(staging_count BETWEEN 1 AND 256),
 staging_age INTEGER NOT NULL CHECK(staging_age BETWEEN 60 AND 604800),
 freq_files INTEGER NOT NULL CHECK(freq_files BETWEEN 1 AND 32),
 freq_bytes INTEGER NOT NULL CHECK(freq_bytes BETWEEN 1 AND 67108864),
 version INTEGER NOT NULL CHECK(version>0)
);
INSERT INTO ftn_file_policy VALUES(1,0,0,16777216,67108864,128,86400,8,33554432,1);
CREATE TABLE ftn_file_areas (
 domain TEXT NOT NULL, tag TEXT NOT NULL, area_id INTEGER NOT NULL REFERENCES file_areas(area_id),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 inbound INTEGER NOT NULL CHECK(inbound IN (0,1)), outbound INTEGER NOT NULL CHECK(outbound IN (0,1)),
 description TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0),
 PRIMARY KEY(domain,tag), UNIQUE(area_id)
);
CREATE TABLE ftn_file_subscriptions (
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), domain TEXT NOT NULL, tag TEXT NOT NULL,
 inbound INTEGER NOT NULL CHECK(inbound IN (0,1)), subscribed INTEGER NOT NULL CHECK(subscribed IN (0,1)),
 held INTEGER NOT NULL CHECK(held IN (0,1)), version INTEGER NOT NULL CHECK(version>0),
 PRIMARY KEY(link_id,domain,tag), FOREIGN KEY(domain,tag) REFERENCES ftn_file_areas(domain,tag)
);
CREATE TABLE ftn_freq_grants (
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), name TEXT NOT NULL,
 file_id INTEGER NOT NULL REFERENCES files(file_id), enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 version INTEGER NOT NULL CHECK(version>0), PRIMARY KEY(link_id,name)
);
CREATE TABLE ftn_file_staging (
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), name TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('tic','payload')), content BLOB NOT NULL,
 digest TEXT NOT NULL, received_at INTEGER NOT NULL,
 PRIMARY KEY(link_id,name,kind)
);
CREATE TABLE ftn_file_publications (
 publication_id TEXT PRIMARY KEY, domain TEXT NOT NULL, tag TEXT NOT NULL,
 file_id INTEGER NOT NULL REFERENCES files(file_id), sha256 TEXT NOT NULL, size INTEGER NOT NULL,
 metadata TEXT NOT NULL, ingress TEXT REFERENCES ftn_link_bindings(link_id),
 source TEXT NOT NULL CHECK(source IN ('inbound','hatch')), created_at INTEGER NOT NULL,
 UNIQUE(domain,tag,sha256,size), FOREIGN KEY(domain,tag) REFERENCES ftn_file_areas(domain,tag)
);
CREATE TABLE ftn_file_deliveries (
 delivery_id TEXT PRIMARY KEY, publication_id TEXT REFERENCES ftn_file_publications(publication_id),
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id),
 file_id INTEGER REFERENCES files(file_id), name TEXT NOT NULL, sha256 TEXT NOT NULL, size INTEGER NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('fileecho','freq-response','freq-request')),
 request BLOB, request_key TEXT,
 payload_accepted INTEGER NOT NULL DEFAULT 0 CHECK(payload_accepted IN (0,1)),
 tic_accepted INTEGER NOT NULL DEFAULT 0 CHECK(tic_accepted IN (0,1)),
 held INTEGER NOT NULL DEFAULT 0 CHECK(held IN (0,1)), attempts INTEGER NOT NULL DEFAULT 0,
 session_id TEXT, payload_offered INTEGER NOT NULL DEFAULT 0, tic_offered INTEGER NOT NULL DEFAULT 0,
 last_error TEXT, created_at INTEGER NOT NULL, accepted_at INTEGER, version INTEGER NOT NULL DEFAULT 1,
 UNIQUE(publication_id,link_id), UNIQUE(request_key,link_id,file_id)
);
CREATE TABLE ftn_file_activity (
 activity_id INTEGER PRIMARY KEY, link_id TEXT REFERENCES ftn_link_bindings(link_id),
 name TEXT, result TEXT NOT NULL, files INTEGER NOT NULL, bytes INTEGER NOT NULL, occurred_at INTEGER NOT NULL
);
CREATE INDEX ftn_file_delivery_link ON ftn_file_deliveries(link_id,accepted_at,created_at);
CREATE TRIGGER ftn_file_publication_update BEFORE UPDATE ON ftn_file_publications BEGIN SELECT RAISE(ABORT,'retained file provenance'); END;
CREATE TRIGGER ftn_file_publication_delete BEFORE DELETE ON ftn_file_publications BEGIN SELECT RAISE(ABORT,'retained file provenance'); END;
CREATE TRIGGER ftn_file_delivery_truth BEFORE UPDATE ON ftn_file_deliveries WHEN NEW.payload_accepted<OLD.payload_accepted OR NEW.tic_accepted<OLD.tic_accepted BEGIN SELECT RAISE(ABORT,'retained file delivery truth'); END;

ALTER TABLE binkp_link_health ADD COLUMN freq_files INTEGER NOT NULL DEFAULT 0;
ALTER TABLE binkp_link_health ADD COLUMN freq_bytes INTEGER NOT NULL DEFAULT 0;

CREATE TABLE ftn_freq_inbound (
 request_id TEXT NOT NULL REFERENCES ftn_file_deliveries(delivery_id),
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), name TEXT NOT NULL,
 area_id INTEGER NOT NULL REFERENCES file_areas(area_id), file_id INTEGER REFERENCES files(file_id),
 sha256 TEXT, size INTEGER, received_at INTEGER,
 PRIMARY KEY(request_id,name)
);
CREATE UNIQUE INDEX ftn_freq_pending_name ON ftn_freq_inbound(link_id,name) WHERE file_id IS NULL;
