//! Curated protocol-specific scenarios for `chaos-engineering-rs`.

/// The searchable scenario-pack catalog used by the CLI and external tooling.
pub const CATALOG_JSON: &str = include_str!("../catalog.json");

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_is_valid_json() {
        let value: serde_json::Value = serde_json::from_str(super::CATALOG_JSON).unwrap();
        assert!(value["packs"].as_array().unwrap().len() >= 30);
    }
}
