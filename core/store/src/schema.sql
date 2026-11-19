-- DigiLegacy schema v16.
-- 31 tables. The v8 lineage is preserved; new tables are additive so an old
-- database can be migrated without recreating it.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS platforms (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    name_en TEXT,
    category TEXT NOT NULL,
    subtype TEXT,
    has_property INTEGER DEFAULT 0,
    has_personality INTEGER DEFAULT 0,
    inheritable INTEGER DEFAULT 0,
    required_documents TEXT,
    process_description TEXT,
    estimated_difficulty TEXT,
    official_policy_url TEXT,
    estimated_time_days TEXT,
    notes TEXT
);

CREATE TABLE IF NOT EXISTS policies (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    name_en TEXT DEFAULT '',
    category TEXT NOT NULL,
    subtype TEXT DEFAULT '',
    has_property INTEGER DEFAULT 0,
    has_personality INTEGER DEFAULT 0,
    inheritable INTEGER DEFAULT 0,
    required_documents TEXT DEFAULT '',
    process_description TEXT DEFAULT '',
    estimated_difficulty TEXT DEFAULT '',
    official_policy_url TEXT DEFAULT '',
    estimated_time_days TEXT DEFAULT '',
    notes TEXT DEFAULT '',
    data_source TEXT DEFAULT '',
    inheritance_form_url TEXT DEFAULT '',
    monetization_path TEXT DEFAULT '',
    legal_template_ids TEXT DEFAULT '',
    risk_warnings TEXT DEFAULT '',
    tags TEXT DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS policy_overrides (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    name_en TEXT DEFAULT '',
    category TEXT NOT NULL,
    subtype TEXT DEFAULT '',
    has_property INTEGER DEFAULT 0,
    has_personality INTEGER DEFAULT 0,
    inheritable INTEGER DEFAULT 0,
    required_documents TEXT DEFAULT '',
    process_description TEXT DEFAULT '',
    estimated_difficulty TEXT DEFAULT '',
    official_policy_url TEXT DEFAULT '',
    estimated_time_days TEXT DEFAULT '',
    notes TEXT DEFAULT '',
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS digital_assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    platform_id INTEGER NOT NULL,
    account_identifier TEXT,
    account_description TEXT,
    category TEXT NOT NULL,
    estimated_value REAL DEFAULT 0,
    value_currency TEXT DEFAULT 'CNY',
    disposal_method TEXT NOT NULL DEFAULT 'no_action',
    heir_name TEXT,
    heir_contact TEXT,
    notes TEXT DEFAULT '',
    credential_enc TEXT DEFAULT '',
    is_confirmed INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    last_verified_at TEXT DEFAULT '',
    source TEXT DEFAULT 'manual',
    policy_snapshot_hash TEXT DEFAULT '',
    FOREIGN KEY (platform_id) REFERENCES platforms(id)
);

CREATE TABLE IF NOT EXISTS security_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    kdf_salt TEXT NOT NULL,
    verifier TEXT NOT NULL,
    recovery_seal TEXT DEFAULT '',
    created_at TEXT NOT NULL,
    wrapped_dek TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS user_profile (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    full_name TEXT,
    phone TEXT,
    email TEXT,
    id_last4 TEXT,
    emergency_contact_name TEXT,
    emergency_contact_phone TEXT,
    emergency_contact_relation TEXT,
    emergency_contact_email TEXT,
    heir_primary_name TEXT,
    heir_primary_phone TEXT,
    heir_primary_relation TEXT,
    heir_primary_email TEXT,
    check_in_interval_days INTEGER DEFAULT 30,
    last_check_in TEXT,
    dms_enabled INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_reviewed_at TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS heirs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    relation TEXT DEFAULT '',
    phone TEXT DEFAULT '',
    email TEXT DEFAULT '',
    share_note TEXT DEFAULT '',
    is_executor INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS check_in_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    checked_at TEXT NOT NULL,
    status TEXT NOT NULL,
    note TEXT
);

CREATE TABLE IF NOT EXISTS dms_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER DEFAULT 0,
    check_in_interval_days INTEGER DEFAULT 30,
    stage1_wait_days INTEGER DEFAULT 3,
    stage2_wait_days INTEGER DEFAULT 7,
    stage3_wait_days INTEGER DEFAULT 14,
    last_check_in TEXT,
    paused_until TEXT DEFAULT '',
    triggered_at TEXT DEFAULT '',
    trigger_stage INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS trigger_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    kind TEXT NOT NULL,
    stage INTEGER NOT NULL DEFAULT 0,
    detail TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS notification_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    trigger_event_id INTEGER NOT NULL DEFAULT 0,
    kind TEXT NOT NULL DEFAULT 'trigger.final',
    recipient TEXT NOT NULL DEFAULT '',
    recipient_role TEXT NOT NULL DEFAULT '',
    subject TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT NOT NULL DEFAULT '',
    sent_at TEXT DEFAULT '',
    UNIQUE(trigger_event_id, recipient)
);

CREATE INDEX IF NOT EXISTS idx_notification_outbox_status ON notification_outbox(status, id);

CREATE TABLE IF NOT EXISTS connections (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    config_enc TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    last_check_at TEXT DEFAULT '',
    last_status TEXT DEFAULT 'unconfigured',
    last_error TEXT DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(kind, label)
);

