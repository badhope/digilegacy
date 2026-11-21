//! Human skill registry commands. Skills are prompt context only; source
//! scripts are never executed.

use serde_json::{json, Value};

use digilegacy_domains::skills;
use digilegacy_foundation::PlatformError;

use crate::{map_domain, parse, Application};

impl Application {
    pub fn skills_list(&self, _args: Value) -> Result<Value, PlatformError> {
        let items = skills::list(self.store()).map_err(map_domain)?;
        let total = items.len() as i64;
        Ok(json!({ "items": items, "total": total }))
    }

    pub fn skills_upsert(&self, args: Value) -> Result<Value, PlatformError> {
        let input: skills::Input = parse(args)?;
        let saved = skills::upsert(self.store(), input).map_err(map_domain)?;
        self.audit(
            "skill.upsert",
            &format!(
                "id={};name={};enabled={}",
                saved.id, saved.name, saved.enabled
            ),
        )?;
        Ok(json!(saved))
    }

    pub fn skills_delete(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        let (id, name) = skills::delete(self.store(), id).map_err(map_domain)?;
        self.audit("skill.delete", &format!("id={id};name={name}"))?;
        Ok(json!({ "id": id, "name": name }))
    }
}
