-- SPITFIRE NG: native CircuitNET live-link configuration and bounded health.
CREATE TABLE circuitnet_live (
 network TEXT PRIMARY KEY REFERENCES circuitnet_profiles(network),
 configuration TEXT NOT NULL,
 version INTEGER NOT NULL CHECK(version > 0)
);
CREATE TABLE circuitnet_link_health (
 network TEXT NOT NULL REFERENCES circuitnet_live(network),
 neighbor TEXT NOT NULL,
 observation TEXT NOT NULL,
 PRIMARY KEY(network,neighbor)
);
