//! Seeds the SLO reference dataset into a throwaway database.
//!
//! Usage:
//!   DIGILEGACY_DB=/tmp/digilegacy-capacity/live.db \
//!     cargo run -q -p digilegacy-server --example seed_reference
//!
//! Target: 1,000 assets, 250 heirs, 10,000 audit entries. Existing rows are
//! counted first, so rerunning tops up instead of duplicating the whole set.

use digilegacy_store::Store;
use rusqlite::params;

fn count(store: &Store, sql: &str) -> i64 {
    store
        .read(|conn| conn.query_row(sql, [], |row| row.get(0)))
        .expect("count rows")
}

fn main() {
    let path = std::env::var("DIGILEGACY_DB").expect("DIGILEGACY_DB");
    let store = Store::open(&path, None).expect("open capacity database");

    let now = "2026-11-20T10:00:00+08:00";
    let assets = count(&store, "SELECT count(*) FROM digital_assets");
    if assets < 1000 {
        store
            .write(|conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT OR IGNORE INTO platforms (id, name, category) VALUES (9001, 'Capacity Bench', 'account')",
                    [],
                )?;
                for index in assets..1000 {
                    tx.execute(
                        "INSERT INTO digital_assets
                         (platform_id, account_identifier, account_description, category,
                          estimated_value, value_currency, disposal_method, notes,
                          created_at, updated_at, status, source)
                         VALUES (9001, ?1, ?2, 'account', 100, 'CNY', 'no_action', '',
                                 ?3, ?3, 'active', 'bench')",
                        params![format!("bench-{index:04}"), format!("容量资产 {index:04}"), now],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .expect("seed assets");
    }

    let heirs = count(&store, "SELECT count(*) FROM heirs");
    if heirs < 250 {
        store
            .write(|conn| {
                let tx = conn.transaction()?;
                for index in heirs..250 {
                    tx.execute(
                        "INSERT INTO heirs (name, relation, email, is_executor, sort_order, created_at, updated_at)
                         VALUES (?1, 'heir', ?2, 0, ?3, ?4, ?4)",
                        params![
                            format!("容量继承人 {index:03}"),
                            format!("heir{index:03}@example.test"),
                            index,
                            now
                        ],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .expect("seed heirs");
    }

    let audit = count(&store, "SELECT count(*) FROM audit_log");
    for index in audit..10_000 {
        store
            .append_audit("bench.seed", &format!("{index}"), "ok", "")
            .expect("append benchmark audit");
    }

    println!(
        "{{\"assets\":{},\"heirs\":{},\"audit\":{},\"schema\":{},\"tables\":{}}}",
        count(&store, "SELECT count(*) FROM digital_assets"),
        count(&store, "SELECT count(*) FROM heirs"),
        count(&store, "SELECT count(*) FROM audit_log"),
        store.schema_version().unwrap_or(-1),
        store.table_count().unwrap_or(-1)
    );
}
