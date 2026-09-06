-- Native FTN schema 23; no message body or transport session authority here.
CREATE TABLE network_queue_work (
 queue_id TEXT PRIMARY KEY, adapter TEXT NOT NULL CHECK(adapter IN ('qwk','ftn'))
);
INSERT INTO network_queue_work SELECT decision_id,'qwk' FROM network_routing_decisions;
CREATE TRIGGER qwk_queue_work AFTER INSERT ON network_routing_decisions BEGIN
 INSERT INTO network_queue_work VALUES(NEW.decision_id,'qwk');
END;
CREATE TABLE network_outbound_queue_n23 (
 queue_id TEXT PRIMARY KEY REFERENCES network_queue_work(queue_id),
 state TEXT NOT NULL CHECK(state IN ('pending','ready','held','retry','accepted','failed','quarantined','cancelled')),
 artifact_id TEXT REFERENCES network_artifacts(artifact_id),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0), attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 12),
 next_attempt INTEGER, reason TEXT NOT NULL DEFAULT 'prepared', created_at INTEGER NOT NULL,
 reserved_bytes INTEGER NOT NULL CHECK(reserved_bytes BETWEEN 0 AND 1048576)
);
INSERT INTO network_outbound_queue_n23 SELECT * FROM network_outbound_queue;
DROP TABLE network_outbound_queue;
ALTER TABLE network_outbound_queue_n23 RENAME TO network_outbound_queue;
CREATE TABLE ftn_addresses (
 address_id INTEGER PRIMARY KEY, domain TEXT NOT NULL CHECK(length(domain) BETWEEN 1 AND 32),
 zone INTEGER NOT NULL CHECK(zone BETWEEN 1 AND 65535), net INTEGER NOT NULL CHECK(net BETWEEN 1 AND 65535),
 node INTEGER NOT NULL CHECK(node BETWEEN 0 AND 65535), point INTEGER NOT NULL CHECK(point BETWEEN 0 AND 65535),
 UNIQUE(domain,zone,net,node,point)
);
CREATE TABLE ftn_link_bindings (
 link_id TEXT PRIMARY KEY, remote_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 local_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id), CHECK(remote_address<>local_address)
);
ALTER TABLE network_quarantine ADD COLUMN ftn_link TEXT REFERENCES ftn_link_bindings(link_id);
CREATE TABLE ftn_serials (
 address_id INTEGER PRIMARY KEY REFERENCES ftn_addresses(address_id),
 next_serial INTEGER NOT NULL CHECK(next_serial BETWEEN 1 AND 4294967296), restored_hold INTEGER NOT NULL CHECK(restored_hold IN (0,1))
);
CREATE TABLE ftn_area_mappings (
 domain TEXT NOT NULL, area TEXT NOT NULL, conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id),
 aka TEXT NOT NULL, receive INTEGER NOT NULL CHECK(receive IN(0,1)), send INTEGER NOT NULL CHECK(send IN(0,1)),
 origin TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0), PRIMARY KEY(domain,area), UNIQUE(domain,conference_id)
);
CREATE TABLE ftn_area_links (
 domain TEXT NOT NULL, area TEXT NOT NULL, link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id),
 PRIMARY KEY(domain,area,link_id), FOREIGN KEY(domain,area) REFERENCES ftn_area_mappings(domain,area)
);
CREATE TABLE ftn_mailbox_aliases (
 address_id INTEGER NOT NULL REFERENCES ftn_addresses(address_id), alias TEXT NOT NULL COLLATE NOCASE,
 caller_id INTEGER NOT NULL REFERENCES callers(caller_id), PRIMARY KEY(address_id,alias), UNIQUE(address_id,caller_id)
);
CREATE TABLE ftn_messages (
 publication_id TEXT PRIMARY KEY, message_id INTEGER NOT NULL REFERENCES messages(message_id),
 domain TEXT NOT NULL, area TEXT NOT NULL, origin_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 destination_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id), recipient TEXT NOT NULL,
 msgid TEXT, reply TEXT, fingerprint TEXT NOT NULL, identity_key TEXT NOT NULL,
 ingress_link TEXT REFERENCES ftn_link_bindings(link_id), charset TEXT NOT NULL,
 original_date BLOB NOT NULL CHECK(length(original_date)=20), utc_offset INTEGER,
 attributes INTEGER NOT NULL CHECK(attributes BETWEEN 0 AND 65535), cost INTEGER NOT NULL,
 origin_line TEXT, tear_line TEXT, received_at INTEGER NOT NULL,
 UNIQUE(message_id,domain,area), UNIQUE(domain,area,identity_key)
);
CREATE TABLE ftn_controls (
 publication_id TEXT NOT NULL REFERENCES ftn_messages(publication_id), ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 127),
 paragraph INTEGER NOT NULL CHECK(paragraph>=0), raw BLOB NOT NULL CHECK(length(raw) BETWEEN 1 AND 1024), PRIMARY KEY(publication_id,ordinal)
);
CREATE TABLE ftn_echo_history (
 publication_id TEXT NOT NULL REFERENCES ftn_messages(publication_id), kind TEXT NOT NULL CHECK(kind IN('seen','path')),
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 1023), net INTEGER NOT NULL CHECK(net BETWEEN 1 AND 65535),
 node INTEGER NOT NULL CHECK(node BETWEEN 0 AND 65535), PRIMARY KEY(publication_id,kind,ordinal)
);
CREATE TABLE ftn_routing_decisions (
 queue_id TEXT PRIMARY KEY REFERENCES network_queue_work(queue_id), publication_id TEXT NOT NULL REFERENCES ftn_messages(publication_id),
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), final_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 next_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id), local_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 aka TEXT NOT NULL, policy_digest TEXT NOT NULL, mapping_version INTEGER NOT NULL, message_version INTEGER NOT NULL,
 reason TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(publication_id,link_id)
);
CREATE TABLE ftn_import_receipts (
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), artifact_id TEXT NOT NULL REFERENCES network_artifacts(artifact_id),
 ordinal INTEGER NOT NULL, message_id INTEGER REFERENCES messages(message_id), outcome TEXT NOT NULL CHECK(outcome IN('imported','duplicate','loop','quarantined')),
 reason TEXT NOT NULL, received_at INTEGER NOT NULL, PRIMARY KEY(link_id,artifact_id,ordinal)
);
CREATE TRIGGER ftn_receipt_budget AFTER INSERT ON ftn_import_receipts BEGIN
 UPDATE network_history_capacity SET receipt_rows=receipt_rows+1,reserved_bytes=reserved_bytes+1024 WHERE singleton=1;
