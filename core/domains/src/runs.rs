//! DOM-00 run history projection for the Agent workbench.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use digilegacy_store::{Store, StoreError};

use crate::assets::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub status: String,
    pub title: String,
    pub steps: i64,
    pub started_at: String,
    pub duration_ms: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunsPage {
    pub items: Vec<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStep {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
    pub status: String,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub id: String,
    pub status: String,
    pub task: String,
    pub steps: Vec<RunStep>,
    pub tools_used: String,
    pub rounds: i64,
    pub started_at: String,
    pub duration_ms: i64,
    pub final_text: String,
    pub tokens: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_micros: i64,
    pub usage_priced: bool,
    pub plan_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_micros: i64,
    pub priced: bool,
}

impl Usage {
    pub fn total_tokens(&self) -> i64 {
        self.tokens_in + self.tokens_out
    }
}

/// Persist one planner call. The plan log is append-only and separate from run
/// history so a plan that the user never starts still counts toward LLM usage.
pub fn record_plan(
    store: &Store,
    id: &str,
    task: &str,
    model: &str,
    usage: &Usage,
    steps_json: &str,
) -> Result<(), DomainError> {
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO agent_plan_log
                 (id, created_at, task, model, tokens_in, tokens_out, cost_micros, priced, steps_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    id,
                    chrono::Utc::now().to_rfc3339(),
                    task,
                    model,
                    usage.tokens_in,
                    usage.tokens_out,
                    usage.cost_micros,
                    if usage.priced { 1 } else { 0 },
                    steps_json
                ],
            )?;
            Ok(())
        })
        .map_err(DomainError::from)
}

pub fn plan_usage(store: &Store, id: &str) -> Option<Usage> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT tokens_in, tokens_out, cost_micros, priced
                 FROM agent_plan_log WHERE id=?1",
                [id],
                |r| {
                    Ok(Usage {
                        tokens_in: r.get(0)?,
                        tokens_out: r.get(1)?,
                        cost_micros: r.get(2)?,
                        priced: r.get::<_, i64>(3)? != 0,
                    })
                },
            )
            .optional()
        })
        .ok()
        .flatten()
}

pub fn list(store: &Store, status: &str, limit: i64) -> Result<RunsPage, DomainError> {
    let limit = limit.clamp(1, 100);
    let status = status.trim();
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, started_at, task, outcome, rounds, duration_ms,
                        coalesce(tokens_in,0) + coalesce(tokens_out,0) AS tokens
                 FROM agent_run_log
                 WHERE (?1 = 'all' OR outcome = ?1)
                 ORDER BY id DESC LIMIT ?2",
            )?;
            let items = stmt
                .query_map(rusqlite::params![status, limit], |r| {
                    Ok(Run {
                        id: r.get::<_, i64>(0)?.to_string(),
                        started_at: r.get(1)?,
                        title: r.get(2)?,
                        status: r.get(3)?,
                        steps: r.get(4)?,
                        duration_ms: r.get(5)?,
                        tokens: r.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(RunsPage { items })
        })
        .map_err(DomainError::from)
}

pub fn get(store: &Store, id: &str) -> Result<RunDetail, DomainError> {
    let id_num: i64 = id
        .trim()
        .parse()
        .map_err(|_| DomainError::Invalid(format!("运行 ID 无效: {id}")))?;
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, started_at, task, outcome, rounds, tools_used, steps_json, final_text, duration_ms,
                        coalesce(tokens_in,0), coalesce(tokens_out,0), coalesce(cost_micros,0),
                        coalesce(cost_priced,0), coalesce(plan_id,'')
                 FROM agent_run_log WHERE id=?1",
                [id_num],
                |r| {
                    let steps_json: String = r.get(6)?;
                    Ok(RunDetail {
                        id: r.get::<_, i64>(0)?.to_string(),
                        started_at: r.get(1)?,
                        task: r.get(2)?,
                        status: r.get(3)?,
                        rounds: r.get(4)?,
                        tools_used: r.get(5)?,
                        steps: parse_steps(&steps_json),
                        final_text: r.get(7)?,
                        duration_ms: r.get(8)?,
                        tokens_in: r.get(9)?,
                        tokens_out: r.get(10)?,
                        tokens: r.get::<_, i64>(9)? + r.get::<_, i64>(10)?,
                        cost_micros: r.get(11)?,
                        usage_priced: r.get::<_, i64>(12)? != 0,
                        plan_id: r.get(13)?,
                    })
                },
            )
        })
        .map_err(|e| match e {
            StoreError::Sql(rusqlite::Error::QueryReturnedNoRows) => {
                DomainError::NotFound(format!("运行记录不存在: {id}"))
            }
            other => DomainError::from(other),
        })
}

pub fn parse_steps(raw: &str) -> Vec<RunStep> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_newest_first_and_filters() {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_run_log (started_at, task, outcome, rounds) VALUES ('t1','A','succeeded',1)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO agent_run_log (started_at, task, outcome, rounds) VALUES ('t2','B','failed',2)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let all = list(&store, "all", 20).unwrap();
        assert_eq!(all.items.len(), 2);
        assert_eq!(all.items[0].title, "B");
        let failed = list(&store, "failed", 20).unwrap();
        assert_eq!(failed.items.len(), 1);
        assert_eq!(failed.items[0].title, "B");
    }

    #[test]
    fn gets_steps_and_rejects_unknown() {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_run_log (started_at, task, outcome, rounds, tools_used, steps_json, final_text)
                     VALUES ('t1','A','succeeded',1,'overview.get','[{\"tool\":\"overview.get\",\"args\":{},\"status\":\"done\"}]','ok')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let detail = get(&store, "1").unwrap();
        assert_eq!(detail.task, "A");
        assert_eq!(detail.steps.len(), 1);
        assert_eq!(detail.steps[0].tool, "overview.get");
        assert!(matches!(get(&store, "99"), Err(DomainError::NotFound(_))));
    }
}
