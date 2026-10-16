use super::*;

fn entry(name: &str, is_dir: bool) -> Entry {
    Entry {
        name: name.into(),
        path: name.into(),
        is_dir,
        is_link: false,
        size: Some(1),
        modified: Some(0),
    }
}

#[test]
fn ascending_name_sort_keeps_folders_before_alphabetically_earlier_files() {
    let mut entries = [
        entry("aardvark.txt", false),
        entry("zeta", true),
        entry("middle.log", false),
        entry("omega", true),
    ];
    entries.sort_by(|left, right| Sort::default().compare(left, right));
    assert_eq!(
        entries.map(|entry| entry.name),
        ["omega", "zeta", "aardvark.txt", "middle.log"]
    );
}

#[test]
fn descending_name_sort_reverses_names_within_folders_and_files() {
    let sort = Sort {
        column: Column::Name,
        descending: true,
    };
    let mut entries = [
        entry("aardvark.txt", false),
        entry("omega", true),
        entry("middle.log", false),
        entry("zeta", true),
    ];
    entries.sort_by(|left, right| sort.compare(left, right));
    assert_eq!(
        entries.map(|entry| entry.name),
        ["zeta", "omega", "middle.log", "aardvark.txt"]
    );
}

#[test]
fn name_sort_preserves_case_insensitive_order_and_deterministic_ties() {
    let mut entries = [
        entry("beta", false),
        entry("alpha", false),
        entry("Alpha", false),
        entry("ALPHA", false),
    ];
    entries.sort_by(|left, right| Sort::default().compare(left, right));
    assert_eq!(
        entries.map(|entry| entry.name),
        ["ALPHA", "Alpha", "alpha", "beta"]
    );
}

#[test]
fn directory_link_sorts_with_folders_while_preserving_link_type() {
    let mut link = entry("zeta", true);
    link.is_link = true;
    assert_eq!(
        Sort::default().compare(&link, &entry("alpha", false)),
        Ordering::Less
    );
    assert_eq!(entry_type(&link), "Link");
}

#[test]
fn modified_sort_does_not_group_folders_before_files() {
    let sort = Sort {
        column: Column::Modified,
        descending: false,
    };
    let mut folder = entry("alpha", true);
    folder.modified = Some(10);
    assert_eq!(
        sort.compare(&folder, &entry("zeta", false)),
        Ordering::Greater
    );
}
