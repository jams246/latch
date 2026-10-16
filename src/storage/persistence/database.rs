use std::{fmt::Write, io, path::Path, time::Duration};

use rusqlite::{Connection, OpenFlags};
use zeroize::Zeroizing;

use super::{invalid, key};

pub(super) fn open(path: &Path) -> io::Result<Connection> {
    open_for_write(path, false)
}

pub(super) fn open_for_write(path: &Path, create: bool) -> io::Result<Connection> {
    let key = key::read(&path.with_file_name("key.dpapi"))?;
    let mut statement = Zeroizing::new(String::from("PRAGMA key = \"x'"));
    for byte in key.iter() {
        write!(statement, "{byte:02x}").map_err(|_| invalid())?;
    }
    statement.push_str("'\";");
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let connection = Connection::open_with_flags(path, flags).map_err(error)?;
    // SQLCipher's Windows allocator recursively logs allocation failures through itself.
    connection
        .execute_batch("PRAGMA cipher_log='off'; PRAGMA cipher_memory_security=ON;")
        .map_err(error)?;
    connection.execute_batch(&statement).map_err(error)?;
    let cipher: String = connection
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .map_err(error)?;
    if cipher.is_empty() {
        return Err(invalid());
    }
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(error)?;
    connection
        .busy_timeout(Duration::from_millis(250))
        .map_err(error)?;
    connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA temp_store=MEMORY;")
        .map_err(error)?;
    if create {
        connection
            .execute_batch(include_str!("schema.sql"))
            .map_err(error)?;
    }
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(error)?;
    if version != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported encrypted settings schema",
        ));
    }
    integrity(&connection)?;
    connection
        .execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA;")
        .map_err(error)?;
    Ok(connection)
}

fn integrity(connection: &Connection) -> io::Result<()> {
    let mut statement = connection
        .prepare("PRAGMA cipher_integrity_check")
        .map_err(error)?;
    if statement
        .query([])
        .map_err(error)?
        .next()
        .map_err(error)?
        .is_some()
    {
        return Err(invalid());
    }
    let mut statement = connection.prepare("PRAGMA quick_check").map_err(error)?;
    let mut rows = statement.query([]).map_err(error)?;
    let Some(row) = rows.next().map_err(error)? else {
        return Err(invalid());
    };
    if row.get::<_, String>(0).map_err(error)? != "ok" || rows.next().map_err(error)?.is_some() {
        return Err(invalid());
    }
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(error)?;
    if statement
        .query([])
        .map_err(error)?
        .next()
        .map_err(error)?
        .is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn error(error: rusqlite::Error) -> io::Error {
    if let rusqlite::Error::SqliteFailure(code, _) = &error
        && matches!(
            code.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        )
    {
        return io::Error::new(
            io::ErrorKind::WouldBlock,
            "Encrypted settings are locked; no changes were saved",
        );
    }
    invalid()
}
