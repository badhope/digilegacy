//! AGT-05: task-to-steps planner.
//!
//! Any OpenAI-compatible chat endpoint can be configured (DeepSeek by default).
//! The planner never executes tools: it only proposes registered calls, and the
//! caller must review or start the run. Writes still create approvals.

use std::env;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use digilegacy_contracts::{resolve_tool, TOOLS};

const MAX_STEPS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
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
    fn from_provider(value: Option<&Value>, config: &PlannerConfig) -> Self {
        let tokens_in = value
            .and_then(|v| v.get("prompt_tokens").or_else(|| v.get("input_tokens")))
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        let tokens_out = value
            .and_then(|v| {
                v.get("completion_tokens")
                    .or_else(|| v.get("output_tokens"))
            })
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        let priced = tokens_in + tokens_out > 0
            && (config.input_cost_micros_per_million > 0
                || config.output_cost_micros_per_million > 0);
        let cost_micros = if priced {
            let input = tokens_in.saturating_mul(config.input_cost_micros_per_million) / 1_000_000;
            let output =
                tokens_out.saturating_mul(config.output_cost_micros_per_million) / 1_000_000;
            input.saturating_add(output)
        } else {
            0
        };
        Self {
            tokens_in,
            tokens_out,
            cost_micros,
            priced,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub steps: Vec<PlanStep>,
    pub model: String,
    pub usage: Usage,
}

#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout: Duration,
    pub input_cost_micros_per_million: i64,
    pub output_cost_micros_per_million: i64,
}

impl PlannerConfig {
    pub fn from_env() -> Result<Self, PlannerError> {
        let base_url = env::var("DIGILEGACY_LLM_BASE_URL").unwrap_or_default();
        let api_key = env::var("DIGILEGACY_LLM_API_KEY").unwrap_or_default();
        if base_url.trim().is_empty() || api_key.trim().is_empty() {
            return Err(PlannerError::NotConfigured(
                "未配置规划器：设置 DIGILEGACY_LLM_BASE_URL 与 DIGILEGACY_LLM_API_KEY".to_string(),
            ));
        }
        let model =
            env::var("DIGILEGACY_LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
        let timeout = env::var("DIGILEGACY_LLM_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(30);
        let input_cost_micros_per_million =
            env::var("DIGILEGACY_LLM_INPUT_COST_MICROS_PER_MILLION")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0)
                .max(0);
        let output_cost_micros_per_million =
            env::var("DIGILEGACY_LLM_OUTPUT_COST_MICROS_PER_MILLION")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0)
                .max(0);
        Ok(Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            api_key,
            model,
            timeout: Duration::from_secs(timeout.clamp(5, 120)),
            input_cost_micros_per_million,
            output_cost_micros_per_million,
        })
    }
}

#[derive(Debug, Clone)]
pub enum PlannerError {
    NotConfigured(String),
    Network(String),
    Timeout(String),
    Auth(String),
    Provider(String),
    Invalid(String),
}

impl PlannerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured(_) => "UNSUPPORTED",
            Self::Network(_) => "NETWORK",
            Self::Timeout(_) => "TIMEOUT",
            Self::Auth(_) => "PROVIDER_AUTH",
            Self::Provider(_) => "PROVIDER_ERROR",
            Self::Invalid(_) => "INVALID_ARGS",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotConfigured(message)
            | Self::Network(message)
            | Self::Timeout(message)
            | Self::Auth(message)
            | Self::Provider(message)
            | Self::Invalid(message) => message.clone(),
        }
    }
}