END;
CREATE TRIGGER ftn_publication_budget AFTER INSERT ON ftn_messages BEGIN
 UPDATE network_history_capacity SET reserved_bytes=reserved_bytes+16384 WHERE singleton=1;
END;
CREATE TABLE ftn_directory_sources (
 source_id TEXT PRIMARY KEY, domain TEXT NOT NULL, profile_digest TEXT NOT NULL
);
CREATE TABLE ftn_directory_generations (
 generation_id TEXT PRIMARY KEY, source_id TEXT NOT NULL REFERENCES ftn_directory_sources(source_id),
 artifact_id TEXT NOT NULL REFERENCES network_artifacts(artifact_id), effective_date TEXT NOT NULL, received_at INTEGER NOT NULL,
 format TEXT NOT NULL, charset TEXT NOT NULL, checksum INTEGER, state TEXT NOT NULL CHECK(state IN('validated','rejected')),
 priority INTEGER NOT NULL, cadence_days INTEGER NOT NULL, record_count INTEGER NOT NULL, UNIQUE(source_id,artifact_id,effective_date)
);
CREATE TABLE ftn_directory_records (
 generation_id TEXT NOT NULL REFERENCES ftn_directory_generations(generation_id), address_id INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 keyword TEXT NOT NULL, system TEXT NOT NULL, location TEXT NOT NULL, sysop TEXT NOT NULL, phone TEXT NOT NULL,
 speed INTEGER NOT NULL, source_line INTEGER NOT NULL, raw BLOB NOT NULL CHECK(length(raw)<=2048), PRIMARY KEY(generation_id,address_id)
);
CREATE TABLE ftn_directory_flags (
 generation_id TEXT NOT NULL, address_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, flag TEXT NOT NULL,
 PRIMARY KEY(generation_id,address_id,ordinal), FOREIGN KEY(generation_id,address_id) REFERENCES ftn_directory_records(generation_id,address_id)
);
CREATE TABLE ftn_directory_services (
 generation_id TEXT NOT NULL, address_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, protocol TEXT NOT NULL, host TEXT, port INTEGER,
 PRIMARY KEY(generation_id,address_id,ordinal), FOREIGN KEY(generation_id,address_id) REFERENCES ftn_directory_records(generation_id,address_id)
);
CREATE TABLE ftn_directory_issues (
 generation_id TEXT NOT NULL REFERENCES ftn_directory_generations(generation_id), ordinal INTEGER NOT NULL, line INTEGER NOT NULL,
 reason TEXT NOT NULL, PRIMARY KEY(generation_id,ordinal)
);
CREATE TABLE ftn_directory_active (
 source_id TEXT PRIMARY KEY REFERENCES ftn_directory_sources(source_id), generation_id TEXT NOT NULL REFERENCES ftn_directory_generations(generation_id),
 version INTEGER NOT NULL CHECK(version>0), activated_at INTEGER NOT NULL
);
CREATE TRIGGER ftn_addresses_update BEFORE UPDATE ON ftn_addresses BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_addresses_delete BEFORE DELETE ON ftn_addresses BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_link_bindings_update BEFORE UPDATE ON ftn_link_bindings BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_link_bindings_delete BEFORE DELETE ON ftn_link_bindings BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_messages_update BEFORE UPDATE ON ftn_messages BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_messages_delete BEFORE DELETE ON ftn_messages BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_controls_update BEFORE UPDATE ON ftn_controls BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_controls_delete BEFORE DELETE ON ftn_controls BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_echo_history_update BEFORE UPDATE ON ftn_echo_history BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_echo_history_delete BEFORE DELETE ON ftn_echo_history BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_routing_decisions_update BEFORE UPDATE ON ftn_routing_decisions BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_routing_decisions_delete BEFORE DELETE ON ftn_routing_decisions BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_import_receipts_update BEFORE UPDATE ON ftn_import_receipts BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_import_receipts_delete BEFORE DELETE ON ftn_import_receipts BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_generations_update BEFORE UPDATE ON ftn_directory_generations BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_generations_delete BEFORE DELETE ON ftn_directory_generations BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_records_update BEFORE UPDATE ON ftn_directory_records BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_records_delete BEFORE DELETE ON ftn_directory_records BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_flags_update BEFORE UPDATE ON ftn_directory_flags BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_flags_delete BEFORE DELETE ON ftn_directory_flags BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_services_update BEFORE UPDATE ON ftn_directory_services BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_services_delete BEFORE DELETE ON ftn_directory_services BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_issues_update BEFORE UPDATE ON ftn_directory_issues BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_directory_issues_delete BEFORE DELETE ON ftn_directory_issues BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
-- Both adapters participate in the same global outstanding-work budget.
CREATE TRIGGER ftn_shared_queue_capacity_insert AFTER INSERT ON network_outbound_queue
WHEN NEW.state IN ('pending','ready','held','retry') BEGIN
 SELECT CASE WHEN (SELECT COUNT(*) FROM network_outbound_queue WHERE state IN ('pending','ready','held','retry'))>10000
 OR (SELECT COALESCE(SUM(reserved_bytes),0) FROM network_outbound_queue WHERE state IN ('pending','ready','held','retry'))>268435456
 THEN RAISE(ABORT,'network queue capacity reached') END;
END;
CREATE TRIGGER ftn_shared_queue_capacity_update AFTER UPDATE OF state,reserved_bytes ON network_outbound_queue
WHEN NEW.state IN ('pending','ready','held','retry') BEGIN
 SELECT CASE WHEN (SELECT COUNT(*) FROM network_outbound_queue WHERE state IN ('pending','ready','held','retry'))>10000
 OR (SELECT COALESCE(SUM(reserved_bytes),0) FROM network_outbound_queue WHERE state IN ('pending','ready','held','retry'))>268435456
 THEN RAISE(ABORT,'network queue capacity reached') END;
END;
