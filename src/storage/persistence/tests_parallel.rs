use std::{fs, process::Command, sync::Barrier, thread};

use super::{Configuration, database, key, rows};
use crate::storage::{SortMode, tests_persistence::path};

#[test]
fn parallel_database_opens_preserve_security_and_contents() {
    const CHILD: &str = "LATCH_PARALLEL_DATABASE_TEST";
    if std::env::var_os(CHILD).is_none() {
        // A fresh process exercises SQLCipher's first global initialization.
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::persistence::tests_parallel::parallel_database_opens_preserve_security_and_contents",
            ])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success(), "parallel database child failed: {status}");
        return;
    }

    let barrier = Barrier::new(16);
    thread::scope(|scope| {
        for _ in 0..16 {
            scope.spawn(|| {
                let path = path("parallel-open");
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                key::create(&path.with_file_name("key.dpapi")).unwrap();
                let expected = Configuration {
                    sort_mode: SortMode::MostUsed,
                    ..Configuration::default()
                };
                barrier.wait();
                for round in 0..3 {
                    let mut connection = database::open_for_write(&path, round == 0).unwrap();
                    rows::write(&mut connection, &expected).unwrap();
                    assert!(rows::read(&connection).unwrap() == expected);
                    let secured: String = connection
                        .query_row("PRAGMA cipher_memory_security", [], |row| row.get(0))
                        .unwrap();
                    assert_eq!(secured, "1");
                }
            });
        }
    });
}
