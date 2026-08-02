use chaos_core::{Executor, InjectorRegistry, InjectorStatus, Target};

#[tokio::test]
async fn test_registry_contains_all_24_chaos_types() {
    let registry = InjectorRegistry::with_defaults();
    let injectors = registry.list();

    let expected_names = vec![
        "network_latency",
        "packet_loss",
        "tcp_reset",
        "cpu_starvation",
        "disk_slow",
        "memory_pressure",
        "process_kill",
        "dns_fault",
        "fd_exhaustion",
        "process_freeze",
        "clock_skew",
        "disk_fill",
        "socket_corrupt",
        "http_fault",
        "nginx_fault",
        "aws_fault",
        "crypto_fault",
        "azure_fault",
        "cloudflare_fault",
        "media_streaming_fault",
        "dependency_proxy",
        "container_fault",
        "database_fault",
        "windows_fault",
    ];

    assert_eq!(
        injectors.len(),
        24,
        "Expected exactly 24 registered chaos types, got {}",
        injectors.len()
    );

    for name in expected_names {
        assert!(
            registry.get(name).is_some(),
            "Injector '{}' should be registered in InjectorRegistry",
            name
        );
    }
}

#[tokio::test]
async fn test_registered_injectors_report_honest_status() {
    let registry = InjectorRegistry::with_defaults();
    let info = registry.list_info();

    assert!(info
        .iter()
        .any(|injector| injector.status == InjectorStatus::Stable));
    assert!(info
        .iter()
        .any(|injector| injector.status == InjectorStatus::Experimental));
    assert!(info
        .iter()
        .any(|injector| injector.status == InjectorStatus::Planned));
}

#[tokio::test]
async fn test_planned_injector_cannot_report_success() {
    let executor = Executor::with_defaults();
    let error = executor
        .inject("aws_fault", &Target::System)
        .await
        .expect_err("planned injector should be rejected");

    assert!(error.to_string().contains("planned but not implemented"));
}
