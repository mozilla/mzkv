/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use skv::{
    database::{Database, GetOptions},
    key::Key,
    store::{Store, StorePath},
    value::Value,
};

fn k(s: &str) -> Key {
    Key::from(s)
}

fn v(j: serde_json::Value) -> Value {
    Value::from(j)
}

#[test]
fn in_memory_store() {
    let store = Store::new(StorePath::for_in_memory());
    let db = Database::new(&store, "test");
    db.put(&[(k("key"), Some(v(serde_json::json!("val"))))]).unwrap();
    assert_eq!(
        db.get(&k("key"), &GetOptions::default()).unwrap(),
        Some(v(serde_json::json!("val")))
    );
    store.close();
}

#[test]
fn two_in_memory_stores_are_independent() {
    let store_a = Store::new(StorePath::for_in_memory());
    let store_b = Store::new(StorePath::for_in_memory());

    Database::new(&store_a, "db")
        .put(&[(k("key"), Some(v(serde_json::json!("a"))))])
        .unwrap();

    assert_eq!(
        Database::new(&store_b, "db")
            .get(&k("key"), &GetOptions::default())
            .unwrap(),
        None
    );
}

#[test]
fn on_disk_store_persists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");

    {
        let store = Store::new(StorePath::OnDisk(path.clone()));
        Database::new(&store, "db")
            .put(&[(k("persistent"), Some(v(serde_json::json!(42))))])
            .unwrap();
        store.close();
    }

    {
        let store = Store::new(StorePath::OnDisk(path));
        assert_eq!(
            Database::new(&store, "db")
                .get(&k("persistent"), &GetOptions::default())
                .unwrap(),
            Some(v(serde_json::json!(42)))
        );
        store.close();
    }
}

#[test]
fn on_disk_store_creates_wal_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");

    let store = Store::new(StorePath::OnDisk(path.clone()));
    Database::new(&store, "db")
        .put(&[(k("k"), Some(v(serde_json::json!("v"))))])
        .unwrap();

    let store_path = StorePath::OnDisk(path);
    let on_disk = store_path.on_disk().unwrap();
    assert!(on_disk.wal().exists(), "WAL file should exist after write");
    assert!(on_disk.shm().exists(), "SHM file should exist after write");

    store.close();
}

#[test]
fn storage_dir_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = StorePath::for_storage_dir(dir.path());
    assert!(path.on_disk().is_some());
}

#[test]
fn canonicalizing_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite");
    std::fs::write(&path, b"").unwrap();
    let store_path = StorePath::canonicalizing(path).expect("canonicalizing should succeed");
    assert!(store_path.on_disk().is_some());
}
