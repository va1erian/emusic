//! Unit tests for the audit writer, store and retention.

use std::sync::atomic::AtomicU64;

use super::*;
use crate::db::Db;

fn temp_db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_at(&dir.path().join("audit.db")).expect("open db");
    (dir, db)
}

fn all_rows(db: &Db) -> Vec<AuditRow> {
    db.audit_rows(&AuditQuery {
        limit: 500,
        ..AuditQuery::default()
    })
    .expect("query")
}

#[test]
fn spawned_writer_persists_events_after_flush() {
    let (_dir, db) = temp_db();
    let audit = AuditLog::spawn(db.clone()).expect("spawn");
    audit.pair_failed("203.0.113.9", PairFailure::Expired);
    audit.device_revoked("127.0.0.1", "dev-1", &Actor::Admin);
    audit.flush();

    let rows = all_rows(&db);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].event, "device_revoked");
    assert_eq!(rows[0].device_id.as_deref(), Some("dev-1"));
    assert_eq!(rows[0].detail["by"], "admin");
    assert_eq!(rows[1].event, "pair_failed");
    assert_eq!(rows[1].client_ip, "203.0.113.9");
    assert_eq!(rows[1].detail["reason"], "expired");
}

#[test]
fn a_full_queue_drops_and_the_writer_reports_the_loss() {
    let (_dir, db) = temp_db();
    let (audit, receiver) = AuditLog::queued(2);
    for _ in 0..5 {
        audit.rate_limited("198.51.100.1", "pair");
    }
    assert_eq!(audit.dropped(), 3, "two fit, three are dropped");

    drop(audit);
    let dropped = AtomicU64::new(3);
    writer::run_writer(&db, &receiver, &dropped);

    let rows = all_rows(&db);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].event, "audit_events_dropped");
    assert_eq!(rows[0].detail["count"], 3);
    assert!(rows[1..].iter().all(|row| row.event == "rate_limited"));
}

#[test]
fn client_triggerable_events_are_throttled_per_client() {
    let (_dir, db) = temp_db();
    let audit = AuditLog::direct(db.clone());
    for _ in 0..PER_CLIENT_LIMIT + 5 {
        audit.auth_failed("198.51.100.7", "invalid token");
    }
    audit.auth_failed("198.51.100.8", "invalid token");
    assert_eq!(all_rows(&db).len() as u32, PER_CLIENT_LIMIT + 1);
    assert_eq!(audit.throttled(), 5);
}

#[test]
fn direct_log_writes_synchronously() {
    let (_dir, db) = temp_db();
    let audit = AuditLog::direct(db.clone());
    audit.pairing_code_created(CLI_SOURCE, &Actor::Cli, 600);
    let rows = all_rows(&db);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].client_ip, "cli");
    assert_eq!(rows[0].detail["by"], "cli");
    assert_eq!(rows[0].detail["ttl_secs"], 600);
}

#[test]
fn queries_filter_and_paginate_newest_first() {
    let (_dir, db) = temp_db();
    let records: Vec<_> = (0..10)
        .map(|i| {
            let event = if i % 2 == 0 {
                "auth_failed"
            } else {
                "token_refreshed"
            };
            AuditRecord {
                at: 1_000 + i,
                ..AuditRecord::new(event).device(format!("dev-{}", i % 3))
            }
        })
        .collect();
    db.insert_audit_records(&records).unwrap();

    let page = db
        .audit_rows(&AuditQuery {
            event: Some("auth_failed".into()),
            limit: 2,
            ..AuditQuery::default()
        })
        .unwrap();
    assert_eq!(page.iter().map(|r| r.at).collect::<Vec<_>>(), [1008, 1006]);

    let next = db
        .audit_rows(&AuditQuery {
            event: Some("auth_failed".into()),
            before_id: Some(page[1].id),
            limit: 2,
            ..AuditQuery::default()
        })
        .unwrap();
    assert_eq!(next.iter().map(|r| r.at).collect::<Vec<_>>(), [1004, 1002]);

    let ranged = db
        .audit_rows(&AuditQuery {
            device_id: Some("dev-0".into()),
            since: Some(1_003),
            until: Some(1_009),
            limit: 50,
            ..AuditQuery::default()
        })
        .unwrap();
    assert_eq!(
        ranged.iter().map(|r| r.at).collect::<Vec<_>>(),
        [1006, 1003]
    );

    assert_eq!(
        db.audit_event_names().unwrap(),
        ["auth_failed", "token_refreshed"]
    );
}

#[test]
fn retention_prunes_old_rows_and_audits_the_prune() {
    let (_dir, db) = temp_db();
    let now = 100 * 24 * 3600;
    let old = AuditRecord {
        at: now - 91 * 24 * 3600,
        ..AuditRecord::new("scan_started")
    };
    let recent = AuditRecord {
        at: now - 89 * 24 * 3600,
        ..AuditRecord::new("scan_finished")
    };
    db.insert_audit_records(&[old, recent]).unwrap();

    let audit = AuditLog::direct(db.clone());
    assert_eq!(retention::prune(&db, &audit, 90, now).unwrap(), 1);
    let events: Vec<_> = all_rows(&db).into_iter().map(|row| row.event).collect();
    assert_eq!(events, ["audit_pruned", "scan_finished"]);
    assert_eq!(retention::prune(&db, &audit, 90, now).unwrap(), 0);
}

#[test]
fn known_events_are_sorted_and_unique() {
    let mut sorted = KNOWN_EVENTS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, KNOWN_EVENTS);
}
