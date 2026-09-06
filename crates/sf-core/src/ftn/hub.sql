-- Schema 25: native hub policy, subscriptions and bounded replay intent; no bodies.
CREATE TABLE ftn_downstreams (
 link_id TEXT PRIMARY KEY REFERENCES ftn_link_bindings(link_id),
 enabled INTEGER NOT NULL CHECK(enabled IN(0,1)), boss_aka TEXT,
 areafix INTEGER NOT NULL CHECK(areafix IN(0,1)),
 rescan INTEGER NOT NULL CHECK(rescan IN(0,1)),
 max_area INTEGER NOT NULL CHECK(max_area BETWEEN 1 AND 500),
 max_total INTEGER NOT NULL CHECK(max_total BETWEEN 1 AND 1000 AND max_total>=max_area),
 cooldown INTEGER NOT NULL CHECK(cooldown BETWEEN 60 AND 86400),
 version INTEGER NOT NULL CHECK(version>0), held INTEGER NOT NULL DEFAULT 0 CHECK(held IN(0,1))
);
CREATE TABLE ftn_subscriptions (
 link_id TEXT NOT NULL REFERENCES ftn_downstreams(link_id), domain TEXT NOT NULL, area TEXT NOT NULL,
 subscribed INTEGER NOT NULL CHECK(subscribed IN(0,1)),
 source TEXT NOT NULL CHECK(source IN('manual','areafix')),
 version INTEGER NOT NULL CHECK(version>0), changed_at INTEGER NOT NULL,
 PRIMARY KEY(link_id,domain,area), FOREIGN KEY(domain,area) REFERENCES ftn_area_mappings(domain,area)
);
CREATE TABLE ftn_area_access (
 domain TEXT NOT NULL, area TEXT NOT NULL,
 remote_subscribe INTEGER NOT NULL CHECK(remote_subscribe IN(0,1)),
 rescan INTEGER NOT NULL CHECK(rescan IN(0,1)), version INTEGER NOT NULL CHECK(version>0),
 PRIMARY KEY(domain,area), FOREIGN KEY(domain,area) REFERENCES ftn_area_mappings(domain,area)
);
CREATE TABLE ftn_areafix_requests (
 request_id TEXT PRIMARY KEY, link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id),
 identity TEXT NOT NULL, fingerprint TEXT NOT NULL, authenticated INTEGER NOT NULL CHECK(authenticated IN(0,1)),
 commands INTEGER NOT NULL CHECK(commands BETWEEN 0 AND 32), changes INTEGER NOT NULL CHECK(changes BETWEEN 0 AND 32),
 rescans INTEGER NOT NULL CHECK(rescans BETWEEN 0 AND 32), result TEXT NOT NULL,
 response_id INTEGER REFERENCES messages(message_id), received_at INTEGER NOT NULL,
 UNIQUE(link_id,identity)
);
CREATE TABLE ftn_rescans (
 request_id TEXT PRIMARY KEY, link_id TEXT NOT NULL REFERENCES ftn_downstreams(link_id),
 source TEXT NOT NULL CHECK(source IN('manual','areafix')), created_at INTEGER NOT NULL
);
CREATE TABLE ftn_rescan_areas (
 request_id TEXT NOT NULL REFERENCES ftn_rescans(request_id), domain TEXT NOT NULL, area TEXT NOT NULL,
 requested INTEGER NOT NULL CHECK(requested BETWEEN 1 AND 500), queued INTEGER NOT NULL CHECK(queued BETWEEN 0 AND 500),
 PRIMARY KEY(request_id,domain,area), FOREIGN KEY(domain,area) REFERENCES ftn_area_mappings(domain,area)
);
ALTER TABLE binkp_link_health ADD COLUMN authenticated INTEGER NOT NULL DEFAULT 0 CHECK(authenticated IN(0,1));
CREATE TABLE ftn_routing_decisions_n25 (
 queue_id TEXT PRIMARY KEY REFERENCES network_queue_work(queue_id), publication_id TEXT NOT NULL REFERENCES ftn_messages(publication_id),
 link_id TEXT NOT NULL REFERENCES ftn_link_bindings(link_id), final_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 next_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id), local_address INTEGER NOT NULL REFERENCES ftn_addresses(address_id),
 aka TEXT NOT NULL, policy_digest TEXT NOT NULL, mapping_version INTEGER NOT NULL, message_version INTEGER NOT NULL,
 reason TEXT NOT NULL, created_at INTEGER NOT NULL, delivery_key TEXT NOT NULL DEFAULT 'normal', UNIQUE(publication_id,link_id,delivery_key)
);
INSERT INTO ftn_routing_decisions_n25 SELECT *, 'normal' FROM ftn_routing_decisions;
DROP TABLE ftn_routing_decisions;
ALTER TABLE ftn_routing_decisions_n25 RENAME TO ftn_routing_decisions;
CREATE TRIGGER ftn_routing_decisions_update BEFORE UPDATE ON ftn_routing_decisions BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_routing_decisions_delete BEFORE DELETE ON ftn_routing_decisions BEGIN SELECT RAISE(ABORT,'retained FTN authority'); END;
CREATE TRIGGER ftn_downstreams_retained BEFORE DELETE ON ftn_downstreams BEGIN SELECT RAISE(ABORT,'disable retained downstream'); END;
CREATE TRIGGER ftn_areafix_budget AFTER INSERT ON ftn_areafix_requests BEGIN
 UPDATE network_history_capacity SET receipt_rows=receipt_rows+1,reserved_bytes=reserved_bytes+2048 WHERE singleton=1;
END;
CREATE TRIGGER ftn_rescan_budget AFTER INSERT ON ftn_rescans BEGIN
 UPDATE network_history_capacity SET receipt_rows=receipt_rows+1,reserved_bytes=reserved_bytes+4096 WHERE singleton=1;
END;
CREATE TRIGGER ftn_areafix_requests_update BEFORE UPDATE ON ftn_areafix_requests BEGIN SELECT RAISE(ABORT,'retained hub receipt'); END;
CREATE TRIGGER ftn_areafix_requests_delete BEFORE DELETE ON ftn_areafix_requests BEGIN SELECT RAISE(ABORT,'retained hub receipt'); END;
CREATE TRIGGER ftn_rescans_update BEFORE UPDATE ON ftn_rescans BEGIN SELECT RAISE(ABORT,'retained hub receipt'); END;
CREATE TRIGGER ftn_rescans_delete BEFORE DELETE ON ftn_rescans BEGIN SELECT RAISE(ABORT,'retained hub receipt'); END;
CREATE TRIGGER ftn_rescan_areas_update BEFORE UPDATE ON ftn_rescan_areas BEGIN SELECT RAISE(ABORT,'retained hub receipt'); END;
CREATE TRIGGER ftn_rescan_areas_delete BEFORE DELETE ON ftn_rescan_areas BEGIN SELECT RAISE(ABORT,'retained hub receipt'); END;
