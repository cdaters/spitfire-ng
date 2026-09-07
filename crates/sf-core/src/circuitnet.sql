-- C2 metadata surrounds native messages; no CircuitNET message body table.
CREATE TABLE network_queue_work_n29 (
 queue_id TEXT PRIMARY KEY, adapter TEXT NOT NULL CHECK(adapter IN ('qwk','ftn','circuitnet'))
);
INSERT INTO network_queue_work_n29 SELECT * FROM network_queue_work;
DROP TABLE network_queue_work;
ALTER TABLE network_queue_work_n29 RENAME TO network_queue_work;
CREATE TABLE circuitnet_profiles (
 network TEXT PRIMARY KEY, configuration TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version>0)
);
CREATE TABLE circuitnet_mappings (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), codename TEXT NOT NULL,
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id),
 send INTEGER NOT NULL CHECK(send IN(0,1)), receive INTEGER NOT NULL CHECK(receive IN(0,1)),
 version INTEGER NOT NULL CHECK(version>0), PRIMARY KEY(network,codename), UNIQUE(network,conference_id)
);
CREATE TABLE circuitnet_dossiers (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), neighbor TEXT NOT NULL,
 codename TEXT NOT NULL, subscribed INTEGER NOT NULL CHECK(subscribed IN(0,1)),
 version INTEGER NOT NULL CHECK(version>0), PRIMARY KEY(network,neighbor,codename)
);
CREATE TABLE circuitnet_messages (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), identity TEXT NOT NULL,
 message_id INTEGER NOT NULL REFERENCES messages(message_id), codename TEXT NOT NULL,
 origin TEXT NOT NULL, ingress TEXT, reply TEXT, path TEXT NOT NULL, fingerprint TEXT NOT NULL,
 message_version INTEGER NOT NULL, created_at INTEGER NOT NULL,
 PRIMARY KEY(network,identity), UNIQUE(network,message_id)
);
CREATE TABLE circuitnet_deliveries (
 queue_id TEXT PRIMARY KEY REFERENCES network_queue_work(queue_id), network TEXT NOT NULL,
 identity TEXT NOT NULL, neighbor TEXT NOT NULL, created_at INTEGER NOT NULL,
 UNIQUE(network,identity,neighbor), FOREIGN KEY(network,identity) REFERENCES circuitnet_messages(network,identity)
);
CREATE TABLE circuitnet_batches (
 artifact TEXT PRIMARY KEY REFERENCES network_artifacts(artifact_id), network TEXT NOT NULL REFERENCES circuitnet_profiles(network),
 neighbor TEXT NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE circuitnet_batch_members (
 artifact TEXT NOT NULL REFERENCES circuitnet_batches(artifact), queue_id TEXT NOT NULL REFERENCES circuitnet_deliveries(queue_id),
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 31), PRIMARY KEY(artifact,ordinal), UNIQUE(artifact,queue_id)
);
CREATE TABLE circuitnet_imports (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), neighbor TEXT NOT NULL,
 artifact TEXT NOT NULL REFERENCES network_artifacts(artifact_id), accepted INTEGER NOT NULL,
 duplicates INTEGER NOT NULL, received_at INTEGER NOT NULL, PRIMARY KEY(network,neighbor,artifact)
);
CREATE TABLE circuitnet_receipts (
 event_id INTEGER PRIMARY KEY, network TEXT NOT NULL REFERENCES circuitnet_profiles(network), neighbor TEXT NOT NULL,
 artifact TEXT NOT NULL REFERENCES network_artifacts(artifact_id),
 outcome TEXT NOT NULL CHECK(outcome IN('imported','replayed','acknowledged','ack-replayed')),
 occurred_at INTEGER NOT NULL
);
CREATE TABLE circuitnet_changes (
 event_id INTEGER PRIMARY KEY, network TEXT NOT NULL REFERENCES circuitnet_profiles(network),
 operation TEXT NOT NULL, actor TEXT NOT NULL, occurred_at INTEGER NOT NULL
);
CREATE TRIGGER circuitnet_messages_update BEFORE UPDATE ON circuitnet_messages BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_messages_delete BEFORE DELETE ON circuitnet_messages BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_deliveries_update BEFORE UPDATE ON circuitnet_deliveries BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_deliveries_delete BEFORE DELETE ON circuitnet_deliveries BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_batches_update BEFORE UPDATE ON circuitnet_batches BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_batches_delete BEFORE DELETE ON circuitnet_batches BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_batch_members_update BEFORE UPDATE ON circuitnet_batch_members BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_batch_members_delete BEFORE DELETE ON circuitnet_batch_members BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_imports_update BEFORE UPDATE ON circuitnet_imports BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_imports_delete BEFORE DELETE ON circuitnet_imports BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_receipts_update BEFORE UPDATE ON circuitnet_receipts BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_receipts_delete BEFORE DELETE ON circuitnet_receipts BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_changes_update BEFORE UPDATE ON circuitnet_changes BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
CREATE TRIGGER circuitnet_changes_delete BEFORE DELETE ON circuitnet_changes BEGIN SELECT RAISE(ABORT,'retained CircuitNET authority'); END;
