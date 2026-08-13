use anyhow::{bail, Context, Result};
use chaos_scenarios::{injector_factory::build_injector, parser::parse_scenario_from_str};
use clap::{Args, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

const CATALOG_JSON: &str = chaos_packs::CATALOG_JSON;

#[derive(Debug, Args)]
pub struct PackArgs {
    #[command(subcommand)]
    command: PackCommand,
}

#[derive(Debug, Subcommand)]
enum PackCommand {
    /// List and search available packs
    List {
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,
        /// Search IDs, descriptions, and protocols
        #[arg(short, long)]
        search: Option<String>,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Show one pack's capabilities and requirements
    Show {
        /// Catalog pack ID
        id: String,
    },
    /// Download and validate a pack
    Install {
        /// Catalog pack ID
        id: String,
        /// Destination directory or YAML filename
        #[arg(short, long, default_value = "scenarios")]
        output: PathBuf,
        /// Replace an existing destination file
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PackCatalog {
    schema_version: u32,
    packs: Vec<PackEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PackEntry {
    id: String,
    title: String,
    category: String,
    status: String,
    description: String,
    protocols: Vec<String>,
    requirements: Vec<String>,
    #[serde(default = "default_pack_kind")]
    kind: String,
    file: Option<String>,
    download_url: Option<String>,
}

fn default_pack_kind() -> String {
    "scenario".to_string()
}

pub async fn execute(args: PackArgs) -> Result<()> {
    let catalog = catalog()?;
    match args.command {
        PackCommand::List {
            category,
            search,
            json,
        } => list(&catalog, category.as_deref(), search.as_deref(), json),
        PackCommand::Show { id } => show(&catalog, &id),
        PackCommand::Install { id, output, force } => install(&catalog, &id, output, force).await,
    }
}

fn catalog() -> Result<PackCatalog> {
    chaos_packs::validate_cli_compatibility(CATALOG_JSON, env!("CARGO_PKG_VERSION"))?;
    let catalog: PackCatalog = serde_json::from_str(CATALOG_JSON)?;
    let mut ids = HashSet::new();
    for entry in &catalog.packs {
        if !ids.insert(&entry.id) {
            bail!("Scenario catalog contains duplicate ID '{}'", entry.id);
        }
    }
    Ok(catalog)
}

fn list(
    catalog: &PackCatalog,
    category: Option<&str>,
    search: Option<&str>,
    json: bool,
) -> Result<()> {
    let category = category.map(str::to_ascii_lowercase);
    let search = search.map(str::to_ascii_lowercase);
    let matches: Vec<_> = catalog
        .packs
        .iter()
        .filter(|entry| {
            category
                .as_ref()
                .is_none_or(|value| entry.category.eq_ignore_ascii_case(value))
        })
        .filter(|entry| {
            search.as_ref().is_none_or(|value| {
                format!(
                    "{} {} {} {}",
                    entry.id,
                    entry.title,
                    entry.description,
                    entry.protocols.join(" ")
                )
                .to_ascii_lowercase()
                .contains(value)
            })
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&matches)?);
        return Ok(());
    }

    println!("{}", "Scenario packs".bold().cyan());
    println!("{:<34} {:<16} {:<13} TITLE", "ID", "CATEGORY", "STATUS");
    for entry in matches {
        println!(
            "{:<34} {:<16} {:<13} {}",
            entry.id, entry.category, entry.status, entry.title
        );
    }
    Ok(())
}

fn show(catalog: &PackCatalog, id: &str) -> Result<()> {
    let entry = find(catalog, id)?;
    println!("{}", entry.title.bold().cyan());
    println!("ID:           {}", entry.id);
    println!("Category:     {}", entry.category);
    println!("Status:       {}", entry.status);
    println!("Kind:         {}", entry.kind);
    println!("Protocols:    {}", entry.protocols.join(", "));
    println!("Requirements: {}", entry.requirements.join(", "));
    println!("\n{}", entry.description);
    if entry.download_url.is_none() {
        println!("\nThis capability is planned and has no downloadable scenario yet.");
    }
    Ok(())
}

async fn install(catalog: &PackCatalog, id: &str, output: PathBuf, force: bool) -> Result<()> {
    let entry = find(catalog, id)?;
    let url = entry.download_url.as_deref().with_context(|| {
        format!(
            "Pack '{}' is planned and does not have a downloadable scenario",
            id
        )
    })?;
    let filename = entry
        .file
        .as_deref()
        .and_then(|path| std::path::Path::new(path).file_name())
        .context("Catalog entry is missing a valid file name")?;
    let destination = if output.extension().is_none() || output.is_dir() {
        output.join(filename)
    } else {
        output
    };
    if destination.exists() && !force {
        bail!(
            "{} already exists; pass --force to replace it",
            destination.display()
        );
    }

    let contents = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to download {}", url))?
        .error_for_status()
        .with_context(|| format!("Pack download failed for {}", url))?
        .text()
        .await?;
    match entry.kind.as_str() {
        "scenario" => {
            let scenario = parse_scenario_from_str(&contents, "yaml")
                .with_context(|| format!("Downloaded pack '{}' is not a valid scenario", id))?;
            for phase in &scenario.phases {
                for injection in &phase.injections {
                    build_injector(injection).with_context(|| {
                        format!(
                            "Downloaded pack '{}' has invalid '{}' parameters",
                            id, injection.r#type
                        )
                    })?;
                }
            }
        }
        "pipeline_fault_plan" => {
            let plan: chaos_pipeline::PipelineFaultPlan = serde_yaml::from_str(&contents)
                .with_context(|| {
                    format!("Downloaded pack '{}' is not a pipeline fault plan", id)
                })?;
            plan.validate()
                .with_context(|| format!("Downloaded pack '{}' has invalid faults", id))?;
        }
        value => bail!("Pack '{}' has unsupported kind '{}'", id, value),
    }

    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&destination, contents).await?;
    println!("Installed {} to {}", id.bold(), destination.display());
    Ok(())
}

fn find<'a>(catalog: &'a PackCatalog, id: &str) -> Result<&'a PackEntry> {
    catalog
        .packs
        .iter()
        .find(|entry| entry.id.eq_ignore_ascii_case(id))
        .with_context(|| format!("Unknown scenario pack '{}'; run `chaos pack list`", id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_unique_and_searchable() {
        let catalog = catalog().unwrap();
        assert!(catalog.packs.len() >= 25);
        assert!(find(&catalog, "hls-stale-manifest").is_ok());
        assert!(find(&catalog, "not-a-pack").is_err());
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for entry in &catalog.packs {
            if let Some(file) = &entry.file {
                assert!(
                    repository.join(file).is_file(),
                    "catalog file does not exist: {}",
                    file
                );
            }
        }
    }
}
