-- Retained local trust decisions; catalog payload/history is never re-signed.
CREATE TABLE circuitnet_catalog_keys (
 network TEXT NOT NULL REFERENCES circuitnet_catalog_authority(network),
 epoch INTEGER NOT NULL CHECK(epoch>0), first_revision INTEGER NOT NULL CHECK(first_revision>0), authority TEXT NOT NULL,
 transition_object TEXT, accepted_at INTEGER NOT NULL, PRIMARY KEY(network,epoch)
);
INSERT INTO circuitnet_catalog_keys SELECT network,1,1,authority,NULL,0 FROM circuitnet_catalog_authority;
DROP TRIGGER circuitnet_catalog_authority_update;
CREATE TRIGGER circuitnet_catalog_authority_update BEFORE UPDATE ON circuitnet_catalog_authority
 WHEN NEW.network != OLD.network OR NEW.authority IS NOT (SELECT authority FROM circuitnet_catalog_keys WHERE network=OLD.network ORDER BY epoch DESC LIMIT 1)
 BEGIN SELECT RAISE(ABORT,'explicit catalog key transition required'); END;
CREATE TRIGGER circuitnet_catalog_keys_update BEFORE UPDATE ON circuitnet_catalog_keys BEGIN SELECT RAISE(ABORT,'retained catalog key'); END;
CREATE TRIGGER circuitnet_catalog_keys_delete BEFORE DELETE ON circuitnet_catalog_keys BEGIN SELECT RAISE(ABORT,'retained catalog key'); END;
