use std::path::{Component, Path};

use russh_sftp::client::SftpSession;
use tokio::{fs, sync::watch};

use super::{
    Direction, Error,
    operations::{child, read_directory, validate_path},
    transfer::wait_running,
    transfer_destination::local_is_link,
};

pub(super) struct Node {
    pub direction: Direction,
    pub children: Option<Vec<Node>>,
    pub total: Option<u64>,
}

pub(super) async fn plan(
    session: &SftpSession,
    directions: Vec<Direction>,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
) -> Result<Vec<Node>, Error> {
    let mut nodes = Vec::new();
    for direction in directions {
        nodes.push(build(session, direction, cancel, paused).await?);
    }
    Ok(nodes)
}

async fn build(
    session: &SftpSession,
    direction: Direction,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
) -> Result<Node, Error> {
    wait_running(cancel, paused).await?;
    let mut children = Vec::new();
    let (is_dir, total) = match &direction {
        Direction::Upload { local, remote } => {
            validate_path(remote)?;
            let metadata = fs::symlink_metadata(local).await?;
            if local_is_link(&metadata) {
                return Err(Error::UnsupportedCopySource);
            }
            if metadata.is_file() {
                (false, Some(metadata.len()))
            } else if metadata.is_dir() {
                let mut entries = fs::read_dir(local).await?;
                while let Some(entry) = entries.next_entry().await? {
                    wait_running(cancel, paused).await?;
                    let name = entry.file_name();
                    children.push(Direction::Upload {
                        local: entry.path(),
                        remote: child(remote, name.to_str().ok_or(Error::InvalidPath)?)?,
                    });
                }
                (true, None)
            } else {
                return Err(Error::UnsupportedCopySource);
            }
        }
        Direction::Download { remote, local } => {
            validate_path(remote)?;
            if local.file_name().is_none() {
                return Err(Error::InvalidPath);
            }
            let metadata = session.symlink_metadata(remote).await?;
            if metadata.file_type().is_file() {
                (false, metadata.size)
            } else if metadata.file_type().is_dir() {
                for entry in read_directory(session, remote).await? {
                    wait_running(cancel, paused).await?;
                    let name = entry.file_name();
                    validate_local_name(&name)?;
                    children.push(Direction::Download {
                        remote: child(remote, &name)?,
                        local: local.join(name),
                    });
                }
                (true, None)
            } else {
                return Err(Error::UnsupportedCopySource);
            }
        }
    };
    let mut nodes = Vec::new();
    for child in children {
        nodes.push(Box::pin(build(session, child, cancel, paused)).await?);
    }
    Ok(Node {
        direction,
        total: if is_dir { total_size(&nodes) } else { total },
        children: is_dir.then_some(nodes),
    })
}

pub(super) fn total_size(nodes: &[Node]) -> Option<u64> {
    nodes
        .iter()
        .try_fold(0_u64, |sum, node| sum.checked_add(node.total?))
}

fn validate_local_name(name: &str) -> Result<(), Error> {
    super::operations::validate_name(name)?;
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(Error::InvalidPath);
    }
    #[cfg(windows)]
    {
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        let numbered = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"));
        if name.contains([':', '<', '>', '"', '|', '?', '*'])
            || name.ends_with([' ', '.'])
            || name.chars().any(|character| character.is_control())
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || numbered.is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        {
            return Err(Error::InvalidPath);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_child_names_cannot_escape_local_destination() {
        for name in [
            "",
            ".",
            "..",
            "/absolute",
            "../escape",
            "a/b",
            "a\\b",
            "nul\0",
        ] {
            assert!(validate_local_name(name).is_err(), "Accepted {name:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn server_child_names_reject_windows_devices_and_aliases() {
        for name in [
            "CON",
            "nul.txt",
            "COM1.txt",
            "LPT9",
            "COM¹",
            "NUL .txt",
            "CONOUT$",
            "file:stream",
            "trailing.",
            "trailing ",
        ] {
            assert!(validate_local_name(name).is_err(), "Accepted {name:?}");
        }
    }
}
