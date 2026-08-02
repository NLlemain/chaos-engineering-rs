//! Curated protocol-specific scenarios for `chaos-engineering-rs`.

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::{collections::HashSet, path::Path};

/// The searchable scenario-pack catalog used by the CLI and external tooling.
pub const CATALOG_JSON: &str = include_str!("../catalog.json");

const KNOWN_CATEGORIES: &[&str] = &[
    "ai",
    "authentication",
    "containers",
    "databases",
    "iot",
    "media",
    "network",
    "object-storage",
    "queues",
    "windows",
];
const KNOWN_STATUSES: &[&str] = &["stable", "experimental", "planned"];

/// Validate catalog metadata and parse every referenced scenario from a repository checkout.
pub fn validate_catalog<F>(
    catalog_json: &str,
    repository_root: impl AsRef<Path>,
    mut parse_scenario: F,
) -> Result<()>
where
    F: FnMut(&str, &str) -> Result<()>,
{
    let root: Value = serde_json::from_str(catalog_json).context("catalog field '<document>'")?;
    if root.get("schema_version").and_then(Value::as_u64) != Some(1) {
        bail!("catalog field 'schema_version': expected integer 1");
    }
    let packs = root
        .get("packs")
        .and_then(Value::as_array)
        .context("catalog field 'packs': expected an array")?;
    let repository_root = repository_root.as_ref();
    let mut ids = HashSet::new();

    for (index, value) in packs.iter().enumerate() {
        let pack = value
            .as_object()
            .with_context(|| format!("pack #{} field '<entry>': expected an object", index + 1))?;
        let fallback = format!("#{}", index + 1);
        let pack_name = pack
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&fallback);
        let id = required_string(pack, pack_name, "id")?;
        if !ids.insert(id) {
            bail!("pack '{}' field 'id': duplicate catalog ID", pack_name);
        }

        required_string(pack, pack_name, "title")?;
        required_string(pack, pack_name, "description")?;
        required_string_list(pack, pack_name, "protocols")?;
        required_string_list(pack, pack_name, "requirements")?;

        let category = required_string(pack, pack_name, "category")?;
        if !KNOWN_CATEGORIES.contains(&category) {
            bail!(
                "pack '{}' field 'category': unknown category '{}'",
                pack_name,
                category
            );
        }

        let status = required_string(pack, pack_name, "status")?;
        if !KNOWN_STATUSES.contains(&status) {
            bail!(
                "pack '{}' field 'status': expected stable, experimental, or planned; got '{}'",
                pack_name,
                status
            );
        }

        let source = required_string(pack, pack_name, "file")?;
        let relative_path = Path::new(source);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            bail!(
                "pack '{}' field 'file': expected a repository-relative path; got '{}'",
                pack_name,
                source
            );
        }
        let source_path = repository_root.join(relative_path);
        let contents = std::fs::read_to_string(&source_path).with_context(|| {
            format!(
                "pack '{}' field 'file': cannot read '{}'",
                pack_name,
                source_path.display()
            )
        })?;
        let format = relative_path
            .extension()
            .and_then(|extension| extension.to_str())
            .with_context(|| {
                format!(
                    "pack '{}' field 'file': '{}' has no supported extension",
                    pack_name, source
                )
            })?;
        parse_scenario(&contents, format).with_context(|| {
            format!(
                "pack '{}' field 'file': '{}' is not a valid scenario",
                pack_name, source
            )
        })?;

        let download_url = required_string(pack, pack_name, "download_url")?;
        if !download_url.starts_with("https://") {
            bail!(
                "pack '{}' field 'download_url': expected an HTTPS URL",
                pack_name
            );
        }
    }

    Ok(())
}

fn required_string<'a>(
    pack: &'a Map<String, Value>,
    pack_name: &str,
    field: &str,
) -> Result<&'a str> {
    pack.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| {
            format!(
                "pack '{}' field '{}': expected a non-empty string",
                pack_name, field
            )
        })
}

fn required_string_list(pack: &Map<String, Value>, pack_name: &str, field: &str) -> Result<()> {
    let values = pack.get(field).and_then(Value::as_array).with_context(|| {
        format!(
            "pack '{}' field '{}': expected a non-empty string array",
            pack_name, field
        )
    })?;
    if values.is_empty()
        || values
            .iter()
            .any(|value| value.as_str().is_none_or(|value| value.trim().is_empty()))
    {
        bail!(
            "pack '{}' field '{}': expected a non-empty string array",
            pack_name,
            field
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_entry_and_source_is_valid() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut parsed_sources = 0usize;
        validate_catalog(CATALOG_JSON, repository_root, |contents, format| {
            assert!(!contents.trim().is_empty());
            assert!(matches!(format, "yaml" | "yml" | "toml" | "json"));
            parsed_sources += 1;
            Ok(())
        })
        .unwrap();

        let value: Value = serde_json::from_str(CATALOG_JSON).unwrap();
        let pack_count = value["packs"].as_array().unwrap().len();
        assert!(pack_count >= 30);
        assert_eq!(parsed_sources, pack_count);
    }

    #[test]
    fn invalid_status_names_the_pack_and_field() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let catalog = serde_json::json!({
            "schema_version": 1,
            "packs": [{
                "id": "bad-status-pack",
                "title": "Bad status fixture",
                "category": "ai",
                "status": "preview",
                "description": "A focused invalid catalog fixture.",
                "protocols": ["HTTP"],
                "requirements": ["None"],
                "file": "scenario-packs/ai/openai-compatible.yaml",
                "download_url": "https://example.com/scenario.yaml"
            }]
        });

        let error =
            validate_catalog(&catalog.to_string(), repository_root, |_, _| Ok(())).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("pack 'bad-status-pack' field 'status'"),
            "{error:#}"
        );
    }
}
