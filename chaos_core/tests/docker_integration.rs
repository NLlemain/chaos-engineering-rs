use chaos_core::{
    ContainerFaultAction, ContainerFaultConfig, ContainerFaultInjector, Injector, Target,
};
use std::process::Command;

#[tokio::test]
async fn live_container_is_paused_and_restored() {
    if std::env::var("CHAOS_DOCKER_INTEGRATION").as_deref() != Ok("1") {
        return;
    }

    let name = format!("chaos-integration-{}", std::process::id());
    let status = Command::new("docker")
        .args(["run", "-d", "--name", &name, "alpine:3.20", "sleep", "300"])
        .status()
        .expect("Docker must be available for the live integration test");
    assert!(status.success());

    let injector = ContainerFaultInjector::new(ContainerFaultConfig {
        action: ContainerFaultAction::Pause,
        stop_timeout_seconds: 1,
    });
    let target = Target::container(&name);
    let handle = injector.inject(&target).await.unwrap();
    assert_eq!(inspect_paused(&name), "true");
    injector.remove(handle).await.unwrap();
    assert_eq!(inspect_paused(&name), "false");

    let _ = Command::new("docker").args(["rm", "-f", &name]).status();
}

fn inspect_paused(name: &str) -> String {
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Paused}}", name])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
