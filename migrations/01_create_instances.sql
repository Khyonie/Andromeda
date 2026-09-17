CREATE TABLE instances (
    id           TEXT PRIMARY KEY NOT NULL,
    hostname     TEXT NOT NULL UNIQUE,

    memory_mib   INTEGER NOT NULL
                 CHECK(memory_mib > 0),

    vcpus        INTEGER NOT NULL
                 CHECK(vcpus > 0),

    mac_address  TEXT NOT NULL UNIQUE,
    ipv4_address TEXT UNIQUE,
    remote_port  INTEGER NOT NULL UNIQUE
                 CHECK(remote_port > 0 AND remote_port < 65536),
    service_port INTEGER NOT NULL UNIQUE
                 CHECK(service_port > 0 AND service_port < 65536)
);
