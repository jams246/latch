use super::PasswordUpdate;
use latch::{
    ssh,
    storage::{Connection, Store},
};

#[test]
fn rejected_attempt_cannot_clear_a_newer_password_or_changed_account() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/saved-passwords/rejected-attempts")
        .join(uuid::Uuid::new_v4().to_string());
    let mut store = Store::load(directory.join("latch.db")).unwrap();
    let original = Connection::new(
        "Synthetic".into(),
        "example.invalid".into(),
        "synthetic".into(),
        latch::storage::AuthMethod::Password,
    );
    store.upsert(original.clone()).unwrap();
    let new_password = ssh::Secret::new("new-synthetic-password".into());
    store
        .save_password(&original, new_password.clone())
        .unwrap();
    PasswordUpdate::Rejected(
        original.clone(),
        ssh::Secret::new("old-synthetic-password".into()),
    )
    .apply(&mut store)
    .unwrap();
    assert!(store.saved_password(&original) == Some(&new_password));
    let mut edited = original.clone();
    edited.host = "other.invalid".into();
    store.upsert(edited.clone()).unwrap();
    store.save_password(&edited, new_password.clone()).unwrap();
    PasswordUpdate::Rejected(original, new_password.clone())
        .apply(&mut store)
        .unwrap();
    assert!(store.saved_password(&edited) == Some(&new_password));
    PasswordUpdate::Rejected(edited.clone(), new_password)
        .apply(&mut store)
        .unwrap();
    assert!(store.saved_password(&edited).is_none());
}
