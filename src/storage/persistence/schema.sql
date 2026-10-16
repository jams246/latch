BEGIN IMMEDIATE;
CREATE TABLE preferences (
    id INTEGER PRIMARY KEY CHECK (id=1),
    sort_mode TEXT NOT NULL,
    terminal_theme TEXT NOT NULL,
    window_width REAL,
    window_height REAL,
    maximized INTEGER CHECK (maximized IN (0,1)),
    CHECK ((window_width IS NULL AND window_height IS NULL AND maximized IS NULL)
        OR (window_width IS NOT NULL AND window_height IS NOT NULL AND maximized IS NOT NULL))
);
CREATE TABLE connections (
    id TEXT PRIMARY KEY NOT NULL,
    position INTEGER NOT NULL UNIQUE CHECK (position>=0),
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    username TEXT NOT NULL,
    auth_method TEXT NOT NULL CHECK (auth_method IN ('password','key_file','agent','keyboard_interactive')),
    key_path TEXT,
    successful_shells BLOB NOT NULL CHECK (length(successful_shells)=8),
    CHECK ((auth_method='key_file' AND key_path IS NOT NULL)
        OR (auth_method<>'key_file' AND key_path IS NULL))
);
CREATE TABLE local_directories (
    connection_id TEXT PRIMARY KEY NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    path TEXT NOT NULL
);
PRAGMA user_version=1;
COMMIT;
