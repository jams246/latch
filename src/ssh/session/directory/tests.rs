use super::Directory;

fn report(directory: &mut Directory, uri: &[u8]) {
    directory.feed(b"\x1b]7;");
    directory.feed(uri);
    directory.feed(b"\x07");
}

#[test]
fn directory_reports_survive_every_chunk_boundary_for_both_terminators() {
    for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
        let mut sequence = b"\x1b]7;file://remote.example/work/a%20b".to_vec();
        sequence.extend_from_slice(terminator);
        for split in 0..sequence.len() {
            let mut directory = Directory::default();
            directory.feed(&sequence[..split]);
            assert_eq!(directory.path(), None);
            directory.feed(&sequence[split..]);
            assert_eq!(directory.path(), Some("/work/a b"));
        }
    }
}

#[test]
fn newest_directory_replaces_previous_report_and_ignores_authority() {
    let mut directory = Directory::default();
    report(&mut directory, b"file://first.example/first");
    report(&mut directory, b"file://unrelated.example/second");
    assert_eq!(directory.path(), Some("/second"));
}

#[test]
fn percent_decoding_happens_once_and_preserves_shell_metacharacters_and_unicode() {
    let mut directory = Directory::default();
    report(
        &mut directory,
        "file:///work/Jason's%20files;λ/%252F/%23%3f".as_bytes(),
    );
    assert_eq!(directory.path(), Some("/work/Jason's files;λ/%2F/#?"));
}

#[test]
fn invalid_directory_reports_preserve_the_last_valid_path() {
    for invalid in [
        b"https://remote.example/path".as_slice(),
        b"file:relative",
        b"file://remote.example",
        b"file:///path?query",
        b"file:///path#fragment",
        b"file:///trailing%",
        b"file:///short%2",
        b"file:///invalid%GG",
        b"file:///nul%00byte",
        b"file:///newline%0Abyte",
        b"file:///delete%7fbyte",
        b"file:///control%C2%85byte",
        b"file:///bad%FFutf8",
        b"file:///bad\xffutf8",
        b"file://bad\xffhost/path",
        b"file:///raw\nnewline",
        b"file:///raw\0nul",
        b"file://host%00/path",
        b"f%69le:///wrong-scheme",
    ] {
        let mut directory = Directory::default();
        report(&mut directory, b"file:///kept");
        report(&mut directory, invalid);
        assert_eq!(directory.path(), Some("/kept"), "{invalid:?}");
    }
}

#[test]
fn incomplete_st_and_invalid_osc_termination_do_not_replace_directory() {
    for ending in [b"X".as_slice(), b"\n\\", b"(\\", b"\x18\\"] {
        let mut directory = Directory::default();
        report(&mut directory, b"file:///kept");
        directory.feed(b"\x1b]7;file:///rejected\x1b");
        assert_eq!(directory.path(), Some("/kept"));
        directory.feed(ending);
        assert_eq!(directory.path(), Some("/kept"));
    }
}

#[test]
fn reconnect_parser_reset_retains_directory_but_discards_partial_sequences() {
    let mut directory = Directory::default();
    report(&mut directory, b"file:///kept");
    directory.feed(b"\x1b]7;file:///partial\x1b");
    directory.reset_parser();
    directory.feed(b"\\");
    assert_eq!(directory.path(), Some("/kept"));
    report(&mut directory, b"file:///new");
    assert_eq!(directory.path(), Some("/new"));
}

#[test]
fn tabs_remember_directories_independently() {
    let mut first = Directory::default();
    let mut second = Directory::default();
    report(&mut first, b"file:///one");
    report(&mut second, b"file:///two");
    first.reset_parser();
    assert_eq!(first.path(), Some("/one"));
    assert_eq!(second.path(), Some("/two"));
}

#[test]
fn oversized_reports_and_excess_parameters_are_rejected_without_truncation() {
    let mut directory = Directory::default();
    report(&mut directory, b"file:///kept");
    directory.feed(b"\x1b]7;file:///");
    for _ in 0..32 {
        directory.feed(&[b'x'; 1024]);
    }
    directory.feed(b"\x07");
    assert_eq!(directory.path(), Some("/kept"));
    report(&mut directory, b"file:///a;b;c;d;e;f;g;h;i;j;k;l;m;n;o;p;q");
    assert_eq!(directory.path(), Some("/kept"));
    report(&mut directory, b"file:///recovered");
    assert_eq!(directory.path(), Some("/recovered"));
}

#[test]
fn unrelated_terminal_sequences_do_not_set_directory() {
    let mut directory = Directory::default();
    directory.feed(b"output\r\n\x1b[32mgreen\x1b[0m\x1b]0;file:///title\x07");
    assert_eq!(directory.path(), None);
    report(&mut directory, b"file:///real");
    assert_eq!(directory.path(), Some("/real"));
}
