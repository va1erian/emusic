//! Backup rotation against real database files in a scratch folder.

use std::time::{Duration, SystemTime};

use super::*;

/// A store in a fresh scratch folder, removed (with its backups) on drop.
struct Scratch {
    dir: PathBuf,
    store: Store,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("emusic-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir.join("library.db")).expect("open store");
        Self { dir, store }
    }

    fn backups(&self) -> Vec<PathBuf> {
        list_backups(&self.dir.join(DIR_NAME)).expect("list backups")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn the_first_backup_is_written_and_holds_the_data() {
    let scratch = Scratch::new("first");
    scratch
        .store
        .add_folder(Path::new(r"C:\music"))
        .expect("add folder");

    let outcome = scratch
        .store
        .backup_if_due(SystemTime::now())
        .expect("backup");

    let BackupOutcome::Written(path) = outcome else {
        panic!("expected a backup, got {outcome:?}");
    };
    assert_eq!(scratch.backups(), std::slice::from_ref(&path));
    let copy = Store::open(&path).expect("open backup");
    assert_eq!(copy.list_folders().expect("folders").len(), 1);
}

#[test]
fn a_recent_backup_is_not_repeated() {
    let scratch = Scratch::new("recent");
    let now = SystemTime::now();
    scratch.store.backup_if_due(now).expect("first backup");

    let outcome = scratch.store.backup_if_due(now).expect("second call");

    assert_eq!(outcome, BackupOutcome::NotDue);
    assert_eq!(scratch.backups().len(), 1);
}

#[test]
fn a_day_later_another_backup_is_written() {
    let scratch = Scratch::new("later");
    let now = SystemTime::now();
    scratch.store.backup_if_due(now).expect("first backup");

    let later = now + INTERVAL + Duration::from_secs(60);
    let outcome = scratch.store.backup_if_due(later).expect("second backup");

    assert!(matches!(outcome, BackupOutcome::Written(_)));
    assert_eq!(scratch.backups().len(), 2);
}

#[test]
fn only_the_newest_backups_are_kept() {
    let scratch = Scratch::new("prune");
    let dir = scratch.dir.join(DIR_NAME);
    std::fs::create_dir_all(&dir).expect("backup dir");
    // Old-dated backups, plus a file that is not a backup and must survive.
    for day in 1..=KEEP + 2 {
        std::fs::write(
            dir.join(format!("library-202601{day:02}-000000.db")),
            b"old",
        )
        .expect("fake backup");
    }
    std::fs::write(dir.join("notes.txt"), b"keep me").expect("other file");

    prune(&dir, KEEP).expect("prune");

    let kept = scratch.backups();
    assert_eq!(kept.len(), KEEP);
    assert!(kept[0].ends_with("library-20260103-000000.db"));
    assert!(dir.join("notes.txt").exists());
}

#[test]
fn a_stale_index_is_rebuilt_in_the_backup() {
    let scratch = Scratch::new("stale-index");
    // Make an index disagree with its table, as a real library's play-history
    // index once did: build it over `t` (3 rows), then repoint its schema
    // entry at `u` (1 row). The edit takes effect on the next open.
    scratch
        .store
        .conn
        .execute_batch(
            "CREATE TABLE t (x INTEGER);
             CREATE TABLE u (x INTEGER);
             INSERT INTO t VALUES (1), (2), (3);
             INSERT INTO u VALUES (1);
             CREATE INDEX t_x ON t (x);
             PRAGMA writable_schema = ON;
             UPDATE sqlite_master SET sql = 'CREATE INDEX t_x ON u (x)', tbl_name = 'u'
                 WHERE name = 't_x';
             PRAGMA writable_schema = OFF;",
        )
        .expect("desynchronise the index");
    let store = Store::open(&scratch.dir.join("library.db")).expect("reopen");
    assert!(
        integrity_problem(&store.conn).expect("check").is_some(),
        "the index should look stale"
    );

    let outcome = store.backup_if_due(SystemTime::now()).expect("backup");

    let BackupOutcome::Written(path) = outcome else {
        panic!("expected a backup, got {outcome:?}");
    };
    let copy = Connection::open(&path).expect("open backup");
    assert_eq!(integrity_problem(&copy).expect("check copy"), None);
}

#[test]
fn a_healthy_database_has_no_integrity_problem() {
    let scratch = Scratch::new("healthy");
    assert_eq!(integrity_problem(&scratch.store.conn).expect("check"), None);
}

#[test]
fn an_in_memory_store_has_nothing_to_back_up() {
    let store = Store::open_in_memory().expect("in-memory store");
    let outcome = store.backup_if_due(SystemTime::now()).expect("backup");
    assert_eq!(outcome, BackupOutcome::NoFile);
}
