-- Generic native Event authority. Runtime leases are cleared on daemon recovery.
CREATE TABLE scheduled_events (
 id TEXT PRIMARY KEY, definition TEXT NOT NULL, version INTEGER NOT NULL,
 next_due INTEGER, last_started INTEGER, last_completed INTEGER, last_result TEXT,
 failures INTEGER NOT NULL DEFAULT 0, running TEXT UNIQUE, requested INTEGER NOT NULL DEFAULT 0,
 observed_generation INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE scheduled_event_history (
 run TEXT PRIMARY KEY, event TEXT NOT NULL REFERENCES scheduled_events(id),
 action TEXT NOT NULL, trigger TEXT NOT NULL, started INTEGER NOT NULL, completed INTEGER, result TEXT,
 targets INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX scheduled_event_history_event ON scheduled_event_history(event,started);
CREATE TABLE network_preparation (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1), generation INTEGER NOT NULL,
 prepared_generation INTEGER NOT NULL
);
INSERT INTO network_preparation VALUES(1,1,0);
-- A crash after message commit cannot lose the obligation to prepare publication.
CREATE TRIGGER native_message_preparation AFTER INSERT ON messages
 WHEN NEW.origin_kind='native' AND NEW.visibility='public' AND NEW.audience_kind='all-callers'
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
CREATE TRIGGER network_queue_activity AFTER INSERT ON network_outbound_queue
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
CREATE TRIGGER circuitnet_control_activity AFTER INSERT ON circuitnet_controls
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
-- Existing FTN FileEcho/FREQ work participates without changing file custody.
CREATE TRIGGER ftn_file_activity_wakeup AFTER INSERT ON ftn_file_deliveries
 BEGIN UPDATE network_preparation SET generation=generation+1 WHERE singleton=1; END;
