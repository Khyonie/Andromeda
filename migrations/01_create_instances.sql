CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    discord_id TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    avatar TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE INDEX sessions_expiry ON sessions(expires_at);

CREATE TABLE oauth_states (
    state_hash TEXT PRIMARY KEY NOT NULL,
    browser_hash TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX oauth_states_expiry ON oauth_states(expires_at);

CREATE TABLE instances (
    id           TEXT PRIMARY KEY NOT NULL,
    hostname     TEXT NOT NULL UNIQUE,
    owner_id     TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,

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

CREATE INDEX instances_owner ON instances(owner_id);
CREATE TABLE instance_members (
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('operator', 'viewer')),
    PRIMARY KEY (instance_id, user_id)
);
CREATE INDEX instance_members_user ON instance_members(user_id);

-- Invitation storage is reserved for the next phase; no invitation endpoints yet.
CREATE TABLE instance_invites (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    invited_by TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_discord_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('operator', 'viewer')),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at INTEGER NOT NULL,
    accepted_at INTEGER,
    revoked_at INTEGER
);
