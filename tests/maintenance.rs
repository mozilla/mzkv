/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::{
    convert::Infallible,
    fmt,
    sync::{mpsc, Arc},
    time::Duration,
};

use skv::{
    checker::{CheckerAction, IntoChecker},
    connection::{ConnectionIncidents, ConnectionMaintenanceTask},
    maintenance::{Maintenance, MaintenanceError},
    store::{Store, StoreError, StorePath},
};

macro_rules! always_check {
    ($name:ident, $err:ty, $body:expr) => {
        struct $name;

        impl IntoChecker<$name> for ConnectionIncidents<'_> {
            fn into_checker(self) -> CheckerAction<$name> {
                self.map(|_| CheckerAction::Check($name))
            }
        }

        impl ConnectionMaintenanceTask for $name {
            type Error = $err;

            fn run(self, conn: &mut rusqlite::Connection) -> Result<(), Self::Error> {
                ($body)(conn)
            }
        }
    };
}

#[test]
fn quick_check_passes_on_clean_store() {
    always_check!(RunQuickCheck, MaintenanceError, |conn| {
        Maintenance::new(conn).quick_check()
    });
    let store = Store::new(StorePath::for_in_memory());
    assert!(store.check::<RunQuickCheck>().is_ok());
    store.close();
}

#[test]
fn integrity_check_passes_on_clean_store() {
    always_check!(RunIntegrityCheck, MaintenanceError, |conn| {
        Maintenance::new(conn).integrity_check()
    });
    let store = Store::new(StorePath::for_in_memory());
    assert!(store.check::<RunIntegrityCheck>().is_ok());
    store.close();
}

#[test]
fn foreign_key_check_passes() {
    always_check!(RunForeignKeyCheck, MaintenanceError, |conn| {
        Maintenance::new(conn).foreign_key_check()
    });
    let store = Store::new(StorePath::for_in_memory());
    assert!(store.check::<RunForeignKeyCheck>().is_ok());
    store.close();
}

#[test]
fn check_succeeds_on_clean_store() {
    let store = Store::new(StorePath::for_in_memory());
    assert!(store.check::<skv::checker::Checker>().is_ok());
    store.close();
}

#[test]
fn maintenance_succeeds() {
    always_check!(AlwaysSucceed, Infallible, |_conn| Ok(()));
    let store = Store::new(StorePath::for_in_memory());
    assert!(store.check::<AlwaysSucceed>().is_ok());
    store.close();
}

#[test]
fn maintenance_fails() {
    #[derive(Debug)]
    struct CheckerError;
    impl std::error::Error for CheckerError {}
    impl fmt::Display for CheckerError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("intentional failure")
        }
    }

    always_check!(AlwaysFail, CheckerError, |_conn| Err(CheckerError));

    let store = Store::new(StorePath::for_in_memory());
    assert!(matches!(
        store.check::<AlwaysFail>(),
        Err(StoreError::Maintenance(_))
    ));
    store.close();
}

#[test]
fn check_blocks_concurrent_access() {
    thread_local! {
        static READY_TX: std::cell::Cell<Option<mpsc::SyncSender<()>>> = std::cell::Cell::new(None);
        static DONE_RX: std::cell::Cell<Option<mpsc::Receiver<()>>> = std::cell::Cell::new(None);
    }

    struct BlockingChecker;

    impl IntoChecker<BlockingChecker> for ConnectionIncidents<'_> {
        fn into_checker(self) -> CheckerAction<BlockingChecker> {
            self.map(|_| CheckerAction::Check(BlockingChecker))
        }
    }

    impl ConnectionMaintenanceTask for BlockingChecker {
        type Error = Infallible;

        fn run(self, _conn: &mut rusqlite::Connection) -> Result<(), Infallible> {
            READY_TX.with(|c| {
                if let Some(tx) = c.take() {
                    let _ = tx.send(());
                }
            });
            DONE_RX.with(|c| {
                if let Some(rx) = c.take() {
                    let _ = rx.recv();
                }
            });
            Ok(())
        }
    }

    let (tx_ready, rx_ready) = mpsc::sync_channel::<()>(0);
    let (tx_done, rx_done) = mpsc::sync_channel::<()>(0);

    let store = Arc::new(Store::new(StorePath::for_in_memory()));
    let thread = {
        let store = store.clone();
        std::thread::spawn(move || {
            READY_TX.with(|c| c.set(Some(tx_ready)));
            DONE_RX.with(|c| c.set(Some(rx_done)));
            assert!(
                store.check::<BlockingChecker>().is_ok(),
                "first check should succeed"
            );
        })
    };

    rx_ready
        .recv_timeout(Duration::from_secs(5))
        .expect("timed out waiting for checker to start");

    assert!(
        matches!(store.check::<BlockingChecker>(), Err(StoreError::Busy)),
        "check during maintenance should return Busy"
    );

    let _ = tx_done.send(());
    thread.join().unwrap();
    store.close();
}

