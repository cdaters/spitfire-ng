-- Schema 24: transport claims and finite observations around the existing queue.
CREATE TABLE binkp_link_health (
 link_id TEXT PRIMARY KEY REFERENCES ftn_link_bindings(link_id),
 policy_digest TEXT NOT NULL, session_id TEXT UNIQUE, daemon_generation TEXT,
 last_attempt INTEGER, last_success INTEGER, last_error TEXT, latency_ms INTEGER,
 failures INTEGER NOT NULL DEFAULT 0 CHECK(failures BETWEEN 0 AND 12),
 next_attempt INTEGER, held INTEGER NOT NULL DEFAULT 0 CHECK(held IN(0,1)), last_test INTEGER,
 CHECK((session_id IS NULL)=(daemon_generation IS NULL))
);
CREATE TABLE binkp_queue_claims (
 queue_id TEXT PRIMARY KEY REFERENCES network_outbound_queue(queue_id),
 session_id TEXT NOT NULL REFERENCES binkp_link_health(session_id),
 queue_version INTEGER NOT NULL, offered INTEGER NOT NULL DEFAULT 0 CHECK(offered IN(0,1))
);
CREATE TABLE binkp_peer_addresses (
 link_id TEXT NOT NULL REFERENCES binkp_link_health(link_id),
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 31),
 address_id INTEGER NOT NULL REFERENCES ftn_addresses(address_id), PRIMARY KEY(link_id,ordinal)
);
CREATE TABLE binkp_capabilities (
 link_id TEXT NOT NULL REFERENCES binkp_link_health(link_id),
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 31),
 capability TEXT NOT NULL CHECK(length(capability) BETWEEN 1 AND 32), PRIMARY KEY(link_id,ordinal)
);
CREATE TABLE binkp_custody_receipts (
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id),
 artifact_id TEXT NOT NULL REFERENCES network_artifacts(artifact_id), received_at INTEGER NOT NULL,
 PRIMARY KEY(link_id,artifact_id)
);
CREATE TRIGGER binkp_receipt_budget AFTER INSERT ON binkp_custody_receipts BEGIN
 UPDATE network_history_capacity SET receipt_rows=receipt_rows+1,reserved_bytes=reserved_bytes+1024 WHERE singleton=1;
END;
CREATE TRIGGER binkp_receipt_immutable BEFORE UPDATE ON binkp_custody_receipts BEGIN SELECT RAISE(ABORT,'custody receipt is immutable'); END;
CREATE TRIGGER binkp_receipt_retained BEFORE DELETE ON binkp_custody_receipts BEGIN SELECT RAISE(ABORT,'custody receipt is retained'); END;
