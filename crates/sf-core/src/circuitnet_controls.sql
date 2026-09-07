-- C4 stores routing and control authority, never conference message payloads.
ALTER TABLE circuitnet_messages ADD COLUMN destination TEXT;
CREATE TABLE circuitnet_destinations (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network),
 message_id INTEGER NOT NULL REFERENCES messages(message_id), destination TEXT NOT NULL,
 accepted INTEGER NOT NULL CHECK(accepted IN(0,1)),
 PRIMARY KEY(network,message_id)
);
CREATE TRIGGER circuitnet_destinations_update BEFORE UPDATE ON circuitnet_destinations BEGIN SELECT RAISE(ABORT,'retained CircuitNET destination'); END;
CREATE TRIGGER circuitnet_destinations_delete BEFORE DELETE ON circuitnet_destinations BEGIN SELECT RAISE(ABORT,'retained CircuitNET destination'); END;
CREATE TABLE circuitnet_control_policy (
 network TEXT PRIMARY KEY REFERENCES circuitnet_profiles(network),
 policy TEXT NOT NULL CHECK(policy IN('require-approval','auto-approve','deny')),
 version INTEGER NOT NULL CHECK(version>0)
);
CREATE TABLE circuitnet_controls (
 network TEXT NOT NULL REFERENCES circuitnet_profiles(network), identity TEXT NOT NULL,
 direction TEXT NOT NULL CHECK(direction IN('incoming','outgoing')),
 neighbor TEXT NOT NULL, request TEXT NOT NULL, fingerprint TEXT NOT NULL,
 result TEXT NOT NULL, settled INTEGER NOT NULL CHECK(settled IN(0,1)),
 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
 PRIMARY KEY(network,identity,direction)
);
CREATE TRIGGER circuitnet_controls_identity BEFORE UPDATE ON circuitnet_controls
 WHEN NEW.network!=OLD.network OR NEW.identity!=OLD.identity OR NEW.direction!=OLD.direction
 OR NEW.neighbor!=OLD.neighbor OR NEW.request!=OLD.request OR NEW.fingerprint!=OLD.fingerprint
 OR NEW.created_at!=OLD.created_at
 BEGIN SELECT RAISE(ABORT,'retained CircuitNET control identity'); END;
CREATE TRIGGER circuitnet_controls_delete BEFORE DELETE ON circuitnet_controls BEGIN SELECT RAISE(ABORT,'retained CircuitNET control'); END;
