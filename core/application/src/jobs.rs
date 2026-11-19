//! Durable background jobs for platform work that outlives one HTTP request.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::{json, Value};

use digilegacy_domains::{approvals, runs};
use digilegacy_foundation::PlatformError;
use digilegacy_store::Store;

use crate::runtime;
use crate::Application;

static JOB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetail {
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub progress: f64,
    pub payload: Value,
    pub result: Value,
    pub error_code: String,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: String,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn internal(message: impl Into<String>) -> PlatformError {
    PlatformError::new("INTERNAL", message.into())
}

pub fn enqueue(store: &Store, kind: &str, payload: Value) -> Result<String, PlatformError> {
    let created = now();
    let id = format!(
        "job_{}_{}_{}",
        Utc::now().timestamp_millis(),
        JOB_COUNTER.fetch_add(1, Ordering::Relaxed),
        kind.replace('.', "_")
    );
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO jobs
                 (id, kind, status, progress, payload_json, result_json, created_at, updated_at)
                 VALUES (?1, ?2, 'queued', 0, ?3, '{}', ?4, ?4)",
                rusqlite::params![id, kind, payload.to_string(), created],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("任务入队失败: {e}")))?;
    Ok(id)
}

fn claim_next(store: &Store) -> Result<Option<Job>, PlatformError> {
    store
        .write(|conn| {
            let tx = conn.transaction()?;
            let row = tx
                .query_row(
                    "SELECT id, kind, payload_json FROM jobs
                     WHERE status='queued' ORDER BY created_at, id LIMIT 1",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((id, kind, payload_json)) = row else {
                tx.commit()?;
                return Ok(None);
            };
            tx.execute(
                "UPDATE jobs SET status='running', updated_at=?2 WHERE id=?1 AND status='queued'",
                rusqlite::params![id, now()],
            )?;
            tx.commit()?;
            Ok(Some(Job {
                id,
                kind,
                payload: serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({})),
            }))
        })
        .map_err(|e| internal(format!("领取任务失败: {e}")))
}

fn finish(store: &Store, id: &str, result: Value) -> Result<(), PlatformError> {
    let finished = now();
    store
        .write(|conn| {
            conn.execute(
                "UPDATE jobs
                 SET status='succeeded', progress=1, result_json=?2, updated_at=?3, finished_at=?3
                 WHERE id=?1",
                rusqlite::params![id, result.to_string(), finished],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("完成任务失败: {e}")))?;
    Ok(())
}

fn fail(store: &Store, id: &str, code: &str, message: &str) -> Result<(), PlatformError> {
    let finished = now();
    let result = json!({ "error": message });
    store
        .write(|conn| {
            conn.execute(
                "UPDATE jobs
                 SET status='failed', result_json=?2, error_code=?3, updated_at=?4, finished_at=?4
                 WHERE id=?1",
                rusqlite::params![id, result.to_string(), code, finished],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("标记任务失败时出错: {e}")))?;
    Ok(())
}

fn requeue_stale(store: &Store) -> Result<(), PlatformError> {
    let cutoff = (Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
    store
        .write(|conn| {
            conn.execute(
                "UPDATE jobs SET status='queued', updated_at=?2
                 WHERE kind='agent.resume' AND status='running' AND updated_at < ?1",
                rusqlite::params![cutoff, now()],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("回收僵尸任务失败: {e}")))?;
    Ok(())
}

fn execute(app: &Application, job: &Job) -> Result<Value, PlatformError> {
    match job.kind.as_str() {
        "agent.resume" => {
            let approval_id = job
                .payload
                .get("approvalId")
                .and_then(Value::as_str)
                .ok_or_else(|| PlatformError::invalid_args("agent.resume 缺少 approvalId"))?;
            let approval = approvals::get(app.store(), approval_id)
                .map_err(|e| PlatformError::new("NOT_FOUND", e.to_string()))?;
            runtime::resume(app, &approval)?;
            let run = runs::get(app.store(), &approval.run_id)
                .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;
            Ok(json!({
                "approvalId": approval.id,
                "runId": approval.run_id,
                "runStatus": run.status,
            }))
        }
        other => Err(PlatformError::new(
            "UNSUPPORTED",
            format!("未知任务类型: {other}"),
        )),
    }
}

/// Claim and run at most one queued job. Returns false when the queue is empty.
pub fn run_one(app: &Application) -> Result<bool, PlatformError> {
    let Some(job) = claim_next(app.store())? else {
        return Ok(false);
    };
    match execute(app, &job) {
        Ok(result) => finish(app.store(), &job.id, result)?,
        Err(error) => {
            tracing::warn!(job = %job.id, code = %error.code, "background job failed");
            fail(app.store(), &job.id, &error.code, &error.message)?;
        }
    }
    Ok(true)
}

pub fn get(store: &Store, id: &str) -> Result<JobDetail, PlatformError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, kind, status, progress, payload_json, result_json,
                        error_code, created_at, updated_at, finished_at
                 FROM jobs WHERE id=?1",
                [id],
                |row| {
                    let payload: String = row.get(4)?;
                    let result: String = row.get(5)?;
                    Ok(JobDetail {
                        job_id: row.get(0)?,
                        kind: row.get(1)?,
                        status: row.get(2)?,
                        progress: row.get(3)?,
                        payload: serde_json::from_str(&payload).unwrap_or_else(|_| json!({})),
                        result: serde_json::from_str(&result).unwrap_or_else(|_| json!({})),
                        error_code: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        finished_at: row.get(9)?,
                    })
                },
            )
            .optional()
        })
        .map_err(|e| internal(format!("读取任务失败: {e}")))?
        .ok_or_else(|| PlatformError::not_found("后台任务"))
}

