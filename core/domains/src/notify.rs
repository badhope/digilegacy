//! DOM-04/notification outbox: final trigger notifications with delivery state.
//!
//! The outbox is the single hand-off between the trigger lifecycle and an
//! external channel. It never sends directly; the application worker owns
//! network egress so the domain stays deterministic and testable.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::assets::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub created_at: String,
    pub trigger_event_id: i64,
    pub kind: String,
    pub recipient: String,
    pub recipient_role: String,
    pub subject: String,
    pub body: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: String,
    pub sent_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<Notification>,
    pub next_cursor: Option<String>,
    pub total: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInput {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

fn map_notification(row: &rusqlite::Row<'_>) -> rusqlite::Result<Notification> {
    Ok(Notification {
        id: row.get::<_, i64>(0)?.to_string(),
        created_at: row.get(1)?,
        trigger_event_id: row.get(2)?,
        kind: row.get(3)?,
        recipient: row.get(4)?,
        recipient_role: row.get(5)?,
        subject: row.get(6)?,
        body: row.get(7)?,
        status: row.get(8)?,
        attempts: row.get(9)?,
        last_error: row.get(10)?,
        sent_at: row.get(11)?,
    })
}

fn recipient_rows(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut recipients = Vec::new();
    let mut add = |value: String, role: &str| {
        if value.trim().is_empty() {
            return;
        }
        if recipients
            .iter()
            .any(|(existing, _): &(String, String)| existing == &value)
        {
            return;
        }
        recipients.push((value, role.to_string()));
    };

    if let Some(row) = conn
        .query_row(
            "SELECT coalesce(emergency_contact_name,''), coalesce(emergency_contact_email,''),
                    coalesce(emergency_contact_phone,''), coalesce(heir_primary_name,''),
                    coalesce(heir_primary_email,''), coalesce(heir_primary_phone,'')
             FROM user_profile WHERE id=1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
    {
        add(
            if !row.1.is_empty() {
                row.1
            } else if !row.2.is_empty() {
                row.2
            } else {
                row.0
            },
            "紧急联系人",
        );
        add(
            if !row.4.is_empty() {
                row.4
            } else if !row.5.is_empty() {
                row.5
            } else {
                row.3
            },
            "主继承人",
        );
    }

    let mut stmt = conn.prepare(
        "SELECT name, email, phone, relation, is_executor FROM heirs ORDER BY sort_order, id",
    )?;
    let heirs = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (name, email, phone, relation, executor) in heirs {
        add(
            if !email.is_empty() {
                email
            } else if !phone.is_empty() {
                phone
            } else {
                name
            },
            if executor != 0 {
                "执行人"
            } else if relation.is_empty() {
                "继承人"
            } else {
                &relation
            },
        );
    }
    Ok(recipients)
}

/// Insert one row per distinct recipient for a final trigger event.
/// `INSERT OR IGNORE` keeps re-evaluation idempotent through the unique
/// `(trigger_event_id, recipient)` constraint.
pub fn enqueue_final(conn: &Connection, at: &str, event_id: i64) -> rusqlite::Result<usize> {
    let holders = conn
        .query_row(
            "SELECT coalesce(full_name,'') FROM user_profile WHERE id=1",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_default();
    let holder = if holders.is_empty() {
        "持有人".to_string()
    } else {
        holders
    };
    let asset_count: i64 = conn.query_row(
        "SELECT count(*) FROM digital_assets WHERE status='active'",
        [],
        |r| r.get(0),
    )?;
    let subject = "DigiLegacy 最终交付通知".to_string();
    let body = format!(
        "{holder} 的数字遗产触发链已进入最终交付阶段（{at}）。\n\n当前登记的有效资产：{asset_count} 项。请按平台内的资产清单、继承人与交付物执行；登录控制台查看完整信息。"
    );

    let mut recipients = recipient_rows(conn)?;
    if recipients.is_empty() {
        recipients.push((String::new(), "未配置接收人".to_string()));
    }

    let mut inserted = 0;
    for (recipient, role) in recipients {
        let status = if recipient.is_empty() {
            "blocked"
        } else {
            "pending"
        };
        let error = if recipient.is_empty() {
            "未配置紧急联系人或继承人"
        } else {
            ""
        };
        let changed = conn.execute(
            "INSERT OR IGNORE INTO notification_outbox
             (created_at, trigger_event_id, kind, recipient, recipient_role, subject, body, status, last_error)
             VALUES (?1,?2,'trigger.final',?3,?4,?5,?6,?7,?8)",
            params![at, event_id, recipient, role, subject, body, status, error],
        )?;
        inserted += changed;
    }
    Ok(inserted)
}

pub fn list(store: &Store, input: &ListInput) -> Result<Page, DomainError> {
    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    let status = input.status.as_deref().unwrap_or("all");
    if !matches!(status, "all" | "pending" | "sent" | "failed" | "blocked") {
        return Err(DomainError::Invalid(
            "通知状态必须是 all/pending/sent/failed/blocked".to_string(),
        ));
    }
    let cursor = match input.cursor.as_deref() {
        Some("") | None => None,
        Some(value) => Some(
            value
                .parse::<i64>()
                .map_err(|_| DomainError::Invalid("cursor 必须是通知 id".to_string()))?,
        ),
    };
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, trigger_event_id, kind, recipient, recipient_role,
                        subject, body, status, attempts, last_error, coalesce(sent_at,'')
                 FROM notification_outbox
                 WHERE (?1='all' OR status=?1) AND (?2 IS NULL OR id < ?2)
                 ORDER BY id DESC LIMIT ?3",
            )?;
            let mut items = stmt
                .query_map(params![status, cursor, limit + 1], map_notification)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let has_more = items.len() as i64 > limit;
            if has_more {
                items.truncate(limit as usize);
            }
            let next_cursor = if has_more {
                items.last().map(|item| item.id.clone())
            } else {
                None
            };
            let total: i64 = conn.query_row(
                "SELECT count(*) FROM notification_outbox WHERE (?1='all' OR status=?1)",
                params![status],
                |r| r.get(0),
            )?;
            Ok(Page {
                items,
                next_cursor,
                total,
            })
        })
        .map_err(DomainError::from)
}

