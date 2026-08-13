use assert_cmd::Command;

#[test]
fn list_json_exposes_every_injector_and_schema_field() {
    let output = Command::cargo_bin("chaos")
        .unwrap()
        .args(["list", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let injectors: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(injectors.len(), 24);

    for injector in injectors {
        assert!(injector["name"]
            .as_str()
            .is_some_and(|name| !name.is_empty()));
        assert!(matches!(
            injector["status"].as_str(),
            Some("stable" | "experimental" | "planned")
        ));
        assert!(injector["required_capabilities"].is_array());
        assert_eq!(injector.as_object().unwrap().len(), 3);
    }
}

#[test]
fn list_table_remains_the_default() {
    Command::cargo_bin("chaos")
        .unwrap()
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("=== Available Injectors ==="))
        .stdout(predicates::str::contains("Total injectors: 24"));
}

#[test]
fn list_json_can_target_a_release_platform() {
    let output = Command::cargo_bin("chaos")
        .unwrap()
        .args(["list", "--json", "--platform", "linux"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let injectors: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let status = |name: &str| {
        injectors
            .iter()
            .find(|injector| injector["name"] == name)
            .and_then(|injector| injector["status"].as_str())
    };
    assert_eq!(status("packet_loss"), Some("experimental"));
    assert_eq!(status("process_freeze"), Some("stable"));
    assert_eq!(status("windows_fault"), Some("planned"));
    let process_kill = injectors
        .iter()
        .find(|injector| injector["name"] == "process_kill")
        .unwrap();
    assert_eq!(
        process_kill["required_capabilities"][0],
        "Permission to signal the target process"
    );
}
