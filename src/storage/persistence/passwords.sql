BEGIN IMMEDIATE;
CREATE TABLE passwords (
    connection_id TEXT PRIMARY KEY NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    password TEXT NOT NULL
);
PRAGMA user_version=2;
COMMIT;
