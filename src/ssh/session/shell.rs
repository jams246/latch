#[cfg(test)]
#[path = "shell/tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Bash,
    Zsh,
    Fish,
}

/// A supported remote login shell, launched with session-only directory reporting.
#[derive(Clone, Debug)]
pub(in crate::ssh) struct Shell {
    path: String,
    kind: Kind,
}

impl Shell {
    pub(in crate::ssh) fn from_path(path: &str) -> Option<Self> {
        if !absolute_path(path) {
            return None;
        }
        let kind = match path.rsplit('/').next()? {
            "bash" => Kind::Bash,
            "zsh" => Kind::Zsh,
            "fish" => Kind::Fish,
            _ => return None,
        };
        Some(Self {
            path: path.into(),
            kind,
        })
    }

    pub(in crate::ssh) fn command(&self, directory: Option<&str>) -> String {
        let startup = match self.kind {
            Kind::Bash => include_str!("shell/bash.sh"),
            Kind::Zsh => include_str!("shell/zsh.sh"),
            Kind::Fish => include_str!("shell/fish.sh"),
        }
        .replace(
            "@COMMON@",
            &include_str!("shell/posix-hook.sh").replace(
                "@STATUS@",
                if matches!(self.kind, Kind::Zsh) {
                    "0"
                } else {
                    "\"$_latch_status\""
                },
            ),
        );
        let script = include_str!("shell/launch.sh").replace("@STARTUP@", &startup);
        let directory = directory
            .filter(|directory| absolute_path(directory))
            .unwrap_or("");
        let quote = |value: &str| quote(value, matches!(self.kind, Kind::Fish));
        format!(
            "exec /bin/sh -c {} latch-shell {} {}",
            quote(&script),
            quote(&self.path),
            quote(directory),
        )
    }
}

fn absolute_path(path: &str) -> bool {
    path.starts_with('/') && path.len() <= 4096 && !path.chars().any(char::is_control)
}

fn quote(value: &str, fish: bool) -> String {
    if fish {
        format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
