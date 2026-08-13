//! Curated protocol-specific scenarios for `chaos-engineering-rs`.

use anyhow::{bail, Context, Result};
use semver::{Version, VersionReq};
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
    "trading",
    "windows",
];
const KNOWN_STATUSES: &[&str] = &["stable", "experimental", "planned"];

/// Validate that a CLI version is allowed to consume this pack index.
pub fn validate_cli_compatibility(catalog_json: &str, cli_version: &str) -> Result<()> {
    let root: Value = serde_json::from_str(catalog_json).context("catalog field '<document>'")?;
    let requirement = root
        .pointer("/compatibility/cli")
        .and_then(Value::as_str)
        .context("catalog field 'compatibility.cli': expected a semantic-version requirement")?;
    let requirement = VersionReq::parse(requirement)
        .context("catalog field 'compatibility.cli': invalid semantic-version requirement")?;
    let version =
        Version::parse(cli_version).context("CLI version is not valid semantic version")?;
    if requirement.matches(&version) {
        Ok(())
    } else {
        bail!(
            "scenario-pack index requires chaos CLI '{}', but this is version {}",
            requirement,
            version
        )
    }
}

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
    if root.get("schema_version").and_then(Value::as_u64) != Some(2) {
        bail!("catalog field 'schema_version': expected integer 2");
    }
    let index_version = root
        .get("index_version")
        .and_then(Value::as_str)
        .context("catalog field 'index_version': expected semantic version")?;
    Version::parse(index_version)
        .context("catalog field 'index_version': invalid semantic version")?;
    validate_cli_compatibility(catalog_json, env!("CARGO_PKG_VERSION"))?;
    if root
        .pointer("/compatibility/scenario_schema")
        .and_then(Value::as_u64)
        != Some(1)
    {
        bail!("catalog field 'compatibility.scenario_schema': expected integer 1");
    }
    let packs = root
        .get("packs")
        .and_then(Value::as_array)
        .context("catalog field 'packs': expected an array")?;
    let repository_root = repository_root.as_ref();
    let mut ids = HashSet::new();
    let mut stable_ids = HashSet::new();

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
        if status == "stable" {
            stable_ids.insert(id);
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

    validate_evidence_suites(&root, repository_root, &ids, &stable_ids)
}

fn validate_evidence_suites<'a>(
    root: &'a Value,
    repository_root: &Path,
    pack_ids: &HashSet<&'a str>,
    stable_ids: &HashSet<&'a str>,
) -> Result<()> {
    let suites = root
        .get("evidence_suites")
        .and_then(Value::as_array)
        .context("catalog field 'evidence_suites': expected an array")?;
    let mut suite_ids = HashSet::new();
    let mut evidenced_stable = HashSet::new();

    for (index, value) in suites.iter().enumerate() {
        let suite = value.as_object().with_context(|| {
            format!(
                "evidence suite #{} field '<entry>': expected an object",
                index + 1
            )
        })?;
        let fallback = format!("#{}", index + 1);
        let suite_name = suite
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&fallback);
        let id = required_string(suite, suite_name, "id")?;
        if !suite_ids.insert(id) {
            bail!("evidence suite '{}' field 'id': duplicate suite ID", id);
        }

        let workflow = required_string(suite, suite_name, "workflow")?;
        let workflow_path = Path::new(workflow);
        if workflow_path.is_absolute()
            || workflow_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
            || !workflow.starts_with(".github/workflows/")
            || !workflow.ends_with(".yml")
            || !repository_root.join(workflow_path).is_file()
        {
            bail!(
                "evidence suite '{}' field 'workflow': '{}' is not a checked-in workflow",
                id,
                workflow
            );
        }
        required_string(suite, suite_name, "command")?;

        let assertions = required_string_values(suite, suite_name, "assertions")?;
        for required in ["disruption", "restoration"] {
            if !assertions.contains(required) {
                bail!(
                    "evidence suite '{}' field 'assertions': missing '{}' assertion",
                    id,
                    required
                );
            }
        }

        for pack_id in required_string_values(suite, suite_name, "packs")? {
            if !pack_ids.contains(pack_id) {
                bail!(
                    "evidence suite '{}' field 'packs': unknown pack '{}'",
                    id,
                    pack_id
                );
            }
            if stable_ids.contains(pack_id) && !evidenced_stable.insert(pack_id) {
                bail!(
                    "stable pack '{}' is covered by more than one evidence suite",
                    pack_id
                );
            }
        }
    }

    let mut missing: Vec<_> = stable_ids.difference(&evidenced_stable).copied().collect();
    missing.sort_unstable();
    if !missing.is_empty() {
        bail!(
            "stable packs require CI disruption and restoration evidence: {}",
            missing.join(", ")
        );
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
    required_string_values(pack, pack_name, field).map(|_| ())
}

fn required_string_values<'a>(
    pack: &'a Map<String, Value>,
    pack_name: &str,
    field: &str,
) -> Result<HashSet<&'a str>> {
    let values = pack.get(field).and_then(Value::as_array).with_context(|| {
        format!(
            "pack '{}' field '{}': expected a non-empty string array",
            pack_name, field
        )
    })?;
    let values: Option<HashSet<_>> = values
        .iter()
        .map(|value| value.as_str().filter(|value| !value.trim().is_empty()))
        .collect();
    let values = values.filter(|values| !values.is_empty());
    if values.is_none() {
        bail!(
            "pack '{}' field '{}': expected a non-empty string array",
            pack_name,
            field
        );
    }
    Ok(values.expect("checked above"))
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
            "schema_version": 2,
            "index_version": "0.3.0",
            "compatibility": {
                "cli": ">=0.2.1, <0.4.0",
                "scenario_schema": 1
            },
            "evidence_suites": [],
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

    #[test]
    fn compatibility_rejects_unsupported_cli_versions() {
        assert!(validate_cli_compatibility(CATALOG_JSON, "0.2.1").is_ok());
        let error = validate_cli_compatibility(CATALOG_JSON, "0.4.0").unwrap_err();
        assert!(error
            .to_string()
            .contains("requires chaos CLI '>=0.2.1, <0.4.0'"));
    }

    #[test]
    fn stable_pack_requires_disruption_and_restoration_evidence() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let catalog = serde_json::json!({
            "schema_version": 2,
            "index_version": "0.3.0",
            "compatibility": {
                "cli": ">=0.2.1, <0.4.0",
                "scenario_schema": 1
            },
            "evidence_suites": [],
            "packs": [{
                "id": "stable-without-evidence",
                "title": "Stable fixture",
                "category": "ai",
                "status": "stable",
                "description": "A stable pack must carry CI evidence.",
                "protocols": ["HTTP"],
                "requirements": ["None"],
                "file": "scenario-packs/ai/openai-compatible.yaml",
                "download_url": "https://example.com/scenario.yaml"
            }]
        });

        let error =
            validate_catalog(&catalog.to_string(), repository_root, |_, _| Ok(())).unwrap_err();
        assert!(error
            .to_string()
            .contains("stable packs require CI disruption and restoration evidence"));
    }
}
