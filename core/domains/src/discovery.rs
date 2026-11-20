//! DOM-01 discovery: curated, explainable asset suggestions.
//!
//! This is a checklist, not a network scanner. Suggestions are static product
//! knowledge; `exists` is derived from the user's current asset registry so the
//! wizard never encourages duplicates.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::assets::DomainError;

const CATALOG: &str = include_str!("../../../content/assets/catalog.json");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub label: String,
    pub label_en: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub platform: String,
    pub category: String,
    pub reason: String,
    pub reason_en: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverResult {
    pub categories: Vec<Category>,
    pub suggestions: Vec<Suggestion>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverInput {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    categories: Vec<CatalogCategory>,
    suggestions: Vec<CatalogSuggestion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogCategory {
    id: String,
    label: String,
    label_en: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSuggestion {
    id: String,
    name: String,
    kind: String,
    platform: String,
    category: String,
    reason: String,
    reason_en: String,
}

fn catalog() -> Result<Catalog, DomainError> {
    serde_json::from_str(CATALOG)
        .map_err(|error| DomainError::Invalid(format!("资产目录损坏: {error}")))
}

pub fn discover(store: &Store, input: &DiscoverInput) -> Result<DiscoverResult, DomainError> {
    let catalog = catalog()?;
    let category = input.category.as_deref().unwrap_or("").trim();
    let q = input.q.as_deref().unwrap_or("").trim().to_lowercase();

    let existing = store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT coalesce(a.account_description,''), coalesce(p.name,'')
                 FROM digital_assets a LEFT JOIN platforms p ON p.id=a.platform_id",
            )?;
            let pairs = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<HashSet<_>>>()?;
            Ok(pairs)
        })
        .map_err(DomainError::from)?;

    let categories: Vec<Category> = catalog
        .categories
        .iter()
        .map(|category| Category {
            id: category.id.clone(),
            label: category.label.clone(),
            label_en: category.label_en.clone(),
            count: catalog
                .suggestions
                .iter()
                .filter(|suggestion| suggestion.category == category.id)
                .count() as i64,
        })
        .collect();

    let suggestions: Vec<Suggestion> = catalog
        .suggestions
        .into_iter()
        .filter(|suggestion| {
            (category.is_empty() || category == "all" || suggestion.category == category)
                && (q.is_empty()
                    || suggestion.name.to_lowercase().contains(&q)
                    || suggestion.platform.to_lowercase().contains(&q)
                    || suggestion.reason.to_lowercase().contains(&q)
                    || suggestion.reason_en.to_lowercase().contains(&q))
        })
        .map(|suggestion| Suggestion {
            exists: existing.contains(&(suggestion.name.clone(), suggestion.platform.clone())),
            id: suggestion.id,
            name: suggestion.name,
            kind: suggestion.kind,
            platform: suggestion.platform,
            category: suggestion.category,
            reason: suggestion.reason,
            reason_en: suggestion.reason_en,
        })
        .collect();

    let total = suggestions.len() as i64;
    Ok(DiscoverResult {
        categories,
        suggestions,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{create, Mutation};

    #[test]
    fn catalog_is_explainable_and_filterable() {
        let store = Store::open_in_memory(None).unwrap();
        let all = discover(
            &store,
            &DiscoverInput {
                category: None,
                q: None,
            },
        )
        .unwrap();
        assert_eq!(all.categories.len(), 8);
        assert_eq!(all.total, 36);

        let finance = discover(
            &store,
            &DiscoverInput {
                category: Some("finance".to_string()),
                q: None,
            },
        )
        .unwrap();
        assert_eq!(finance.total, 6);
        assert!(finance
            .suggestions
            .iter()
            .all(|suggestion| suggestion.category == "finance"));
    }

    #[test]
    fn existing_assets_are_marked_and_never_recommended_twice() {
        let store = Store::open_in_memory(None).unwrap();
        create(
            &store,
            &Mutation {
                id: None,
                name: "微信零钱".to_string(),
                kind: "account".to_string(),
                platform: Some("微信".to_string()),
                value: None,
                note: None,
                status: None,
            },
        )
        .unwrap();
        let result = discover(
            &store,
            &DiscoverInput {
                category: Some("finance".to_string()),
                q: Some("微信".to_string()),
            },
        )
        .unwrap();
        assert_eq!(result.total, 1);
        assert!(result.suggestions[0].exists);
    }
}
