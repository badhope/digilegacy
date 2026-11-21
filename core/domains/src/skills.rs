//! Agent skill registry (SKILL-01).
//!
//! Skills are reviewable prompt fragments, not executable scripts. The
//! planner receives enabled skill instructions as additional context.

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::assets::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub enabled: bool,
    pub builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub instructions: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Skill> {
    Ok(Skill {
        id: row.get::<_, i64>(0)?.to_string(),
        name: row.get(1)?,
        description: row.get(2)?,
        instructions: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        builtin: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn list(store: &Store) -> Result<Vec<Skill>, DomainError> {
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, instructions, enabled, builtin, created_at, updated_at
                 FROM agent_skills ORDER BY enabled DESC, name, id",
            )?;
            let rows = stmt
                .query_map([], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .map_err(DomainError::from)
}

fn get(store: &Store, id: &str) -> Result<Skill, DomainError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, name, description, instructions, enabled, builtin, created_at, updated_at
                 FROM agent_skills WHERE id=?1",
                params![id],
                map_row,
            )
        })
        .map_err(|error| match error {
            digilegacy_store::StoreError::Sql(rusqlite::Error::QueryReturnedNoRows) => {
                DomainError::NotFound(format!("Skill {id} 不存在"))
            }
            other => DomainError::from(other),
        })
}

pub fn upsert(store: &Store, input: Input) -> Result<Skill, DomainError> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(DomainError::Invalid(
            "Skill 名称不能为空且不能超过 80 字符".to_string(),
        ));
    }
    let description = input.description.trim();
    if description.chars().count() > 500 {
        return Err(DomainError::Invalid(
            "Skill 描述不能超过 500 字符".to_string(),
        ));
    }
    let instructions = input.instructions.trim();
    if instructions.is_empty() || instructions.chars().count() > 20_000 {
        return Err(DomainError::Invalid(
            "Skill 指令不能为空且不能超过 20,000 字符".to_string(),
        ));
    }
    let id_hint = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    store
        .write(|conn| {
            let now = Utc::now().to_rfc3339();
            if let Some(id) = id_hint {
                let changed = conn.execute(
                    "UPDATE agent_skills
                     SET name=?2, description=?3, instructions=?4, enabled=?5, updated_at=?6
                     WHERE id=?1",
                    params![
                        id,
                        name,
                        description,
                        instructions,
                        if input.enabled { 1 } else { 0 },
                        now
                    ],
                )?;
                if changed == 0 {
                    return Err(digilegacy_store::StoreError::State(
                        "SKILL_NOT_FOUND".to_string(),
                    ));
                }
            } else {
                conn.execute(
                    "INSERT INTO agent_skills
                     (name, description, instructions, enabled, created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?5)",
                    params![
                        name,
                        description,
                        instructions,
                        if input.enabled { 1 } else { 0 },
                        now
                    ],
                )?;
            }
            Ok(())
        })
        .map_err(|error| match error {
            digilegacy_store::StoreError::State(message) if message == "SKILL_NOT_FOUND" => {
                DomainError::NotFound("Skill 不存在".to_string())
            }
            digilegacy_store::StoreError::Sql(rusqlite::Error::SqliteFailure(
                _,
                Some(ref text),
            )) if text.contains("UNIQUE") => DomainError::Conflict("同名 Skill 已存在".to_string()),
            other => DomainError::from(other),
        })?;

    if let Some(id) = id_hint {
        get(store, id)
    } else {
        list(store)?
            .into_iter()
            .find(|skill| skill.name == name && skill.instructions == instructions)
            .ok_or_else(|| DomainError::NotFound("Skill 写入后未找到".to_string()))
    }
}

pub fn delete(store: &Store, id: &str) -> Result<(String, String), DomainError> {
    let skill = get(store, id)?;
    store
        .write(|conn| {
            conn.execute("DELETE FROM agent_skills WHERE id=?1", params![id])?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    Ok((skill.id, skill.name))
}

/// Concatenate enabled skills into a bounded planner prompt section.
pub fn enabled_prompt(store: &Store, max_chars: usize) -> Result<String, DomainError> {
    let mut output = String::new();
    for skill in list(store)?.into_iter().filter(|skill| skill.enabled) {
        let block = format!("\n### {}\n{}\n", skill.name, skill.instructions);
        if output.chars().count() + block.chars().count() > max_chars {
            break;
        }
        output.push_str(&block);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn store() -> Arc<Store> {
        Arc::new(Store::open_in_memory(None).unwrap())
    }

    #[test]
    fn stores_lists_and_deletes_skills() {
        let store = store();
        let saved = upsert(
            &store,
            Input {
                id: None,
                name: "Research".to_string(),
                description: "Find sources".to_string(),
                instructions: "Always cite sources.".to_string(),
                enabled: true,
            },
        )
        .unwrap();
        assert_eq!(saved.name, "Research");
        assert!(enabled_prompt(&store, 4096)
            .unwrap()
            .contains("Always cite sources."));
        assert_eq!(list(&store).unwrap().len(), 9, "8 built-ins plus Research");
        assert!(
            list(&store).unwrap()[0].builtin || list(&store).unwrap().iter().any(|s| s.builtin)
        );
        assert_eq!(delete(&store, &saved.id).unwrap().1, "Research");
        assert_eq!(list(&store).unwrap().len(), 8);
    }

    #[test]
    fn rejects_duplicate_names() {
        let store = store();
        for _ in 0..2 {
            let result = upsert(
                &store,
                Input {
                    id: None,
                    name: "Same".to_string(),
                    description: String::new(),
                    instructions: "Do it.".to_string(),
                    enabled: true,
                },
            );
            if let Err(error) = result {
                assert!(matches!(error, DomainError::Conflict(_)));
                return;
            }
        }
        panic!("duplicate skill name was accepted");
    }
}
