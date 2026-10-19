use std::{io, path::PathBuf};

use rusqlite::{Connection as Database, Row, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::storage::{AuthMethod, Configuration, Connection, WindowSize};

use super::{database::error, invalid};

pub(super) fn read(database: &Database) -> io::Result<Configuration> {
    let mut configuration = database.query_row(
        "SELECT sort_mode,terminal_theme,window_width,window_height,maximized FROM preferences WHERE id=1",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?,
            row.get::<_, Option<f32>>(2)?, row.get::<_, Option<f32>>(3)?, row.get::<_, Option<i64>>(4)?)),
    ).map_err(error).and_then(|(sort, theme, width, height, maximized)| {
        let window_size = match (width, height, maximized) {
            (None, None, None) => None,
            (Some(width), Some(height), Some(maximized @ (0 | 1))) => Some(WindowSize { width, height, maximized: maximized == 1 }),
            _ => return Err(invalid()),
        };
        Ok(Configuration { sort_mode: decode(sort)?, terminal_theme: decode(theme)?, window_size, ..Configuration::default() })
    })?;
    let mut statement = database.prepare("SELECT id,name,host,port,username,auth_method,key_path,successful_shells FROM connections ORDER BY position").map_err(error)?;
    let mut rows = statement.query([]).map_err(error)?;
    while let Some(row) = rows.next().map_err(error)? {
        configuration.connections.push(read_connection(row)?);
    }
    let mut statement = database
        .prepare("SELECT connection_id,path FROM local_directories")
        .map_err(error)?;
    let mut rows = statement.query([]).map_err(error)?;
    while let Some(row) = rows.next().map_err(error)? {
        configuration.local_directories.insert(
            row.get(0).map_err(error)?,
            PathBuf::from(row.get::<_, String>(1).map_err(error)?),
        );
    }
    let mut statement = database
        .prepare("SELECT connection_id,password FROM passwords")
        .map_err(error)?;
    let mut rows = statement.query([]).map_err(error)?;
    while let Some(row) = rows.next().map_err(error)? {
        configuration.passwords.insert(
            row.get(0).map_err(error)?,
            zeroize::Zeroizing::new(row.get(1).map_err(error)?),
        );
    }
    Ok(configuration)
}

fn read_connection(row: &Row<'_>) -> io::Result<Connection> {
    let method: String = row.get(5).map_err(error)?;
    let path: Option<String> = row.get(6).map_err(error)?;
    let auth = match (method.as_str(), path) {
        ("password", None) => AuthMethod::Password,
        ("agent", None) => AuthMethod::Agent,
        ("keyboard_interactive", None) => AuthMethod::KeyboardInteractive,
        ("key_file", Some(path)) => AuthMethod::KeyFile { path: path.into() },
        _ => return Err(invalid()),
    };
    let count: Vec<u8> = row.get(7).map_err(error)?;
    let count: [u8; 8] = count.try_into().map_err(|_| invalid())?;
    Ok(Connection {
        id: row.get(0).map_err(error)?,
        name: row.get(1).map_err(error)?,
        host: row.get(2).map_err(error)?,
        port: row.get(3).map_err(error)?,
        username: row.get(4).map_err(error)?,
        auth,
        successful_shells: u64::from_be_bytes(count),
    })
}

pub(super) fn write(database: &mut Database, configuration: &Configuration) -> io::Result<()> {
    let transaction = database
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(error)?;
    transaction
        .execute_batch(
            "DELETE FROM local_directories; DELETE FROM connections; DELETE FROM preferences;",
        )
        .map_err(error)?;
    transaction
        .execute(
            "INSERT INTO preferences VALUES(1,?1,?2,?3,?4,?5)",
            params![
                encode(&configuration.sort_mode)?,
                encode(&configuration.terminal_theme)?,
                configuration.window_size.map(|size| size.width),
                configuration.window_size.map(|size| size.height),
                configuration.window_size.map(|size| size.maximized),
            ],
        )
        .map_err(error)?;
    for (position, connection) in configuration.connections.iter().enumerate() {
        let (method, path) = match &connection.auth {
            AuthMethod::Password => ("password", None),
            AuthMethod::Agent => ("agent", None),
            AuthMethod::KeyboardInteractive => ("keyboard_interactive", None),
            AuthMethod::KeyFile { path } => ("key_file", Some(path.to_str().ok_or_else(invalid)?)),
        };
        let position: i64 = position.try_into().map_err(|_| invalid())?;
        transaction
            .execute(
                "INSERT INTO connections VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    connection.id,
                    position,
                    connection.name,
                    connection.host,
                    connection.port,
                    connection.username,
                    method,
                    path,
                    connection.successful_shells.to_be_bytes().as_slice(),
                ],
            )
            .map_err(error)?;
    }
    for (id, path) in &configuration.local_directories {
        transaction
            .execute(
                "INSERT INTO local_directories VALUES(?1,?2)",
                params![id, path.to_str().ok_or_else(invalid)?],
            )
            .map_err(error)?;
    }
    for (id, password) in &configuration.passwords {
        transaction
            .execute(
                "INSERT INTO passwords VALUES(?1,?2)",
                params![id, password.as_str()],
            )
            .map_err(error)?;
    }
    transaction.commit().map_err(error)
}

fn encode(value: &impl Serialize) -> io::Result<String> {
    serde_json::to_value(value)
        .map_err(|_| invalid())?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(invalid)
}

fn decode<T: DeserializeOwned>(value: String) -> io::Result<T> {
    serde_json::from_value(serde_json::Value::String(value)).map_err(|_| invalid())
}
