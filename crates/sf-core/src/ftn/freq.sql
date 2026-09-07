-- Schema 27: attempt history extends, never replaces, original FREQ authority.
CREATE TABLE ftn_freq_recovery (
 request_id TEXT PRIMARY KEY REFERENCES ftn_file_deliveries(delivery_id),
 held INTEGER NOT NULL DEFAULT 0 CHECK(held IN (0,1)),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0)
);
CREATE TABLE ftn_freq_attempts (
 request_id TEXT NOT NULL REFERENCES ftn_freq_recovery(request_id),
 number INTEGER NOT NULL CHECK(number BETWEEN 1 AND 3),
 delivery_id TEXT NOT NULL UNIQUE REFERENCES ftn_file_deliveries(delivery_id),
 offered INTEGER NOT NULL DEFAULT 0 CHECK(offered IN (0,1)),
 PRIMARY KEY(request_id,number)
);
INSERT INTO ftn_freq_recovery(request_id)
 SELECT DISTINCT request_id FROM ftn_freq_inbound;
INSERT INTO ftn_freq_attempts(request_id,number,delivery_id,offered)
 SELECT r.request_id,1,r.request_id,MAX(d.payload_offered,d.payload_accepted)
 FROM ftn_freq_recovery r JOIN ftn_file_deliveries d ON d.delivery_id=r.request_id;
CREATE TRIGGER ftn_freq_attempt_truth BEFORE UPDATE ON ftn_freq_attempts
 WHEN NEW.request_id<>OLD.request_id OR NEW.number<>OLD.number OR NEW.delivery_id<>OLD.delivery_id OR NEW.offered<OLD.offered
 BEGIN SELECT RAISE(ABORT,'retained FREQ attempt'); END;
CREATE TRIGGER ftn_freq_attempt_delete BEFORE DELETE ON ftn_freq_attempts
 BEGIN SELECT RAISE(ABORT,'retained FREQ attempt'); END;
CREATE TRIGGER ftn_freq_receipt_truth BEFORE UPDATE ON ftn_freq_inbound
 WHEN NEW.request_id<>OLD.request_id OR NEW.link_id<>OLD.link_id OR NEW.name<>OLD.name OR NEW.area_id<>OLD.area_id
 OR (OLD.file_id IS NOT NULL AND (NEW.file_id IS NOT OLD.file_id OR NEW.sha256 IS NOT OLD.sha256 OR NEW.size IS NOT OLD.size OR NEW.received_at IS NOT OLD.received_at))
 BEGIN SELECT RAISE(ABORT,'retained FREQ receipt'); END;
CREATE TRIGGER ftn_freq_receipt_delete BEFORE DELETE ON ftn_freq_inbound
 BEGIN SELECT RAISE(ABORT,'retained FREQ receipt'); END;
