use super::{actions::Target, *};

impl Files {
    pub(super) fn copy_targets(&mut self, targets: Vec<Target>, cx: &mut Context<Self>) {
        if !self.actions_idle() || targets.is_empty() {
            return;
        }
        let result = self
            .local
            .as_ref()
            .zip(self.directory.as_ref())
            .ok_or_else(|| "Open both directories before copying".to_string())
            .and_then(|(local, remote)| copy_directions(targets, &local.path, &remote.path));
        let directions = match result {
            Ok(directions) => directions,
            Err(error) => {
                self.error = Some(format!("Cannot copy selected items: {error}"));
                cx.notify();
                return;
            }
        };
        let Some(browser) = &self.browser else {
            self.error = Some("Refresh the server directory to reconnect before copying".into());
            cx.notify();
            return;
        };
        let (events, progress) = tokio::sync::mpsc::channel(32);
        let _guard = self.runtime.enter();
        self.transfer = Some(browser.transfer_many_interactive(directions, events));
        self.metrics = Some(operations::Metrics::new(std::time::Instant::now()));
        self.progress = Some(progress);
        cx.notify();
    }
}

fn copy_directions(
    targets: Vec<Target>,
    local_parent: &std::path::Path,
    remote_parent: &str,
) -> Result<Vec<Direction>, String> {
    targets
        .into_iter()
        .map(|target| match target {
            Target::Local(local) => {
                let name = local
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        "The selected local filename cannot be represented on this server"
                            .to_string()
                    })?;
                Ok(Direction::Upload {
                    remote: format!("{}/{name}", remote_parent.trim_end_matches('/')),
                    local,
                })
            }
            Target::Remote(remote) => {
                let name = remote.rsplit('/').next().unwrap_or_default();
                local::validate_name(name).map_err(|error| error.to_string())?;
                Ok(Direction::Download {
                    local: local_parent.join(name),
                    remote,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Target, copy_directions};
    use latch::sftp::Direction;
    use std::path::Path;

    #[test]
    fn copies_use_opposite_panes_directory_without_prompting() {
        let upload = copy_directions(
            vec![Target::Local("C:/local/file.txt".into())],
            Path::new("C:/target"),
            "/remote",
        )
        .unwrap();
        assert!(
            matches!(&upload[0], Direction::Upload {remote,..} if remote == "/remote/file.txt")
        );
        let download = copy_directions(
            vec![Target::Remote("/remote/folder".into())],
            Path::new("C:/target"),
            "/remote",
        )
        .unwrap();
        assert!(
            matches!(&download[0], Direction::Download {local,..} if local == Path::new("C:/target/folder"))
        );
    }

    #[test]
    fn remote_names_cannot_escape_or_alias_local_targets() {
        for name in ["..", "C:secret", "CON", "file:stream", "file\\escape"] {
            assert!(
                copy_directions(
                    vec![Target::Remote(format!("/remote/{name}"))],
                    Path::new("C:/target"),
                    "/remote"
                )
                .is_err()
            );
        }
    }
}