fn tool_catalog() -> String {
    TOOLS
        .iter()
        .map(|tool| {
            format!(
                "### {}\ncommand: {}\npermission: {}\n{}\ninput schema: {}",
                tool.name, tool.command, tool.permission, tool.description, tool.input_schema
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn system_prompt(max_steps: usize) -> String {
    format!(
        "你是 DigiLegacy 数字遗产平台的计划器。把用户任务拆成可执行的工具步骤。\n\
         只能使用下方注册工具；不得编造工具名，不得调用 agent.runs.start 或 agent.approvals.decide。\n\
         最多 {max_steps} 步；只输出 JSON 对象：{{\"steps\":[{{\"tool\":\"工具名\",\"args\":{{...}}}}]}}。\n\
         args 必须符合对应 input schema。只读工具可直接读取；permission=write 的工具会由平台转人工审批，不要试图绕过。\n\n\
         注册工具：\n{}",
        tool_catalog()
    )
}

fn strip_json_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let without_start = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    without_start.trim().trim_end_matches("```").trim()
}

fn parse_plan(text: &str, max_steps: usize) -> Result<Vec<PlanStep>, PlannerError> {
    let raw = strip_json_fence(text);
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| PlannerError::Invalid(format!("规划器返回不是合法 JSON: {e}")))?;
    let steps_value = value
        .get("steps")
        .cloned()
        .or_else(|| value.as_array().cloned().map(Value::Array))
        .ok_or_else(|| PlannerError::Invalid("规划器返回缺少 steps 数组".to_string()))?;
    let items = steps_value
        .as_array()
        .ok_or_else(|| PlannerError::Invalid("steps 必须是数组".to_string()))?;
    if items.is_empty() || items.len() > max_steps {
        return Err(PlannerError::Invalid(format!(
            "规划步骤数量必须在 1..={max_steps}"
        )));
    }
    let mut steps = Vec::new();
    for item in items {
        let tool = item
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| PlannerError::Invalid("步骤缺少 tool".to_string()))?;
        let command = resolve_tool(tool)
            .ok_or_else(|| PlannerError::Invalid(format!("规划器选择未注册工具: {tool}")))?;
        if command == "agent.runs.start" || command == "agent.approvals.decide" {
            return Err(PlannerError::Invalid(format!(
                "规划器不允许调用编排命令: {tool}"
            )));
        }
        let args = item.get("args").cloned().unwrap_or_else(|| json!({}));
        if !args.is_object() {
            return Err(PlannerError::Invalid(format!("{tool} 的 args 必须是对象")));
        }
        steps.push(PlanStep {
            tool: tool.to_string(),
            args,
        });
    }
    Ok(steps)
}

pub fn plan_with_config(
    config: &PlannerConfig,
    task: &str,
    max_steps: usize,
) -> Result<Plan, PlannerError> {
    if task.trim().is_empty() {
        return Err(PlannerError::Invalid("任务不能为空".to_string()));
    }
    let max_steps = max_steps.clamp(1, MAX_STEPS);
    let url = if config.base_url.ends_with("/chat/completions") {
        config.base_url.clone()
    } else {
        format!("{}/chat/completions", config.base_url)
    };
    let body = json!({
        "model": config.model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": system_prompt(max_steps) },
            { "role": "user", "content": task.trim() }
        ]
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(config.timeout)
        .build()
        .map_err(|e| PlannerError::Network(format!("创建规划器客户端失败: {e}")))?;
    let response = client
        .post(&url)
        .bearer_auth(&config.api_key)
        .json(&body)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                PlannerError::Timeout(format!("规划器请求超时: {e}"))
            } else {
                PlannerError::Network(format!("规划器请求失败: {e}"))
            }
        })?;
    let status = response.status();
    let text = response
        .text()
        .map_err(|e| PlannerError::Network(format!("读取规划器响应失败: {e}")))?;
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(PlannerError::Auth(format!(
            "规划器拒绝认证（HTTP {}）",
            status.as_u16()
        )));
    }
    if !status.is_success() {
        let snippet: String = text.chars().take(500).collect();
        return Err(PlannerError::Provider(format!(
            "规划器返回 HTTP {}: {snippet}",
            status.as_u16()
        )));
    }
    let payload: Value = serde_json::from_str(&text)
        .map_err(|e| PlannerError::Provider(format!("规划器响应不是 JSON: {e}")))?;
    let content = payload
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            PlannerError::Provider("规划器响应缺少 choices[0].message.content".to_string())
        })?;
    let usage = Usage::from_provider(payload.get("usage"), config);
    let steps = parse_plan(content, max_steps)?;
    Ok(Plan {
        steps,
        model: config.model.clone(),
        usage,
    })
}

pub fn plan(task: &str, max_steps: usize) -> Result<Plan, PlannerError> {
    let config = PlannerConfig::from_env()?;
    plan_with_config(&config, task, max_steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_usage_and_prices_it() {
        let config = PlannerConfig {
            base_url: "http://x".into(),
            api_key: "k".into(),
            model: "m".into(),
            timeout: Duration::from_secs(1),
            input_cost_micros_per_million: 1_000_000,
            output_cost_micros_per_million: 2_000_000,
        };
        let usage = Usage::from_provider(
            Some(&json!({ "prompt_tokens": 120, "completion_tokens": 45 })),
            &config,
        );
        assert_eq!(usage.tokens_in, 120);
        assert_eq!(usage.tokens_out, 45);
        assert_eq!(usage.cost_micros, 210);
        assert!(usage.priced);
        let fallback = Usage::from_provider(
            Some(&json!({ "input_tokens": 7, "output_tokens": 3 })),
            &config,
        );
        assert_eq!((fallback.tokens_in, fallback.tokens_out), (7, 3));
        assert_eq!(fallback.cost_micros, 13);
    }

    #[test]
    fn parses_object_and_rejects_unknown_tools() {
        let steps = parse_plan(r#"{"steps":[{"tool":"overview.get","args":{}}]}"#, 20).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].tool, "overview.get");
        assert!(matches!(
            parse_plan(r#"{"steps":[{"tool":"rm -rf","args":{}}]}"#, 20),
            Err(PlannerError::Invalid(_))
        ));
        assert!(matches!(
            parse_plan(r#"{"steps":[{"tool":"agent.runs.start"}]}"#, 20),
            Err(PlannerError::Invalid(_))
        ));
    }

    #[test]
    fn calls_openai_compatible_endpoint() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let content = serde_json::json!({
                "steps": [{ "tool": "overview.get", "args": {} }]
            })
            .to_string();
            let body = serde_json::json!({
                "choices": [{ "message": { "role": "assistant", "content": content } }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let config = PlannerConfig {
            base_url: format!("http://{address}/v1"),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            timeout: Duration::from_secs(5),
            input_cost_micros_per_million: 0,
            output_cost_micros_per_million: 0,
        };
        let plan = plan_with_config(&config, "读取概览", 3).unwrap();
        assert_eq!(plan.model, "test-model");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].tool, "overview.get");
        assert_eq!(plan.usage.tokens_in, 10);
        assert_eq!(plan.usage.tokens_out, 5);
        assert!(!plan.usage.priced);
    }

    #[test]
    fn accepts_array_and_fenced_json() {
        let steps = parse_plan(
            "```json\n[{\"tool\":\"assets.list\",\"args\":{\"status\":\"all\"}}]\n```",
            20,
        )
        .unwrap();
        assert_eq!(steps[0].tool, "assets.list");
        assert_eq!(steps[0].args["status"], "all");
    }
}
