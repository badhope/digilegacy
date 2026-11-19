//! DOM-03 plan: holder profile, heirs and trigger summary.

use rusqlite::OptionalExtension;
use serde::Serialize;

use digilegacy_store::Store;

use crate::assets::DomainError;

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub holder: Holder,
    pub heirs: Vec<Heir>,
    pub trigger: Trigger,
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

            let trigger = conn
                .query_row(
                    "SELECT enabled, check_in_interval_days, coalesce(paused_until,'')
                     FROM dms_config WHERE id=1",
                    [],
                    |r| {
                        let enabled: i64 = r.get(0)?;
                        let days: i64 = r.get(1)?;
                        let paused: String = r.get(2)?;
                        let status = if !paused.is_empty() {
                            "paused".to_string()
                        } else if enabled != 0 {
                            "armed".to_string()
                        } else {
                            "off".to_string()
                        };
                        Ok(Trigger {
                            mode: "dms".to_string(),
                            days,
                            status,
                        })
                    },
                )
                .optional()?
                .unwrap_or(Trigger {
                    mode: "dms".to_string(),
                    days: 30,
                    status: "off".to_string(),
                });

            Ok(Plan {
                holder,
                heirs,
                trigger,
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
                conn.execute("UPDATE dms_config SET enabled=1, check_in_interval_days=45 WHERE id=1", [])?;
                Ok(())
            })
            .unwrap();
        let plan = get(&store).unwrap();
        assert_eq!(plan.holder.name, "张三");
        assert_eq!(plan.heirs[0].role, "executor");
        assert_eq!(plan.trigger.days, 45);
        assert_eq!(plan.trigger.status, "armed");
    }
}
