#[cfg(test)]
use super::*;

#[test]
fn only_bounded_absolute_supported_shell_paths_are_accepted() {
    for path in ["/bin/bash", "/usr/bin/zsh", "/opt/my shell/fish"] {
        assert!(Shell::from_path(path).is_some(), "{path}");
    }
    for path in [
        "bash",
        "/bin/sh",
        "/bin/rbash",
        "/bin/bash\n",
        "/bin/fish\0",
        "/bin/bash/",
    ] {
        assert!(Shell::from_path(path).is_none(), "{path:?}");
    }
    assert!(Shell::from_path(&format!("/{}bash", "x".repeat(4096))).is_none());
}

#[test]
fn startup_is_controlled_and_paths_are_shell_quoted() {
    for path in ["/bin/bash", "/bin/zsh", "/bin/fish"] {
        let command = Shell::from_path(path)
            .unwrap()
            .command(Some("/srv/a'\"$()`;b\\c"));
        assert!(command.starts_with("exec /bin/sh -c "));
        assert!(!command.contains("@COMMON@"));
        assert!(!command.contains("@STARTUP@"));
        assert!(!command.contains("@STATUS@"));
        assert!(command.contains("latch-shell"));
        assert!(command.contains("_latch_report_cwd"));
    }
}

#[test]
fn invalid_restore_directory_is_discarded() {
    let shell = Shell::from_path("/bin/bash").unwrap();
    assert_eq!(shell.command(Some("relative")), shell.command(None));
    assert_eq!(shell.command(Some("/bad\npath")), shell.command(None));
}
