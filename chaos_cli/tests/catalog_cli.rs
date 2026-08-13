#[test]
fn every_catalog_source_parses_with_the_production_parser() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();

    chaos_packs::validate_catalog(
        chaos_packs::CATALOG_JSON,
        repository_root,
        |contents, format, kind| match kind {
            "scenario" => chaos_scenarios::parse_scenario_from_str(contents, format).map(|_| ()),
            "pipeline_fault_plan" => {
                let plan: chaos_pipeline::PipelineFaultPlan = match format {
                    "yaml" | "yml" => serde_yaml::from_str(contents)?,
                    "json" => serde_json::from_str(contents)?,
                    value => anyhow::bail!("unsupported pipeline plan format '{value}'"),
                };
                plan.validate()
            }
            value => anyhow::bail!("unsupported catalog kind '{value}'"),
        },
    )
    .unwrap();
}
