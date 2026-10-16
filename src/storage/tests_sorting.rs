use super::{AuthMethod, Connection, SortMode, tests_persistence::store};

fn connection(id: &str, name: &str) -> Connection {
    let mut connection = Connection::new(
        name.into(),
        "localhost".into(),
        "synthetic-user".into(),
        AuthMethod::Password,
    );
    connection.id = id.into();
    connection
}

#[test]
fn all_modes_keep_zero_through_five_connections_visible() {
    for count in 0..=5 {
        let mut store = store("sizes");
        for index in (0..count).rev() {
            store
                .upsert(connection(&format!("id-{index}"), &format!("Name {index}")))
                .unwrap();
        }
        for mode in [
            SortMode::Name,
            SortMode::MostUsed,
            SortMode::FrequentAndName,
        ] {
            store.set_sort_mode(mode).unwrap();
            let actual: Vec<_> = store.sorted().iter().map(|item| item.id.clone()).collect();
            let expected: Vec<_> = (0..count).map(|index| format!("id-{index}")).collect();
            assert_eq!(actual, expected);
            assert_eq!(store.frequent().len(), count.min(3));
        }
    }
}

#[test]
fn usage_sort_descends_then_breaks_ties_by_name_and_stable_id() {
    let mut store = store("ties");
    for (id, name) in [
        ("b", "alpha"),
        ("c", "Zulu"),
        ("a", "Alpha"),
        ("d", "Never"),
    ] {
        store.upsert(connection(id, name)).unwrap();
    }
    for id in ["c", "c", "b", "a"] {
        store.record_successful_shell(id).unwrap();
    }
    store.set_sort_mode(SortMode::MostUsed).unwrap();
    let ids: Vec<_> = store.sorted().iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["c", "a", "b", "d"]);
}

#[test]
fn frequent_section_keeps_intentional_duplicates_in_full_alphabetical_list() {
    let mut store = store("duplicates");
    for (id, name) in [("a", "Alpha"), ("b", "Beta"), ("c", "Gamma"), ("d", "Zulu")] {
        store.upsert(connection(id, name)).unwrap();
    }
    store.record_successful_shell("d").unwrap();
    store.set_sort_mode(SortMode::FrequentAndName).unwrap();
    let full: Vec<_> = store.sorted().iter().map(|item| item.id.as_str()).collect();
    let frequent: Vec<_> = store
        .frequent()
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        (frequent, full),
        (vec!["d", "a", "b"], vec!["a", "b", "c", "d"])
    );
}

#[test]
fn rename_retains_usage_and_delete_removes_it() {
    let mut store = store("rename-delete");
    store.upsert(connection("a", "Before")).unwrap();
    store.record_successful_shell("a").unwrap();
    let mut edited = connection("a", "After");
    edited.successful_shells = 50;
    store.upsert(edited).unwrap();
    assert_eq!(store.connections()[0].successful_shells, 1);
    store.delete("a").unwrap();
    store.upsert(connection("a", "Recreated")).unwrap();
    assert_eq!(store.connections()[0].successful_shells, 0);
}

#[test]
fn only_explicit_success_events_increment_usage() {
    let mut store = store("success-only");
    store.upsert(connection("a", "Server")).unwrap();
    store.set_sort_mode(SortMode::MostUsed).unwrap();
    assert_eq!(store.connections()[0].successful_shells, 0);
    store.record_successful_shell("a").unwrap();
    assert_eq!(store.connections()[0].successful_shells, 1);
    assert!(store.record_successful_shell("missing").is_err());
}
