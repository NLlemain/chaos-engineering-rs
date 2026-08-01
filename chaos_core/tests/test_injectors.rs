use chaos_core::{InjectorRegistry, Target};

#[tokio::test]
async fn test_registry_contains_all_20_chaos_types() {
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
    ];

    assert_eq!(
        injectors.len(),
        20,
        "Expected exactly 20 registered chaos types, got {}",
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
async fn test_all_20_injectors_lifecycle() {
    let registry = InjectorRegistry::with_defaults();
    let target = Target::System;

    let names = registry.list();
    for name in names {
        let injector = registry.get(&name).unwrap();
        assert_eq!(injector.name(), name);

        assert!(injector.validate().await.is_ok());

        if name != "process_kill" && name != "process_freeze" {
            if let Ok(handle) = injector.inject(&target).await {
                assert_eq!(handle.injector_name, name);
                let remove_res = injector.remove(handle).await;
                assert!(
                    remove_res.is_ok(),
                    "Removing injector '{}' failed: {:?}",
                    name,
                    remove_res.err()
                );
            }
        }
    }
}
