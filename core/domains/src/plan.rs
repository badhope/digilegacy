//! DOM-03 plan: holder profile, heirs and the DMS trigger lifecycle.
//!
//! The trigger is a small state machine over `dms_config` plus an append-only
//! `trigger_events` ledger. Reads never mutate; `configure` and `evaluate` do.
//! A check-in clears any active trigger and resets the timer.

use chrono::{DateTime, Duration, FixedOffset, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::assets::DomainError;
use crate::notify;

const MAX_DAYS: i64 = 365;
const MAX_PAUSE_DAYS: i64 = 3650;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
    pub name: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Heir {
    pub id: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    pub mode: String,
    pub days: i64,
    pub status: String,
    pub last_check_in: String,
    pub next_check_at: String,
    pub paused_until: String,
    pub triggered_at: String,
    pub stage: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerEvent {
    pub id: String,
    pub created_at: String,
    pub kind: String,
    pub stage: i64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    pub items: Vec<TriggerEvent>,
    pub next_cursor: Option<String>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub holder: Holder,
    pub heirs: Vec<Heir>,
    pub trigger: Trigger,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureInput {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub days: Option<i64>,
    /// 0 clears the pause; a positive value pauses for that many days.
    #[serde(default)]
    pub pause_days: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsInput {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

struct DmsConfig {
    enabled: i64,
    days: i64,
    stage1_days: i64,
    stage2_days: i64,
    stage3_days: i64,
    last_check_in: String,
    paused_until: String,
    triggered_at: String,
    stage: i64,
}

fn read_config(conn: &Connection) -> rusqlite::Result<Option<DmsConfig>> {
    conn.query_row(
        "SELECT enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days,
                stage3_wait_days, coalesce(last_check_in,''), coalesce(paused_until,''),
                coalesce(triggered_at,''), coalesce(trigger_stage,0)
         FROM dms_config WHERE id=1",
        [],
        |r| {
            Ok(DmsConfig {
                enabled: r.get(0)?,
                days: r.get(1)?,
                stage1_days: r.get(2)?,
                stage2_days: r.get(3)?,
                stage3_days: r.get(4)?,
                last_check_in: r.get(5)?,
                paused_until: r.get(6)?,
                triggered_at: r.get(7)?,
                stage: r.get(8)?,
            })
        },
    )
    .optional()
}

fn parse_time(value: &str) -> Option<DateTime<FixedOffset>> {
    value.parse::<DateTime<FixedOffset>>().ok()
}

fn status_for(config: &DmsConfig, now: DateTime<FixedOffset>) -> &'static str {
    if !config.triggered_at.is_empty() {
        "triggered"
    } else if parse_time(&config.paused_until)
        .map(|p| p > now)
        .unwrap_or(false)
    {
        "paused"
    } else if config.enabled != 0 {
        "armed"
    } else {
        "off"
    }
}

fn trigger_from(config: &DmsConfig, now: DateTime<FixedOffset>) -> Trigger {
    let status = status_for(config, now).to_string();
    let next_check_at = parse_time(&config.last_check_in)
        .map(|last| (last + Duration::days(config.days)).to_rfc3339())
        .unwrap_or_default();
    Trigger {
        mode: "dms".to_string(),
        days: config.days,
        status,
        last_check_in: config.last_check_in.clone(),
        next_check_at,
        paused_until: config.paused_until.clone(),
        triggered_at: config.triggered_at.clone(),
        stage: config.stage,
    }
}

fn load_trigger(conn: &Connection) -> rusqlite::Result<Trigger> {
    let now = Utc::now().fixed_offset();
    match read_config(conn)? {
        Some(config) => Ok(trigger_from(&config, now)),
        None => Ok(Trigger {
            mode: "dms".to_string(),
            days: 30,
            status: "off".to_string(),
            last_check_in: String::new(),
            next_check_at: String::new(),
            paused_until: String::new(),
            triggered_at: String::new(),
            stage: 0,
        }),
    }
}

fn insert_event(
    conn: &Connection,
    at: &str,
    kind: &str,
    stage: i64,
    detail: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO trigger_events (created_at, kind, stage, detail) VALUES (?1,?2,?3,?4)",
        params![at, kind, stage, detail],
    )?;
    Ok(())
}

pub fn get(store: &Store) -> Result<Plan, DomainError> {
    store
        .read(|conn| {
            let holder = conn
                .query_row(
                    "SELECT coalesce(full_name,''), coalesce(updated_at,'') FROM user_profile WHERE id=1",
                    [],
                    |r| {
                        Ok(Holder {
                            name: r.get(0)?,
                            updated_at: r.get(1)?,
                        })
                    },
                )
                .optional()?
                .unwrap_or(Holder {
                    name: String::new(),
                    updated_at: String::new(),
                });

            let mut stmt = conn.prepare(
                "SELECT id, name, CASE WHEN is_executor != 0 THEN 'executor'
                                       ELSE coalesce(nullif(relation,''), 'heir') END
                 FROM heirs ORDER BY sort_order, id",
            )?;
            let heirs = stmt
                .query_map([], |r| {
                    Ok(Heir {
                        id: r.get::<_, i64>(0)?.to_string(),
                        name: r.get(1)?,
                        role: r.get(2)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let trigger = load_trigger(conn)?;
            Ok(Plan {
                holder,
                heirs,
                trigger,
            })
        })
        .map_err(DomainError::from)
}

pub fn configure(store: &Store, input: &ConfigureInput) -> Result<Trigger, DomainError> {
    if let Some(days) = input.days {
        if !(1..=MAX_DAYS).contains(&days) {
            return Err(DomainError::Invalid(format!(
                "签到间隔必须在 1..={MAX_DAYS} 天之间"
            )));
        }
    }
    if let Some(pause_days) = input.pause_days {
        if !(0..=MAX_PAUSE_DAYS).contains(&pause_days) {
            return Err(DomainError::Invalid(format!(
                "暂停天数必须在 0..={MAX_PAUSE_DAYS} 天之间"
            )));
        }
    }

    store
        .write(|conn| {
            let now = Utc::now().fixed_offset();
            let Some(current) = read_config(conn)? else {
                return Ok(Trigger {
                    mode: "dms".to_string(),
                    days: 30,
                    status: "off".to_string(),
                    last_check_in: String::new(),
                    next_check_at: String::new(),
                    paused_until: String::new(),
                    triggered_at: String::new(),
                    stage: 0,
                });
            };
            let enabled = input.enabled.unwrap_or(current.enabled != 0);
            let days = input.days.unwrap_or(current.days);
            let paused_until = match input.pause_days {
                Some(0) => String::new(),
                Some(n) => (now + Duration::days(n)).to_rfc3339(),
                None => current.paused_until.clone(),
            };
            let detail = format!(
                "enabled={};days={};pauseDays={}",
                enabled,
                days,
                input
                    .pause_days
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            let clear_trigger = !enabled || input.pause_days.map(|n| n > 0).unwrap_or(false);
            conn.execute(
                "UPDATE dms_config SET enabled=?1, check_in_interval_days=?2, paused_until=?3,
                 triggered_at=CASE WHEN ?4=1 THEN '' ELSE triggered_at END,
                 trigger_stage=CASE WHEN ?4=1 THEN 0 ELSE trigger_stage END
                 WHERE id=1",
                params![
                    if enabled { 1 } else { 0 },
                    days,
                    paused_until,
                    if clear_trigger { 1 } else { 0 }
                ],
            )?;
            insert_event(conn, &now.to_rfc3339(), "configure", 0, &detail)?;
            Ok(load_trigger(conn)?)
        })
        .map_err(DomainError::from)
}

pub fn evaluate(store: &Store) -> Result<Trigger, DomainError> {
    store
        .write(|conn| {
            let now = Utc::now().fixed_offset();
            let Some(mut config) = read_config(conn)? else {
                return Ok(load_trigger(conn)?);
            };
            let status = status_for(&config, now);
            if status == "paused" || status == "off" {
                return Ok(trigger_from(&config, now));
            }

            if !config.triggered_at.is_empty() {
                let Some(started) = parse_time(&config.triggered_at) else {
                    return Ok(trigger_from(&config, now));
                };
                let elapsed = now.signed_duration_since(started);
                let target = if elapsed
                    >= Duration::days(config.stage1_days + config.stage2_days + config.stage3_days)
                {
                    4
                } else if elapsed >= Duration::days(config.stage1_days + config.stage2_days) {
                    3
                } else if elapsed >= Duration::days(config.stage1_days) {
                    2
                } else {
                    1
                };
                if target > config.stage {
                    for stage in (config.stage + 1)..=target {
                        let kind = match stage {
                            2 => "stage2",
                            3 => "stage3",
                            4 => "final",
                            _ => "stage1",
                        };
                        let detail = match stage {
                            2 => "第一等待期结束，进入第二通知阶段",
                            3 => "第二等待期结束，进入第三通知阶段",
                            4 => "全部等待期结束，进入最终交付阶段",
                            _ => "触发开始，进入第一等待阶段",
                        };
                        insert_event(conn, &now.to_rfc3339(), kind, stage, detail)?;
                        if stage == 4 {
                            let event_id = conn.last_insert_rowid();
                            notify::enqueue_final(conn, &now.to_rfc3339(), event_id)?;
                        }
                    }
                    conn.execute(
                        "UPDATE dms_config SET trigger_stage=?1 WHERE id=1",
                        params![target],
                    )?;
                    config.stage = target;
                }
                return Ok(trigger_from(&config, now));
            }

            if config.enabled == 0 {
                return Ok(trigger_from(&config, now));
            }
            let Some(last) = parse_time(&config.last_check_in) else {
                return Ok(trigger_from(&config, now));
            };
            let due_at = last + Duration::days(config.days);
            if now < due_at {
                return Ok(trigger_from(&config, now));
            }

            let started = now.to_rfc3339();
            conn.execute(
                "UPDATE dms_config SET triggered_at=?1, trigger_stage=1 WHERE id=1",
                params![started],
            )?;
            insert_event(
                conn,
                &started,
                "stage1",
                1,
                "签到超期，触发链启动，进入第一等待阶段",
            )?;
            config.triggered_at = started;
            config.stage = 1;
            Ok(trigger_from(&config, now))
        })
        .map_err(DomainError::from)
}

pub fn events(store: &Store, input: &EventsInput) -> Result<EventPage, DomainError> {
    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    let cursor = match input.cursor.as_deref() {
        Some("") | None => None,
        Some(value) => Some(
            value
                .parse::<i64>()
                .map_err(|_| DomainError::Invalid("cursor 必须是事件 id".to_string()))?,
        ),
    };
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, kind, stage, detail FROM trigger_events
                 WHERE (?1 IS NULL OR id < ?1)
                 ORDER BY id DESC LIMIT ?2",
            )?;
            let mut items = stmt
                .query_map(params![cursor, limit + 1], |r| {
                    Ok(TriggerEvent {
                        id: r.get::<_, i64>(0)?.to_string(),
                        created_at: r.get(1)?,
                        kind: r.get(2)?,
                        stage: r.get(3)?,
                        detail: r.get(4)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let has_more = items.len() as i64 > limit;
            if has_more {
                items.truncate(limit as usize);
            }
            let next_cursor = if has_more {
                items.last().map(|event| event.id.clone())
            } else {
                None
            };
            let total: i64 =
                conn.query_row("SELECT count(*) FROM trigger_events", [], |r| r.get(0))?;
            Ok(EventPage {
                items,
                next_cursor,
                total,
            })
        })
        .map_err(DomainError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_plan_has_default_trigger() {
        let store = Store::open_in_memory(None).unwrap();
        let plan = get(&store).unwrap();
        assert_eq!(plan.holder.name, "");
        assert!(plan.heirs.is_empty());
        assert_eq!(plan.trigger.days, 30);
        assert_eq!(plan.trigger.status, "off");
    }

    #[test]
    fn heirs_and_trigger_are_mapped() {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO user_profile (id, full_name, created_at, updated_at) VALUES (1,'张三','now','now')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO heirs (id,name,relation,is_executor,sort_order,created_at,updated_at) VALUES (1,'李四','配偶',1,1,'now','now')",
                    [],
                )?;
                conn.execute("UPDATE dms_config SET enabled=1, check_in_interval_days=45, last_check_in='2026-11-01T00:00:00+00:00' WHERE id=1", [])?;
                Ok(())
            })
            .unwrap();
        let plan = get(&store).unwrap();
        assert_eq!(plan.holder.name, "张三");
        assert_eq!(plan.heirs[0].role, "executor");
        assert_eq!(plan.trigger.days, 45);
        assert_eq!(plan.trigger.status, "armed");
        assert_eq!(plan.trigger.last_check_in, "2026-11-01T00:00:00+00:00");
        assert!(plan.trigger.next_check_at.starts_with("2026-12-16"));
    }

    #[test]
    fn configure_pause_and_resume_updates_status_and_events() {
        let store = Store::open_in_memory(None).unwrap();
        let paused = configure(
            &store,
            &ConfigureInput {
                enabled: Some(true),
                days: Some(30),
                pause_days: Some(7),
            },
        )
        .unwrap();
        assert_eq!(paused.status, "paused");
        assert!(!paused.paused_until.is_empty());

        let resumed = configure(
            &store,
            &ConfigureInput {
                enabled: Some(true),
                days: Some(30),
                pause_days: Some(0),
            },
        )
        .unwrap();
        assert_eq!(resumed.status, "armed");

        let page = events(
            &store,
            &EventsInput {
                cursor: None,
                limit: Some(20),
            },
        )
        .unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].kind, "configure");
    }

    #[test]
    fn evaluate_due_trigger_starts_once_then_advances() {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                conn.execute("UPDATE dms_config SET enabled=1, check_in_interval_days=1, last_check_in='2026-01-01T00:00:00+00:00' WHERE id=1", [])?;
                conn.execute(
                    "INSERT INTO user_profile (full_name, emergency_contact_name, emergency_contact_email, created_at, updated_at)
                     VALUES ('张三','李四','lisi@example.com','2026-01-01','2026-01-01')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO heirs (name, relation, email, created_at, updated_at)
                     VALUES ('王五','子女','wangwu@example.com','2026-01-01','2026-01-01')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let first = evaluate(&store).unwrap();
        assert_eq!(first.status, "triggered");
        assert_eq!(first.stage, 1);

        let again = evaluate(&store).unwrap();
        assert_eq!(again.status, "triggered");
        assert_eq!(again.stage, 1);

        store
            .write(|conn| {
                conn.execute(
                    "UPDATE dms_config SET triggered_at='2020-01-01T00:00:00+00:00' WHERE id=1",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let advanced = evaluate(&store).unwrap();
        assert_eq!(advanced.stage, 4);
        let page = events(
            &store,
            &EventsInput {
                cursor: None,
                limit: Some(20),
            },
        )
        .unwrap();
        assert!(page.items.iter().any(|event| event.kind == "final"));
        let notifications: i64 = store
            .read(|conn| {
                conn.query_row("SELECT count(*) FROM notification_outbox", [], |r| r.get(0))
            })
            .unwrap();
        assert_eq!(notifications, 2);
    }

    #[test]
    fn invalid_days_are_rejected() {
        let store = Store::open_in_memory(None).unwrap();
        let error = configure(
            &store,
            &ConfigureInput {
                enabled: Some(true),
                days: Some(0),
                pause_days: None,
            },
        )
        .unwrap_err();
        assert!(matches!(error, DomainError::Invalid(_)));
    }
}
