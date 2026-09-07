-- SPITFIRE NG identity authority, schema 28. Legacy identity bytes are preserved.
ALTER TABLE callers ADD COLUMN first_name TEXT
 CHECK(first_name IS NULL OR (length(CAST(first_name AS BLOB)) BETWEEN 1 AND 60));
ALTER TABLE callers ADD COLUMN last_name TEXT
 CHECK(last_name IS NULL OR (length(CAST(last_name AS BLOB)) BETWEEN 1 AND 60));
CREATE TRIGGER caller_legacy_name_immutable BEFORE UPDATE OF real_name ON callers
 WHEN NEW.real_name IS NOT OLD.real_name BEGIN
 SELECT RAISE(ABORT,'legacy name is preservation data');
END;
CREATE TRIGGER caller_name_bounds_insert BEFORE INSERT ON callers
 WHEN NEW.first_name IS NOT NULL AND NEW.last_name IS NOT NULL
 AND (length(NEW.first_name || ' ' || NEW.last_name)>60
 OR length(CAST(NEW.first_name || ' ' || NEW.last_name AS BLOB))>120) BEGIN
 SELECT RAISE(ABORT,'name components exceed posting identity bounds');
END;
CREATE TRIGGER caller_name_bounds_update BEFORE UPDATE OF first_name,last_name ON callers
 WHEN NEW.first_name IS NOT NULL AND NEW.last_name IS NOT NULL
 AND (length(NEW.first_name || ' ' || NEW.last_name)>60
 OR length(CAST(NEW.first_name || ' ' || NEW.last_name AS BLOB))>120) BEGIN
 SELECT RAISE(ABORT,'name components exceed posting identity bounds');
END;
ALTER TABLE message_conferences ADD COLUMN posting_identity TEXT
 CHECK(posting_identity IS NULL OR posting_identity IN ('handle-allowed','real-name-required'));
ALTER TABLE message_conferences ADD COLUMN identity_policy_version INTEGER NOT NULL DEFAULT 1
 CHECK(identity_policy_version>0);
ALTER TABLE ftn_area_mappings ADD COLUMN posting_identity TEXT NOT NULL DEFAULT 'handle-allowed'
 CHECK(posting_identity IN ('handle-allowed','real-name-required'));
ALTER TABLE qwk_link_mappings ADD COLUMN posting_identity TEXT NOT NULL DEFAULT 'handle-allowed'
 CHECK(posting_identity IN ('handle-allowed','real-name-required'));
ALTER TABLE qwk_links ADD COLUMN posting_identity TEXT NOT NULL DEFAULT 'handle-allowed'
 CHECK(posting_identity IN ('handle-allowed','real-name-required'));
ALTER TABLE messages ADD COLUMN identity_mode TEXT NOT NULL DEFAULT 'legacy-unclassified'
 CHECK(identity_mode IN ('handle','real-name','enrolled-network-alias','external-asserted','system','legacy-unclassified'));
ALTER TABLE messages ADD COLUMN identity_proof TEXT;
UPDATE messages SET identity_mode='external-asserted'
 WHERE origin_kind='external-network' AND author_caller_id IS NULL;
CREATE TRIGGER message_author_immutable
 BEFORE UPDATE OF author_caller_id,author_name,identity_mode,identity_proof ON messages
 WHEN NEW.author_caller_id IS NOT OLD.author_caller_id OR NEW.author_name IS NOT OLD.author_name
 OR NEW.identity_mode IS NOT OLD.identity_mode OR NEW.identity_proof IS NOT OLD.identity_proof BEGIN
 SELECT RAISE(ABORT,'posted author identity is immutable');
END;
CREATE TABLE network_sender_snapshots (
 queue_id TEXT PRIMARY KEY REFERENCES network_outbound_queue(queue_id) ON DELETE RESTRICT,
 message_id INTEGER NOT NULL REFERENCES messages(message_id) ON DELETE RESTRICT,
 sender TEXT NOT NULL CHECK(length(sender) BETWEEN 1 AND 60),
 wire_sender BLOB NOT NULL CHECK(length(wire_sender)>0),
 wire_profile TEXT NOT NULL,
 identity_mode TEXT NOT NULL,
 identity_proof TEXT
);
CREATE TRIGGER network_sender_snapshot_update BEFORE UPDATE ON network_sender_snapshots BEGIN
 SELECT RAISE(ABORT,'queued sender identity is immutable');
END;
CREATE TRIGGER network_sender_snapshot_delete BEFORE DELETE ON network_sender_snapshots BEGIN
 SELECT RAISE(ABORT,'queued sender identity is immutable');
END;
-- Separate append-only component events avoid rewriting the existing identity audit.
CREATE TABLE caller_name_events (
 event_id INTEGER PRIMARY KEY,
 caller_id INTEGER NOT NULL REFERENCES callers(caller_id) ON DELETE RESTRICT,
 occurred_at INTEGER NOT NULL CHECK(occurred_at>=0),
 prior_state_version INTEGER NOT NULL CHECK(prior_state_version>=0),
 new_state_version INTEGER NOT NULL CHECK(new_state_version=prior_state_version+1),
 actor_kind TEXT NOT NULL CHECK(actor_kind IN ('caller','local-operator'))
);
CREATE TRIGGER caller_name_events_update BEFORE UPDATE ON caller_name_events BEGIN
 SELECT RAISE(ABORT,'caller name events are append-only');
END;
CREATE TRIGGER caller_name_events_delete BEFORE DELETE ON caller_name_events BEGIN
 SELECT RAISE(ABORT,'caller name events are append-only');
END;