pub fn pending(store: &Store) -> Result<Vec<Notification>, DomainError> {
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, trigger_event_id, kind, recipient, recipient_role,
                        subject, body, status, attempts, last_error, coalesce(sent_at,'')
                 FROM notification_outbox WHERE status='pending' ORDER BY id LIMIT 50",
            )?;
            let items = stmt
                .query_map([], map_notification)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(items)
        })
        .map_err(DomainError::from)
}

pub fn mark_sent(store: &Store, id: &str) -> Result<(), DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    store
        .write(|conn| {
            conn.execute(
                "UPDATE notification_outbox
                 SET status='sent', attempts=attempts+1, last_error='', sent_at=?2
                 WHERE id=?1 AND status='pending'",
                params![id, now],
            )?;
            Ok(())
        })
        .map_err(DomainError::from)
}

/// Move failed rows back to pending for an explicit manual retry.
pub fn retry_failed(store: &Store) -> Result<usize, DomainError> {
    store
        .write(|conn| {
            let changed = conn.execute(
                "UPDATE notification_outbox
                 SET status='pending', last_error=''
                 WHERE status='failed'",
                [],
            )?;
            Ok(changed)
        })
        .map_err(DomainError::from)
}

pub fn mark_failed(store: &Store, id: &str, error: &str) -> Result<(), DomainError> {
    store
        .write(|conn| {
            conn.execute(
                "UPDATE notification_outbox
                 SET status='failed', attempts=attempts+1, last_error=?2
                 WHERE id=?1 AND status='pending'",
                params![id, error],
            )?;
            Ok(())
        })
        .map_err(DomainError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::DomainError;
    use digilegacy_store::Store;

    fn store() -> Store {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO user_profile
                     (full_name, emergency_contact_name, emergency_contact_email,
                      heir_primary_name, heir_primary_email, created_at, updated_at)
                     VALUES ('张三','李四','lisi@example.com','王五','wangwu@example.com','2026-01-01','2026-01-01')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO heirs (name, relation, email, is_executor, sort_order, created_at, updated_at)
                     VALUES ('王五','子女','wangwu@example.com',1,0,'2026-01-01','2026-01-01')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO trigger_events (created_at, kind, stage, detail) VALUES ('2026-01-01','final',4,'done')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        store
    }

    fn event_id(store: &Store) -> i64 {
        store
            .read(|conn| conn.query_row("SELECT max(id) FROM trigger_events", [], |r| r.get(0)))
            .unwrap()
    }

    #[test]
    fn final_event_deduplicates_recipients() {
        let store = store();
        let id = event_id(&store);
        store
            .write(|conn| {
                enqueue_final(conn, "2026-01-01T00:00:00+00:00", id)?;
                Ok(())
            })
            .unwrap();
        store
            .write(|conn| {
                enqueue_final(conn, "2026-01-01T00:00:00+00:00", id)?;
                Ok(())
            })
            .unwrap();
        let page = list(&store, &ListInput::default()).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].status, "pending");
    }

    #[test]
    fn block_when_no_recipient_is_visible() {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                enqueue_final(conn, "2026-01-01T00:00:00+00:00", 9)?;
                Ok(())
            })
            .unwrap();
        let page = list(&store, &ListInput::default()).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].status, "blocked");
    }

    #[test]
    fn failed_delivery_is_recorded() {
        let store = store();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO notification_outbox (created_at, recipient, status)
                     VALUES ('2026-01-01','a@example.com','pending')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let id: i64 = store
            .read(|conn| {
                conn.query_row("SELECT max(id) FROM notification_outbox", [], |r| r.get(0))
            })
            .unwrap();
        mark_failed(&store, &id.to_string(), "timeout").unwrap();
        let page = list(&store, &ListInput::default()).unwrap();
        assert_eq!(page.items[0].status, "failed");
        assert_eq!(page.items[0].last_error, "timeout");
        assert!(matches!(
            mark_failed(&store, &id.to_string(), "again"),
            Err(DomainError::Store(_)) | Ok(())
        ));
        let changed = retry_failed(&store).unwrap();
        assert_eq!(changed, 1);
        let page = list(&store, &ListInput::default()).unwrap();
        assert_eq!(page.items[0].status, "pending");
        assert_eq!(page.items[0].last_error, "");
    }
}