#[test]
fn close_during_maintenance() {
    thread_local! {
        static READY_TX: std::cell::Cell<Option<mpsc::SyncSender<()>>> = std::cell::Cell::new(None);
    }

    struct InfiniteChecker;

    impl IntoChecker<InfiniteChecker> for ConnectionIncidents<'_> {
        fn into_checker(self) -> CheckerAction<InfiniteChecker> {
            self.map(|_| CheckerAction::Check(InfiniteChecker))
        }
    }

    impl ConnectionMaintenanceTask for InfiniteChecker {
        type Error = rusqlite::Error;

        fn run(self, conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
            READY_TX.with(|c| {
                if let Some(tx) = c.take() {
                    let _ = tx.send(());
                }
            });
            conn.execute(
                "WITH RECURSIVE x(i) AS (
                   SELECT 1
                   UNION ALL
                   SELECT i + 1 FROM x
                 )
                 SELECT i FROM x",
                [],
            )?;
            unreachable!("query should never return")
        }
    }

    let (tx_ready, rx_ready) = mpsc::sync_channel::<()>(0);

    let store = Arc::new(Store::new(StorePath::for_in_memory()));
    let thread = {
        let store = store.clone();
        std::thread::spawn(move || {
            READY_TX.with(|c| c.set(Some(tx_ready)));
            assert!(
                matches!(
                    store.check::<InfiniteChecker>(),
                    Err(StoreError::Maintenance(_))
                ),
                "interrupted check should return Maintenance error"
            );
        })
    };

    rx_ready
        .recv_timeout(Duration::from_secs(5))
        .expect("timed out waiting for checker to start");

    store.close();

    assert!(
        matches!(store.check::<InfiniteChecker>(), Err(StoreError::Closed)),
        "check on closed store should return Closed"
    );

    thread.join().unwrap();
}

#[test]
fn renames_corrupt_database_file() {
    #[derive(Debug)]
    struct CheckerError;
    impl std::error::Error for CheckerError {}
    impl fmt::Display for CheckerError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("simulated corruption")
        }
    }

    always_check!(AlwaysFail, CheckerError, |_conn| Err(CheckerError));
    always_check!(AlwaysSucceed, Infallible, |_conn| Ok(()));

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.sqlite");

    {
        let store = Store::new(StorePath::OnDisk(file_path.clone()));
        skv::database::Database::new(&store, "db")
            .put(&[(
                skv::key::Key::from("k"),
                Some(skv::value::Value::from(serde_json::json!(1))),
            )])
            .unwrap();
        store.close();
    }

    let store = Arc::new(Store::new(StorePath::OnDisk(file_path.clone())));
    assert!(matches!(
        store.check::<AlwaysFail>(),
        Err(StoreError::Maintenance(_))
    ));
    store.close();

    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        files.iter().all(|f| f.contains(".corrupt-")),
        "all database files should be renamed as corrupt: {files:?}"
    );

    let store = Arc::new(Store::new(StorePath::OnDisk(file_path)));
    assert!(
        store.check::<AlwaysSucceed>().is_ok(),
        "fresh store should pass check"
    );
    assert!(
        matches!(
            store.writer().and_then(|w| w.read(|conn| Ok(conn.query_row(
                "SELECT count(*) FROM dbs",
                [],
                |row| row.get::<_, usize>(0)
            )?))),
            Ok(0)
        ),
        "fresh store should be empty"
    );
    store.close();
}