/// Start a single in-process worker. The server owns the process lifetime; CLI
/// callers that need deterministic completion use the synchronous command path.
pub fn spawn_worker(app: Application) {
    let _ = std::thread::Builder::new()
        .name("digilegacy-jobs".to_string())
        .spawn(move || {
            if let Err(error) = requeue_stale(app.store()) {
                tracing::warn!(code = %error.code, message = %error.message, "job worker recovery failed");
            }
            loop {
                match run_one(&app) {
                    Ok(true) => continue,
                    Ok(false) => std::thread::sleep(Duration::from_millis(250)),
                    Err(error) => {
                        tracing::warn!(code = %error.code, message = %error.message, "job worker loop error");
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::Application;
    use digilegacy_store::Store;

    fn app() -> Application {
        let app = Application::new(Arc::new(Store::open_in_memory(None).unwrap()));
        digilegacy_domains::policy::seed_if_empty(app.store()).unwrap();
        app
    }

    #[test]
    fn queued_job_runs_approval_resume() {
        let app = app();
        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "后台审批",
                    "steps": [{ "tool": "assets.create", "args": {
                        "name": "后台资产", "kind": "other", "platform": "job", "value": "1", "note": ""
                    }}]
                }),
            )
            .unwrap();
        let approval_id = app
            .dispatch("agent.approvals.list", json!({ "status": "pending" }))
            .unwrap()["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        app.dispatch(
            "agent.approvals.decide",
            json!({ "id": approval_id, "decision": "approve", "reason": "后台执行", "background": true }),
        )
        .unwrap();
        assert!(run_one(&app).unwrap());
        let detail = app
            .dispatch("agent.runs.get", json!({ "id": run["id"] }))
            .unwrap();
        assert_eq!(detail["status"], "succeeded");
        let job = app
            .dispatch("jobs.get", json!({ "id": "missing" }))
            .unwrap_err();
        assert_eq!(job.code, "NOT_FOUND");
    }

    #[test]
    fn job_detail_lists_queued_job() {
        let app = app();
        let id = enqueue(app.store(), "agent.resume", json!({ "approvalId": "a1" })).unwrap();
        let detail = get(app.store(), &id).unwrap();
        assert_eq!(detail.status, "queued");
        assert_eq!(detail.payload["approvalId"], "a1");
    }
}