CREATE TABLE IF NOT EXISTS review_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT NOT NULL,
    item_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    ai_verdict TEXT NOT NULL,
    ai_note TEXT DEFAULT '',
    human_verdict TEXT,
    human_note TEXT DEFAULT '',
    decided_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_chat_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    model TEXT DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_run_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    task TEXT NOT NULL,
    outcome TEXT NOT NULL DEFAULT 'final',
    final_text TEXT DEFAULT '',
    rounds INTEGER DEFAULT 0,
    tools_used TEXT DEFAULT '',
    steps_json TEXT DEFAULT '[]',
    duration_ms INTEGER DEFAULT 0,
    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,
    cost_micros INTEGER DEFAULT 0,
    cost_priced INTEGER DEFAULT 0,
    plan_id TEXT DEFAULT '',
    cancel_requested INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS agent_plan_log (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    task TEXT NOT NULL,
    model TEXT DEFAULT '',
    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,
    cost_micros INTEGER DEFAULT 0,
    priced INTEGER DEFAULT 0,
    steps_json TEXT DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS heir_recovery (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    threshold INTEGER NOT NULL,
    total INTEGER NOT NULL,
    fingerprint TEXT NOT NULL,
    created_at TEXT NOT NULL,
    passphrase_protected INTEGER DEFAULT 0,
    kdf_salt TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS custody_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    backup_location_enc TEXT DEFAULT '',
    recovery_code_location_enc TEXT DEFAULT '',
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS imap_seen (
    uid INTEGER PRIMARY KEY,
    sender TEXT DEFAULT '',
    seen_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY,
    at TEXT NOT NULL,
    kind TEXT NOT NULL,
    subject TEXT NOT NULL DEFAULT '',
    outcome TEXT NOT NULL DEFAULT 'ok',
    detail_enc TEXT DEFAULT '',
    prev_hash TEXT NOT NULL DEFAULT '',
    hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS policy_provenance (
    platform_id INTEGER PRIMARY KEY,
    status TEXT NOT NULL,
    source_url TEXT DEFAULT '',
    verified_at TEXT DEFAULT '',
    verified_by TEXT DEFAULT '',
    evidence_hash TEXT DEFAULT '',
    note TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS policy_candidates (
    id INTEGER PRIMARY KEY,
    platform_id INTEGER NOT NULL,
    fetched_at TEXT NOT NULL,
    source TEXT NOT NULL,
    payload_enc TEXT NOT NULL,
    diff_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reviewed_at TEXT DEFAULT '',
    reviewed_by TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_policy_candidates_status
    ON policy_candidates (status, fetched_at DESC);

CREATE TABLE IF NOT EXISTS policy_sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    source TEXT NOT NULL DEFAULT '',
    cursor TEXT NOT NULL DEFAULT '',
    etag TEXT NOT NULL DEFAULT '',
    last_sync_at TEXT NOT NULL DEFAULT '',
    last_status TEXT NOT NULL DEFAULT 'never',
    last_error TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS drill_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT DEFAULT '',
    scenario TEXT NOT NULL DEFAULT 'default',
    status TEXT NOT NULL DEFAULT 'running',
    score INTEGER NOT NULL DEFAULT 0,
    findings_json TEXT NOT NULL DEFAULT '{}',
    notes TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS deliverable_exports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    locale TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    template_version TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    path TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS llm_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at TEXT NOT NULL,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    cost_estimate REAL NOT NULL DEFAULT 0,
    run_id TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    settings_json TEXT DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS agent_approvals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    tool TEXT NOT NULL,
    object_key TEXT DEFAULT '',
    diff_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending',
    reason TEXT DEFAULT '',
    created_at TEXT NOT NULL,
    decided_at TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    progress REAL NOT NULL DEFAULT 0,
    payload_json TEXT NOT NULL DEFAULT '{}',
    result_json TEXT NOT NULL DEFAULT '{}',
    error_code TEXT DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    finished_at TEXT DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_assets_category ON digital_assets(category);
CREATE INDEX IF NOT EXISTS idx_policies_category ON policies(category);
CREATE INDEX IF NOT EXISTS idx_policies_name ON policies(name);
CREATE INDEX IF NOT EXISTS idx_assets_platform ON digital_assets(platform_id);
CREATE INDEX IF NOT EXISTS idx_assets_disposal ON digital_assets(disposal_method);
CREATE INDEX IF NOT EXISTS idx_assets_created ON digital_assets(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_assets_identifier ON digital_assets(account_identifier);
CREATE INDEX IF NOT EXISTS idx_checkin_checked_at ON check_in_log(checked_at);
CREATE INDEX IF NOT EXISTS idx_trigger_events_at ON trigger_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_heirs_order ON heirs(sort_order, id);
CREATE INDEX IF NOT EXISTS idx_review_item ON review_records(profile_id, item_key, id DESC);
CREATE INDEX IF NOT EXISTS idx_ai_chat_created ON ai_chat_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_at ON audit_log(at DESC);
CREATE INDEX IF NOT EXISTS idx_jobs_status_created ON jobs(status, created_at);
CREATE INDEX IF NOT EXISTS idx_connections_kind ON connections(kind, enabled);
