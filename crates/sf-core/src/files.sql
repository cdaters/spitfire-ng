-- C6 extends the native catalog. Networks reference files, never own payloads.
ALTER TABLE files ADD COLUMN safety_required INTEGER NOT NULL DEFAULT 0 CHECK(safety_required IN (0,1));
CREATE TABLE file_area_safety (
 area_id INTEGER PRIMARY KEY REFERENCES file_areas(area_id), policy TEXT NOT NULL
);
CREATE TABLE file_content (
 sha256 TEXT PRIMARY KEY CHECK(length(sha256)=64), size_bytes INTEGER NOT NULL CHECK(size_bytes>=0),
 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE file_validation (
 file_id INTEGER PRIMARY KEY REFERENCES files(file_id), sha256 TEXT NOT NULL REFERENCES file_content(sha256),
 original_filename TEXT NOT NULL, source TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('inspecting','quarantined','pending-approval','published','rejected')),
 policy TEXT NOT NULL, report TEXT, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE file_safety_history (
 id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(file_id),
 operation TEXT NOT NULL, result TEXT NOT NULL, actor TEXT NOT NULL DEFAULT 'native-admission', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TRIGGER file_safety_fence BEFORE UPDATE OF lifecycle ON files
 WHEN NEW.lifecycle='active' AND (NEW.safety_required=1 OR EXISTS(SELECT 1 FROM file_validation WHERE file_id=NEW.file_id)) AND NOT EXISTS(SELECT 1 FROM file_validation WHERE file_id=NEW.file_id AND status='published')
 BEGIN SELECT RAISE(ABORT,'file safety decision prevents publication'); END;
CREATE TRIGGER file_content_identity_fence BEFORE UPDATE OF sha256,size_bytes ON files
 WHEN EXISTS(SELECT 1 FROM file_validation WHERE file_id=NEW.file_id)
 AND (NEW.sha256<>OLD.sha256 OR NEW.size_bytes<>OLD.size_bytes)
 BEGIN SELECT RAISE(ABORT,'original artifact is immutable'); END;
CREATE TRIGGER file_publication_activity AFTER UPDATE OF status ON file_validation
 WHEN NEW.status='published' AND OLD.status<>'published'
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
CREATE TABLE circuitnet_file_mappings (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), codename TEXT NOT NULL,
 area_id INTEGER NOT NULL REFERENCES file_areas(area_id), send INTEGER NOT NULL, receive INTEGER NOT NULL,
 maximum_bytes INTEGER NOT NULL, version INTEGER NOT NULL, PRIMARY KEY(network,codename)
);
CREATE TABLE circuitnet_file_dossiers (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), neighbor TEXT NOT NULL, codename TEXT NOT NULL,
 subscribed INTEGER NOT NULL, version INTEGER NOT NULL, PRIMARY KEY(network,neighbor,codename)
);
CREATE TABLE circuitnet_file_publications (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), identity TEXT NOT NULL,
 file_id INTEGER REFERENCES files(file_id), codename TEXT NOT NULL, envelope TEXT NOT NULL,
 fingerprint TEXT NOT NULL, ingress TEXT, receipt TEXT, PRIMARY KEY(network,identity)
);
CREATE UNIQUE INDEX circuitnet_file_local_identity ON circuitnet_file_publications(network,file_id,codename) WHERE ingress IS NULL;
CREATE TABLE circuitnet_file_deliveries (
 network TEXT NOT NULL, identity TEXT NOT NULL, neighbor TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0,
 receipt TEXT, error TEXT, updated_at INTEGER NOT NULL,
 PRIMARY KEY(network,identity,neighbor), FOREIGN KEY(network,identity) REFERENCES circuitnet_file_publications(network,identity)
);
CREATE TRIGGER circuitnet_file_activity AFTER INSERT ON circuitnet_file_deliveries
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
