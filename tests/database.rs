/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use skv::{
    database::{Database, GetOptions},
    key::Key,
    store::{Store, StorePath},
    value::Value,
};

fn make_store() -> Store {
    Store::new(StorePath::for_in_memory())
}

fn k(s: &str) -> Key {
    Key::from(s)
}

fn v(j: serde_json::Value) -> Value {
    Value::from(j)
}

#[test]
fn put_and_get() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    let cases: &[(Key, Value)] = &[
        (k("bool_true"), v(serde_json::Value::Bool(true))),
        (k("bool_false"), v(serde_json::Value::Bool(false))),
        (k("int"), v(serde_json::json!(42_i64))),
        (k("float"), v(serde_json::json!(3.14_f64))),
        (k("string"), v(serde_json::json!("hello"))),
        (k("null"), v(serde_json::Value::Null)),
    ];

    for (key, val) in cases {
        db.put(&[(key.clone(), Some(val.clone()))]).unwrap();
        assert_eq!(db.get(key, &opts).unwrap().as_ref(), Some(val));
    }
}

#[test]
fn get_missing_key_returns_none() {
    let store = make_store();
    let db = Database::new(&store, "test");
    assert_eq!(db.get(&k("absent"), &GetOptions::default()).unwrap(), None);
}

#[test]
fn has_existing_and_missing() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    db.put(&[(k("key"), Some(v(serde_json::json!("x"))))]).unwrap();
    assert!(db.has(&k("key"), &opts).unwrap());
    assert!(!db.has(&k("absent"), &opts).unwrap());
}

#[test]
fn put_overwrites() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    db.put(&[(k("k"), Some(v(serde_json::json!("first"))))]).unwrap();
    db.put(&[(k("k"), Some(v(serde_json::json!("second"))))]).unwrap();
    assert_eq!(
        db.get(&k("k"), &opts).unwrap(),
        Some(v(serde_json::json!("second")))
    );
}

#[test]
fn delete_existing() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    db.put(&[(k("key"), Some(v(serde_json::json!(1))))]).unwrap();
    db.delete(&k("key")).unwrap();
    assert!(!db.has(&k("key"), &opts).unwrap());
    assert_eq!(db.get(&k("key"), &opts).unwrap(), None);
}

#[test]
fn delete_missing_is_ok() {
    let store = make_store();
    let db = Database::new(&store, "test");
    db.delete(&k("absent")).unwrap();
}

#[test]
fn clear_empties_database() {
    let store = make_store();
    let db = Database::new(&store, "test");

    for i in 0..5 {
        db.put(&[(k(&format!("k{i}")), Some(v(serde_json::json!(i))))]).unwrap();
    }
    assert!(!db.is_empty().unwrap());

    db.clear().unwrap();
    assert!(db.is_empty().unwrap());
}

#[test]
fn is_empty_and_count() {
    let store = make_store();
    let db = Database::new(&store, "test");

    assert!(db.is_empty().unwrap());
    assert_eq!(db.count().unwrap(), 0);

    db.put(&[(k("a"), Some(v(serde_json::json!(1))))]).unwrap();
    db.put(&[(k("b"), Some(v(serde_json::json!(2))))]).unwrap();
    assert!(!db.is_empty().unwrap());
    assert_eq!(db.count().unwrap(), 2);

    db.delete(&k("a")).unwrap();
    assert_eq!(db.count().unwrap(), 1);
}

#[test]
fn put_multiple_pairs() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    db.put(&[
        (k("x"), Some(v(serde_json::json!(1)))),
        (k("y"), Some(v(serde_json::json!(2)))),
        (k("z"), Some(v(serde_json::json!(3)))),
    ])
    .unwrap();

    assert_eq!(db.count().unwrap(), 3);
    assert_eq!(db.get(&k("y"), &opts).unwrap(), Some(v(serde_json::json!(2))));
}

#[test]
fn put_none_deletes() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    db.put(&[(k("key"), Some(v(serde_json::json!("val"))))]).unwrap();
    assert!(db.has(&k("key"), &opts).unwrap());

    db.put(&[(k("key"), None::<Value>)]).unwrap();
    assert!(!db.has(&k("key"), &opts).unwrap());
}

#[test]
fn enumerate_all() {
    let store = make_store();
    let db = Database::new(&store, "test");

    db.put(&[
        (k("a"), Some(v(serde_json::json!(1)))),
        (k("b"), Some(v(serde_json::json!(2)))),
        (k("c"), Some(v(serde_json::json!(3)))),
    ])
    .unwrap();

    let pairs = db.enumerate(.., &GetOptions::default()).unwrap();
    assert_eq!(pairs.len(), 3);
    assert_eq!(pairs[0].0, k("a"));
    assert_eq!(pairs[1].0, k("b"));
    assert_eq!(pairs[2].0, k("c"));
}

#[test]
fn enumerate_bounded_range() {
    let store = make_store();
    let db = Database::new(&store, "test");

    for ch in ["a", "b", "c", "d", "e"] {
        db.put(&[(k(ch), Some(v(serde_json::json!(ch))))]).unwrap();
    }

    let pairs = db.enumerate(k("b")..k("d"), &GetOptions::default()).unwrap();
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].0, k("b"));
    assert_eq!(pairs[1].0, k("c"));
}

