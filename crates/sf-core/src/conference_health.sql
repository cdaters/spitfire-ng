-- Native Conference Health projections; no message bodies or account names.
CREATE TABLE conference_health_config (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1), enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN(0,1)),
 retention_days INTEGER NOT NULL DEFAULT 365 CHECK(retention_days BETWEEN 180 AND 730),
 dormant_days INTEGER NOT NULL DEFAULT 30 CHECK(dormant_days IN(7,30,90)),
 bulletin INTEGER NOT NULL DEFAULT 0 CHECK(bulletin IN(0,1)), bulletin_limit INTEGER NOT NULL DEFAULT 10 CHECK(bulletin_limit BETWEEN 1 AND 20),
 monitoring_since INTEGER NOT NULL, seed_cursor INTEGER NOT NULL DEFAULT 0,
 seed_until INTEGER NOT NULL, last_rollup INTEGER
);
INSERT INTO conference_health_config(singleton,monitoring_since,seed_until)
 VALUES(1,unixepoch(),COALESCE((SELECT max(message_id) FROM messages),0));
CREATE TABLE conference_health_tokens (
 caller_id INTEGER PRIMARY KEY REFERENCES callers(caller_id) ON DELETE CASCADE,
 token TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16))))
);
CREATE TRIGGER conference_health_forget_caller AFTER UPDATE OF account_state ON callers
 WHEN NEW.account_state='deleted' BEGIN
 DELETE FROM conference_health_tokens WHERE caller_id=NEW.caller_id; END;
CREATE TABLE conference_health_reads (
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE CASCADE,
 day INTEGER NOT NULL, token TEXT NOT NULL, progress INTEGER NOT NULL, last_at INTEGER NOT NULL,
 PRIMARY KEY(conference_id,day,token)
);
CREATE INDEX conference_health_reads_day ON conference_health_reads(day);
CREATE TABLE conference_health_messages (
 message_id INTEGER PRIMARY KEY REFERENCES messages(message_id) ON DELETE CASCADE,
 conference_id INTEGER NOT NULL REFERENCES message_conferences(conference_id) ON DELETE CASCADE,
 placed_at INTEGER NOT NULL, local INTEGER NOT NULL, reply INTEGER NOT NULL,
 thread INTEGER NOT NULL, poster TEXT, thread_incomplete INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX conference_health_messages_window ON conference_health_messages(conference_id,placed_at);
CREATE INDEX conference_health_messages_expire ON conference_health_messages(placed_at);
CREATE TABLE conference_health_work (message_id INTEGER PRIMARY KEY);
CREATE TABLE conference_health_dirty (
 conference_id INTEGER PRIMARY KEY REFERENCES message_conferences(conference_id) ON DELETE CASCADE
);
CREATE TABLE conference_health_snapshots (
 conference_id INTEGER PRIMARY KEY REFERENCES message_conferences(conference_id) ON DELETE CASCADE,
 as_of INTEGER NOT NULL, snapshot TEXT NOT NULL
);
INSERT INTO conference_health_dirty SELECT conference_id FROM message_conferences;
CREATE TRIGGER conference_health_new_conference AFTER INSERT ON message_conferences BEGIN
 INSERT INTO conference_health_dirty SELECT NEW.conference_id WHERE NOT EXISTS(SELECT 1 FROM conference_health_dirty WHERE conference_id=NEW.conference_id); END;
CREATE TRIGGER conference_health_message_insert AFTER INSERT ON messages
 WHEN NEW.conference_id IS NOT NULL BEGIN
 INSERT OR IGNORE INTO conference_health_work VALUES(NEW.message_id); END;
CREATE TRIGGER conference_health_message_update AFTER UPDATE OF conference_id,parent_message_id,lifecycle_state,visibility,audience_kind,placed_at,origin_kind,author_caller_id ON messages BEGIN
 INSERT OR IGNORE INTO conference_health_work VALUES(NEW.message_id);
 INSERT OR IGNORE INTO conference_health_work SELECT message_id FROM messages WHERE parent_message_id=NEW.message_id;
 END;
CREATE TRIGGER conference_health_message_delete BEFORE DELETE ON messages BEGIN
 INSERT OR IGNORE INTO conference_health_dirty SELECT conference_id FROM conference_health_messages WHERE message_id=OLD.message_id;
 DELETE FROM conference_health_work WHERE message_id=OLD.message_id; END;
CREATE TRIGGER conference_health_read_insert AFTER INSERT ON caller_last_read
 WHEN NEW.reset_version=0 AND NEW.last_message_number>0 AND (SELECT enabled FROM conference_health_config)=1
 AND EXISTS(SELECT 1 FROM messages WHERE conference_id=NEW.conference_id AND message_number=NEW.last_message_number AND visibility='public' AND audience_kind='all-callers' AND lifecycle_state='active')
 BEGIN
 INSERT INTO conference_health_tokens(caller_id) SELECT NEW.caller_id WHERE NOT EXISTS(SELECT 1 FROM conference_health_tokens WHERE caller_id=NEW.caller_id);
 INSERT INTO conference_health_reads VALUES(NEW.conference_id,unixepoch()/86400,(SELECT token FROM conference_health_tokens WHERE caller_id=NEW.caller_id),1,unixepoch())
 ON CONFLICT(conference_id,day,token) DO UPDATE SET progress=progress+1,last_at=max(last_at,excluded.last_at);
 DELETE FROM conference_health_reads WHERE rowid IN
 (SELECT rowid FROM conference_health_reads
 WHERE day<unixepoch()/86400-(SELECT retention_days FROM conference_health_config)
 ORDER BY day LIMIT 1);
 INSERT INTO conference_health_dirty SELECT NEW.conference_id WHERE NOT EXISTS(SELECT 1 FROM conference_health_dirty WHERE conference_id=NEW.conference_id);
 END;
CREATE TRIGGER conference_health_read_update AFTER UPDATE OF last_message_number ON caller_last_read
 WHEN NEW.reset_version=OLD.reset_version AND NEW.last_message_number>OLD.last_message_number AND (SELECT enabled FROM conference_health_config)=1
 AND EXISTS(SELECT 1 FROM messages WHERE conference_id=NEW.conference_id AND message_number=NEW.last_message_number AND visibility='public' AND audience_kind='all-callers' AND lifecycle_state='active')
 BEGIN
 INSERT INTO conference_health_tokens(caller_id) SELECT NEW.caller_id WHERE NOT EXISTS(SELECT 1 FROM conference_health_tokens WHERE caller_id=NEW.caller_id);
 INSERT INTO conference_health_reads VALUES(NEW.conference_id,unixepoch()/86400,(SELECT token FROM conference_health_tokens WHERE caller_id=NEW.caller_id),1,unixepoch())
 ON CONFLICT(conference_id,day,token) DO UPDATE SET progress=progress+1,last_at=max(last_at,excluded.last_at);
 DELETE FROM conference_health_reads WHERE rowid IN
 (SELECT rowid FROM conference_health_reads
 WHERE day<unixepoch()/86400-(SELECT retention_days FROM conference_health_config)
 ORDER BY day LIMIT 1);
 INSERT INTO conference_health_dirty SELECT NEW.conference_id WHERE NOT EXISTS(SELECT 1 FROM conference_health_dirty WHERE conference_id=NEW.conference_id);
 END;

CREATE INDEX conference_health_native_parent ON messages(parent_message_id);
