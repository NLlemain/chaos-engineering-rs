use anyhow::{bail, Result};
use chaos_core::{InjectorRegistry, InjectorStatus, RecoveryJournal};
use colored::Colorize;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct DoctorReport {
    injectors: Vec<InjectorCheck>,
    summary: DoctorSummary,
    recovery_journal: RecoveryJournalReport,
}

#[derive(Debug, Serialize)]
struct InjectorCheck {
    name: String,
    state: InjectorState,
    required_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum InjectorState {
    Ready,
    Blocked,
    Planned,
}

#[derive(Debug, Default, Serialize)]
struct DoctorSummary {
    ready: usize,
    blocked: usize,
    planned: usize,
}

#[derive(Debug, Serialize)]
struct RecoveryJournalReport {
    path: String,
    active_entries: usize,
}

pub async fn execute(json: bool) -> Result<()> {
    let registry = InjectorRegistry::with_defaults();
    let journal = RecoveryJournal::new(RecoveryJournal::default_path());
    let report = collect_report(&registry, &journal).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_table(&report);
    }

    ensure_operational(report.summary.blocked)
}

async fn collect_report(
    registry: &InjectorRegistry,
    journal: &RecoveryJournal,
) -> Result<DoctorReport> {
    let mut injectors = Vec::new();
    let mut summary = DoctorSummary::default();

    for info in registry.list_info() {
        let injector = registry
            .get(&info.name)
            .expect("registry info should reference an injector");
        let result = if info.status == InjectorStatus::Planned {
            None
        } else {
            Some(injector.validate().await)
        };

        let (state, reason) = match result {
            None => {
                summary.planned += 1;
                (InjectorState::Planned, None)
            }
            Some(Ok(())) => {
                summary.ready += 1;
                (InjectorState::Ready, None)
            }
            Some(Err(error)) => {
                summary.blocked += 1;
                (
                    InjectorState::Blocked,
                    Some(sanitize_reason(&error.to_string())),
                )
            }
        };

        injectors.push(InjectorCheck {
            name: info.name,
            state,
            required_capabilities: info.required_capabilities,
            reason,
        });
    }

    let active = journal.entries().await?;
    Ok(DoctorReport {
        injectors,
        summary,
        recovery_journal: RecoveryJournalReport {
            path: journal.path().to_string_lossy().into_owned(),
            active_entries: active.len(),
        },
    })
}

fn print_table(report: &DoctorReport) {
    println!("{}", "=== Chaos Doctor ===".bold().cyan());

    for injector in &report.injectors {
        match injector.state {
            InjectorState::Planned => {
                println!("  {:<28} {}", injector.name, "planned".dimmed());
            }
            InjectorState::Ready => {
                println!("  {:<28} {}", injector.name, "ready".green());
            }
            InjectorState::Blocked => {
                println!(
                    "  {:<28} {} ({})",
                    injector.name,
                    "blocked".red(),
                    injector.reason.as_deref().unwrap_or("validation failed")
                );
            }
        }
    }

    println!("\nRecovery journal: {}", report.recovery_journal.path);
    println!(
        "Recorded active injections: {}",
        report.recovery_journal.active_entries
    );

    if report.summary.blocked == 0 {
        println!("{}", "Doctor checks passed.".green().bold());
    }
}

fn sanitize_reason(reason: &str) -> String {
    const MAX_REASON_CHARS: usize = 240;
    reason
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_REASON_CHARS)
        .collect()
}

fn ensure_operational(blocked: usize) -> Result<()> {
    if blocked == 0 {
        Ok(())
    } else {
        bail!("{} operational injector(s) are blocked", blocked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chaos_core::{async_trait, ChaosError, InjectionHandle, Injector, Target};
    use std::sync::Arc;

    struct FixtureInjector {
        name: &'static str,
        status: InjectorStatus,
        failure: Option<&'static str>,
    }

    #[async_trait]
    impl Injector for FixtureInjector {
        async fn inject(&self, _target: &Target) -> chaos_core::Result<InjectionHandle> {
            Err(ChaosError::InjectionFailed("fixture only".into()))
        }

        async fn remove(&self, _handle: InjectionHandle) -> chaos_core::Result<()> {
            Ok(())
        }

        fn name(&self) -> &str {
            self.name
        }

        fn status(&self) -> InjectorStatus {
            self.status
        }

        async fn validate(&self) -> chaos_core::Result<()> {
            match self.failure {
                Some(reason) => Err(ChaosError::PermissionDenied(reason.into())),
                None => Ok(()),
            }
        }

        fn required_capabilities(&self) -> Vec<String> {
            vec!["fixture-capability".into()]
        }
    }

    #[tokio::test]
    async fn report_covers_ready_blocked_and_planned_fixtures() {
        let mut registry = InjectorRegistry::new();
        for (name, status, failure) in [
            ("ready", InjectorStatus::Stable, None),
            (
                "blocked",
                InjectorStatus::Experimental,
                Some("administrator\naccess\tis required"),
            ),
            ("planned", InjectorStatus::Planned, Some("not validated")),
        ] {
            registry.register(
                name,
                Arc::new(FixtureInjector {
                    name,
                    status,
                    failure,
                }),
            );
        }
        let path = std::env::temp_dir().join("chaos-doctor-fixture-missing.json");
        let report = collect_report(&registry, &RecoveryJournal::new(&path))
            .await
            .unwrap();

        assert_eq!(report.summary.ready, 1);
        assert_eq!(report.summary.blocked, 1);
        assert_eq!(report.summary.planned, 1);
        assert_eq!(report.recovery_journal.path, path.to_string_lossy());
        assert_eq!(report.recovery_journal.active_entries, 0);
        assert_eq!(report.injectors[0].state, InjectorState::Blocked);
        assert_eq!(
            report.injectors[0].reason.as_deref(),
            Some("Permission denied: administrator access is required")
        );
        assert_eq!(
            report.injectors[0].required_capabilities,
            ["fixture-capability"]
        );
        assert!(ensure_operational(report.summary.blocked).is_err());

        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["injectors"][0]["state"], "blocked");
        assert_eq!(
            json["injectors"][0]["required_capabilities"][0],
            "fixture-capability"
        );
        assert_eq!(json["injectors"][1]["state"], "planned");
        assert!(json["injectors"][1].get("reason").is_none());
        assert_eq!(json["injectors"][2]["state"], "ready");
        assert_eq!(json["recovery_journal"]["active_entries"], 0);
    }
}
