mod database;
mod key;
mod rows;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_parallel;

use std::{fs, io, path::Path};

use super::Configuration;

pub(super) fn read(path: &Path) -> io::Result<Configuration> {
    if !pair_exists(path)? {
        return Ok(Configuration::default());
    }
    let connection = database::open(path)?;
    let configuration = rows::read(&connection)?;
    super::validate(&configuration)?;
    Ok(configuration)
}

pub(super) fn write(path: &Path, configuration: &Configuration) -> io::Result<()> {
    super::validate(configuration)?;
    let existing = pair_exists(path)?;
    if !existing {
        fs::create_dir_all(
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?;
        key::create(&path.with_file_name("key.dpapi"))?;
    }
    let mut connection = database::open_for_write(path, !existing)?;
    rows::write(&mut connection, configuration)
}

fn pair_exists(path: &Path) -> io::Result<bool> {
    let exists = |path: &Path| match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(invalid()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    };
    match (exists(path)?, exists(&path.with_file_name("key.dpapi"))?) {
        (false, false) => Ok(false),
        (true, true) => Ok(true),
        _ => Err(invalid()),
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Encrypted settings are incomplete, damaged, or unavailable to this Windows account; original files were preserved",
    )
}