#[test]
fn enumerate_empty_range() {
    let store = make_store();
    let db = Database::new(&store, "test");

    db.put(&[(k("b"), Some(v(serde_json::json!(1))))]).unwrap();

    let pairs = db.enumerate(k("b")..k("b"), &GetOptions::default()).unwrap();
    assert!(pairs.is_empty());

    let pairs = db.enumerate(k("c")..k("a"), &GetOptions::default()).unwrap();
    assert!(pairs.is_empty());
}

#[test]
fn enumerate_from_nonexistent() {
    let store = make_store();
    let db = Database::new(&store, "test");

    for ch in ["a", "c", "e"] {
        db.put(&[(k(ch), Some(v(serde_json::json!(ch))))]).unwrap();
    }

    let pairs = db.enumerate(k("b").., &GetOptions::default()).unwrap();
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].0, k("c"));
    assert_eq!(pairs[1].0, k("e"));
}

#[test]
fn enumerate_to_nonexistent() {
    let store = make_store();
    let db = Database::new(&store, "test");

    for ch in ["a", "c", "e"] {
        db.put(&[(k(ch), Some(v(serde_json::json!(ch))))]).unwrap();
    }

    let pairs = db.enumerate(..k("d"), &GetOptions::default()).unwrap();
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].0, k("a"));
    assert_eq!(pairs[1].0, k("c"));
}

#[test]
fn delete_range() {
    let store = make_store();
    let db = Database::new(&store, "test");

    for ch in ["a", "b", "c", "d", "e"] {
        db.put(&[(k(ch), Some(v(serde_json::json!(ch))))]).unwrap();
    }

    db.delete_range(k("b")..k("d")).unwrap();

    let pairs = db.enumerate(.., &GetOptions::default()).unwrap();
    let keys: Vec<_> = pairs.iter().map(|(k, _)| k.as_str().to_owned()).collect();
    assert_eq!(keys, vec!["a", "d", "e"]);
}

#[test]
fn delete_range_unbounded() {
    let store = make_store();
    let db = Database::new(&store, "test");

    for ch in ["a", "b", "c"] {
        db.put(&[(k(ch), Some(v(serde_json::json!(ch))))]).unwrap();
    }

    db.delete_range(..).unwrap();
    assert!(db.is_empty().unwrap());
}

#[test]
fn multiple_databases_isolated() {
    let store = make_store();
    let foo = Database::new(&store, "foo");
    let bar = Database::new(&store, "bar");
    let opts = GetOptions::default();

    foo.put(&[(k("key"), Some(v(serde_json::json!("foo_val"))))]).unwrap();
    bar.put(&[(k("key"), Some(v(serde_json::json!("bar_val"))))]).unwrap();

    assert_eq!(
        foo.get(&k("key"), &opts).unwrap(),
        Some(v(serde_json::json!("foo_val")))
    );
    assert_eq!(
        bar.get(&k("key"), &opts).unwrap(),
        Some(v(serde_json::json!("bar_val")))
    );

    foo.clear().unwrap();
    assert!(foo.is_empty().unwrap());
    assert!(!bar.is_empty().unwrap());
}

#[test]
fn whitespace_keys() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    let keys = [" leading", "trailing ", " both "];
    for key in &keys {
        db.put(&[(k(key), Some(v(serde_json::json!(1))))]).unwrap();
        assert!(db.has(&k(key), &opts).unwrap());
    }

    let pairs = db.enumerate(.., &GetOptions::default()).unwrap();
    assert_eq!(pairs.len(), keys.len());
    for (pair, expected_key) in pairs.iter().zip([" both ", " leading", "trailing "]) {
        assert_eq!(pair.0.as_str(), expected_key);
    }
}

#[test]
fn unicode_keys() {
    let store = make_store();
    let db = Database::new(&store, "test");
    let opts = GetOptions::default();

    let key = "Héllo, wőrld!";
    db.put(&[(k(key), Some(v(serde_json::json!(42))))]).unwrap();
    assert!(db.has(&k(key), &opts).unwrap());
    assert_eq!(
        db.get(&k(key), &opts).unwrap(),
        Some(v(serde_json::json!(42)))
    );
}

#[test]
fn concurrent_get_option() {
    let store = make_store();
    let db = Database::new(&store, "test");

    db.put(&[(k("key"), Some(v(serde_json::json!("val"))))]).unwrap();

    let mut opts = GetOptions::default();
    opts.concurrent(true);

    assert_eq!(
        db.get(&k("key"), &opts).unwrap(),
        Some(v(serde_json::json!("val")))
    );
    assert!(db.has(&k("key"), &opts).unwrap());

    let pairs = db.enumerate(.., &opts).unwrap();
    assert_eq!(pairs.len(), 1);
}

#[test]
fn size_increases_with_data() {
    let store = make_store();
    let db = Database::new(&store, "test");

    assert_eq!(db.size().unwrap(), 0);

    db.put(&[(k("key"), Some(v(serde_json::json!("value"))))]).unwrap();
    let size_after_one = db.size().unwrap();
    assert!(size_after_one > 0);

    db.put(&[(k("another_key"), Some(v(serde_json::json!("another_value"))))]).unwrap();
    assert!(db.size().unwrap() > size_after_one);
}
