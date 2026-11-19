//! DOM-00 run history projection for the Agent workbench.

use serde::Serialize;

use digilegacy_store::Store;

use crate::assets::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub status: String,
    pub title: String,
    pub steps: i64,
    pub started_at: String,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunsPage {
    pub items: Vec<Run>,
}

pub fn list(store: &Store, status: &str, limit: i64) -> Result<RunsPage, DomainError> {
    let limit = limit.clamp(1, 100);
    let status = status.trim();
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, started_at, task, outcome, rounds
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
                        tokens: 0,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(RunsPage { items })
        })
        .map_err(DomainError::from)
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
}
