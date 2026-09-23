CREATE TABLE system_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    gateway_ip TEXT NOT NULL,
    sysadmin_ssh_key TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0)
);
