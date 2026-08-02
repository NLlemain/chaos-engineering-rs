#[test]
fn every_catalog_source_parses_with_the_production_parser() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();

    chaos_packs::validate_catalog(
        chaos_packs::CATALOG_JSON,
        repository_root,
        |contents, format| chaos_scenarios::parse_scenario_from_str(contents, format).map(|_| ()),
    )
    .unwrap();
}
