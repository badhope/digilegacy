//! DOM-08: LLM usage rollup.
//!
//! Token/cost totals come from `agent_plan_log` only. `agent_run_log` counts
//! executions separately because a run inherits its plan usage; summing both
//! would double count the same tokens.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::assets::DomainError;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryInput {
    #[serde(default)]
    pub days: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub plans: i64,
    pub runs: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tokens: i64,
    pub cost_micros: i64,
    pub priced_plans: i64,
    pub unpriced_plans: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDay {
    pub day: String,
    pub plans: i64,
    pub runs: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tokens: i64,
    pub cost_micros: i64,
    pub unpriced_plans: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageModel {
    pub model: String,
    pub plans: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tokens: i64,
    pub cost_micros: i64,
    pub unpriced_plans: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub since: String,
    pub days: i64,
    pub totals: UsageTotals,
    pub by_day: Vec<UsageDay>,
    pub by_model: Vec<UsageModel>,
}

pub fn summary(store: &Store, days: i64) -> Result<UsageSummary, DomainError> {
    let days = days.clamp(1, 90);
    let since = (Utc::now() - Duration::days(days - 1))
        .format("%Y-%m-%d")
        .to_string();

    store
        .read(|conn| {
            let (plans, tokens_in, tokens_out, cost_micros, priced_plans): (
                i64,
                i64,
                i64,
                i64,
                i64,
            ) = conn.query_row(
                "SELECT count(*),
                            coalesce(sum(tokens_in),0),
                            coalesce(sum(tokens_out),0),
                            coalesce(sum(cost_micros),0),
                            coalesce(sum(CASE WHEN priced=1 THEN 1 ELSE 0 END),0)
                     FROM agent_plan_log
                     WHERE substr(created_at,1,10) >= ?1",
                [&since],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?;
            let runs: i64 = conn.query_row(
                "SELECT count(*) FROM agent_run_log WHERE substr(started_at,1,10) >= ?1",
                [&since],
                |r| r.get(0),
            )?;

            let mut by_day: BTreeMap<String, UsageDay> = BTreeMap::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT substr(created_at,1,10) AS day,
                            count(*), coalesce(sum(tokens_in),0), coalesce(sum(tokens_out),0),
                            coalesce(sum(cost_micros),0),
                            coalesce(sum(CASE WHEN priced=0 THEN 1 ELSE 0 END),0)
                     FROM agent_plan_log
                     WHERE substr(created_at,1,10) >= ?1
                     GROUP BY day",
                )?;
                let rows = stmt.query_map([&since], |r| {
                    let day: String = r.get(0)?;
                    let plans: i64 = r.get(1)?;
                    let tokens_in: i64 = r.get(2)?;
                    let tokens_out: i64 = r.get(3)?;
                    let cost_micros: i64 = r.get(4)?;
                    let unpriced_plans: i64 = r.get(5)?;
                    Ok(UsageDay {
                        day,
                        plans,
                        runs: 0,
                        tokens_in,
                        tokens_out,
                        tokens: tokens_in + tokens_out,
                        cost_micros,
                        unpriced_plans,
                    })
                })?;
                for row in rows {
                    let day = row?;
                    by_day.insert(day.day.clone(), day);
                }
            }
            {
                let mut stmt = conn.prepare(
                    "SELECT substr(started_at,1,10) AS day, count(*)
                     FROM agent_run_log
                     WHERE substr(started_at,1,10) >= ?1
                     GROUP BY day",
                )?;
                let rows = stmt.query_map([&since], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?;
                for row in rows {
                    let (day, runs) = row?;
                    by_day
                        .entry(day.clone())
                        .or_insert_with(|| UsageDay {
                            day,
                            plans: 0,
                            runs: 0,
                            tokens_in: 0,
                            tokens_out: 0,
                            tokens: 0,
                            cost_micros: 0,
                            unpriced_plans: 0,
                        })
                        .runs = runs;
                }
            }

            let mut by_model = Vec::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT model, count(*), coalesce(sum(tokens_in),0),
                            coalesce(sum(tokens_out),0), coalesce(sum(cost_micros),0),
                            coalesce(sum(CASE WHEN priced=0 THEN 1 ELSE 0 END),0)
                     FROM agent_plan_log
                     WHERE substr(created_at,1,10) >= ?1
                     GROUP BY model
                     ORDER BY count(*) DESC, model",
                )?;
                let rows = stmt.query_map([&since], |r| {
                    let model: String = r.get(0)?;
                    let plans: i64 = r.get(1)?;
                    let tokens_in: i64 = r.get(2)?;
                    let tokens_out: i64 = r.get(3)?;
                    let cost_micros: i64 = r.get(4)?;
                    let unpriced_plans: i64 = r.get(5)?;
                    Ok(UsageModel {
                        model,
                        plans,
                        tokens_in,
                        tokens_out,
                        tokens: tokens_in + tokens_out,
                        cost_micros,
                        unpriced_plans,
                    })
                })?;
                for row in rows {
                    by_model.push(row?);
                }
            }

            Ok(UsageSummary {
                since,
                days,
                totals: UsageTotals {
                    plans,
                    runs,
                    tokens_in,
                    tokens_out,
                    tokens: tokens_in + tokens_out,
                    cost_micros,
                    priced_plans,
                    unpriced_plans: plans - priced_plans,
                },
                by_day: by_day.into_values().collect(),
                by_model,
            })
        })
        .map_err(DomainError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(store: &Store) {
        let now = Utc::now().to_rfc3339();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_plan_log
                     (id, created_at, task, model, tokens_in, tokens_out, cost_micros, priced, steps_json)
                     VALUES ('p1', ?1, 'A', 'mock-model', 100, 20, 140, 1, '[]')",
                    [&now],
                )?;
                conn.execute(
                    "INSERT INTO agent_plan_log
                     (id, created_at, task, model, tokens_in, tokens_out, cost_micros, priced, steps_json)
                     VALUES ('p2', ?1, 'B', 'mock-model', 10, 5, 0, 0, '[]')",
                    [&now],
                )?;
                conn.execute(
                    "INSERT INTO agent_run_log
                     (started_at, task, outcome, tokens_in, tokens_out, cost_micros, cost_priced)
                     VALUES (?1, 'A', 'succeeded', 100, 20, 140, 1)",
                    [&now],
                )?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn totals_do_not_double_count_run_tokens() {
        let store = Store::open_in_memory(None).unwrap();
        seed(&store);
        let summary = summary(&store, 30).unwrap();
        assert_eq!(summary.totals.plans, 2);
        assert_eq!(summary.totals.runs, 1);
        assert_eq!(summary.totals.tokens, 135);
        assert_eq!(summary.totals.unpriced_plans, 1);
        assert_eq!(summary.by_day.len(), 1);
        assert_eq!(summary.by_day[0].runs, 1);
        assert_eq!(summary.by_model[0].model, "mock-model");
    }
}
