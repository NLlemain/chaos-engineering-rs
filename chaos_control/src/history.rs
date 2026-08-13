use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub manifest_digest: String,
    pub policy_digest: String,
    pub target_count: usize,
    pub artifact_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub experiment_id: String,
    pub name: String,
    pub media_type: String,
    pub sha256: String,
    pub bytes: u64,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionPolicy {
    pub max_runs: usize,
    pub max_age_days: u32,
    pub max_artifact_bytes: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_runs: 500,
            max_age_days: 30,
            max_artifact_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

pub struct ExperimentHistory {
    artifact_root: PathBuf,
    connection: Mutex<Connection>,
}

impl ExperimentHistory {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let artifact_root = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("artifacts");
        std::fs::create_dir_all(&artifact_root)?;
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS experiments (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 seed TEXT NOT NULL,
                 status TEXT NOT NULL,
                 started_at TEXT NOT NULL,
                 finished_at TEXT,
                 manifest_digest TEXT NOT NULL,
                 policy_digest TEXT NOT NULL,
                 target_count INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS artifacts (
                 experiment_id TEXT NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
                 name TEXT NOT NULL,
                 media_type TEXT NOT NULL,
                 sha256 TEXT NOT NULL,
                 bytes INTEGER NOT NULL,
                 path TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 PRIMARY KEY (experiment_id, name)
             );
             CREATE INDEX IF NOT EXISTS experiments_started_at
                 ON experiments(started_at DESC);
             CREATE INDEX IF NOT EXISTS artifacts_created_at
                 ON artifacts(created_at ASC);",
        )?;
        Ok(Self {
            artifact_root,
            connection: Mutex::new(connection),
        })
    }

    pub fn begin<T: Serialize, P: Serialize>(
        &self,
        id: &str,
        name: &str,
        seed: u64,
        target_count: usize,
        manifest: &T,
        policy: &P,
    ) -> anyhow::Result<ExperimentRecord> {
        anyhow::ensure!(!id.trim().is_empty(), "experiment ID cannot be empty");
        anyhow::ensure!(!name.trim().is_empty(), "experiment name cannot be empty");
        let started_at = Utc::now();
        let manifest_digest = digest_json(manifest)?;
        let policy_digest = digest_json(policy)?;
        self.connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?
            .execute(
                "INSERT INTO experiments
                 (id, name, seed, status, started_at, manifest_digest, policy_digest, target_count)
                 VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?7)",
                params![
                    id,
                    name,
                    seed.to_string(),
                    started_at.to_rfc3339(),
                    manifest_digest,
                    policy_digest,
                    target_count
                ],
            )?;
        self.get(id)?
            .ok_or_else(|| anyhow::anyhow!("new experiment was not persisted"))
    }

    pub fn finish(&self, id: &str, status: &str) -> anyhow::Result<ExperimentRecord> {
        anyhow::ensure!(
            matches!(status, "succeeded" | "failed" | "cancelled" | "recovered"),
            "unsupported experiment status '{status}'"
        );
        let changed = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?
            .execute(
                "UPDATE experiments SET status = ?2, finished_at = ?3 WHERE id = ?1",
                params![id, status, Utc::now().to_rfc3339()],
            )?;
        anyhow::ensure!(changed == 1, "unknown experiment '{id}'");
        self.get(id)?
            .ok_or_else(|| anyhow::anyhow!("finished experiment was not persisted"))
    }

    pub fn attach_artifact(
        &self,
        experiment_id: &str,
        name: &str,
        media_type: &str,
        contents: &[u8],
    ) -> anyhow::Result<ArtifactRecord> {
        anyhow::ensure!(
            self.get(experiment_id)?.is_some(),
            "unknown experiment '{experiment_id}'"
        );
        let safe_name = safe_component(name)?;
        let directory = self.artifact_root.join(safe_component(experiment_id)?);
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(&safe_name);
        let temporary = directory.join(format!(".{safe_name}.next"));
        std::fs::write(&temporary, contents)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(&temporary, &path)?;
        let created_at = Utc::now();
        let sha256 = digest(contents);
        self.connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?
            .execute(
                "INSERT INTO artifacts
                 (experiment_id, name, media_type, sha256, bytes, path, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(experiment_id, name) DO UPDATE SET
                    media_type = excluded.media_type,
                    sha256 = excluded.sha256,
                    bytes = excluded.bytes,
                    path = excluded.path,
                    created_at = excluded.created_at",
                params![
                    experiment_id,
                    name,
                    media_type,
                    sha256,
                    contents.len() as u64,
                    path.to_string_lossy(),
                    created_at.to_rfc3339()
                ],
            )?;
        Ok(ArtifactRecord {
            experiment_id: experiment_id.to_string(),
            name: name.to_string(),
            media_type: media_type.to_string(),
            sha256,
            bytes: contents.len() as u64,
            path,
            created_at,
        })
    }

    pub fn list(&self, limit: usize) -> anyhow::Result<Vec<ExperimentRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?;
        let mut statement = connection.prepare(
            "SELECT e.id, e.name, e.seed, e.status, e.started_at, e.finished_at,
                    e.manifest_digest, e.policy_digest, e.target_count, COUNT(a.name)
             FROM experiments e
             LEFT JOIN artifacts a ON a.experiment_id = e.id
             GROUP BY e.id
             ORDER BY e.started_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as u64], row_to_record)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Option<ExperimentRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?;
        connection
            .query_row(
                "SELECT e.id, e.name, e.seed, e.status, e.started_at, e.finished_at,
                        e.manifest_digest, e.policy_digest, e.target_count, COUNT(a.name)
                 FROM experiments e
                 LEFT JOIN artifacts a ON a.experiment_id = e.id
                 WHERE e.id = ?1
                 GROUP BY e.id",
                [id],
                row_to_record,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn artifacts(&self, experiment_id: &str) -> anyhow::Result<Vec<ArtifactRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?;
        let mut statement = connection.prepare(
            "SELECT experiment_id, name, media_type, sha256, bytes, path, created_at
             FROM artifacts WHERE experiment_id = ?1 ORDER BY created_at",
        )?;
        let rows = statement.query_map([experiment_id], |row| {
            Ok(ArtifactRecord {
                experiment_id: row.get(0)?,
                name: row.get(1)?,
                media_type: row.get(2)?,
                sha256: row.get(3)?,
                bytes: row.get(4)?,
                path: PathBuf::from(row.get::<_, String>(5)?),
                created_at: parse_time(row.get::<_, String>(6)?, 6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn prune(&self, policy: RetentionPolicy) -> anyhow::Result<usize> {
        anyhow::ensure!(
            policy.max_runs > 0,
            "retention max_runs must be greater than zero"
        );
        let cutoff = Utc::now() - Duration::days(i64::from(policy.max_age_days));
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("history database lock poisoned"))?;
        let mut statement = connection.prepare(
            "SELECT id FROM experiments
             WHERE started_at < ?1
                OR id NOT IN (SELECT id FROM experiments ORDER BY started_at DESC LIMIT ?2)",
        )?;
        let ids = statement
            .query_map(
                params![cutoff.to_rfc3339(), policy.max_runs as u64],
                |row| row.get::<_, String>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for id in &ids {
            remove_directory_if_exists(&self.artifact_root.join(id))?;
            connection.execute("DELETE FROM experiments WHERE id = ?1", [id])?;
        }

        let total: u64 =
            connection.query_row("SELECT COALESCE(SUM(bytes), 0) FROM artifacts", [], |row| {
                row.get(0)
            })?;
        let mut removed = ids.len();
        let mut remaining = total;
        if remaining > policy.max_artifact_bytes {
            let mut artifacts = connection.prepare(
                "SELECT experiment_id, name, bytes, path FROM artifacts ORDER BY created_at ASC",
            )?;
            let rows = artifacts
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u64>(2)?,
                        PathBuf::from(row.get::<_, String>(3)?),
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(artifacts);
            for (experiment_id, name, bytes, path) in rows {
                if remaining <= policy.max_artifact_bytes {
                    break;
                }
                remove_file_if_exists(&path)?;
                connection.execute(
                    "DELETE FROM artifacts WHERE experiment_id = ?1 AND name = ?2",
                    params![experiment_id, name],
                )?;
                remaining = remaining.saturating_sub(bytes);
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExperimentRecord> {
    let seed = row.get::<_, String>(2)?.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(ExperimentRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        seed,
        status: row.get(3)?,
        started_at: parse_time(row.get::<_, String>(4)?, 4)?,
        finished_at: row
            .get::<_, Option<String>>(5)?
            .map(|value| parse_time(value, 5))
            .transpose()?,
        manifest_digest: row.get(6)?,
        policy_digest: row.get(7)?,
        target_count: row.get(8)?,
        artifact_count: row.get(9)?,
    })
}

fn parse_time(value: String, column: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn digest_json<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(digest(&serde_json::to_vec(value)?))
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn safe_component(value: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        !value.trim().is_empty(),
        "artifact path component cannot be empty"
    );
    anyhow::ensure!(
        value
            .chars()
            .all(|character| character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.')),
        "artifact path component '{value}' contains unsafe characters"
    );
    Ok(value.to_string())
}

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_directory_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_reproducible_runs_and_verified_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let history = ExperimentHistory::open(directory.path().join("history.sqlite")).unwrap();
        let manifest = serde_json::json!({"name": "opening-auction"});
        let policy = serde_json::json!({"max_targets": 2});
        history
            .begin("run-1", "opening-auction", 42, 2, &manifest, &policy)
            .unwrap();
        let artifact = history
            .attach_artifact("run-1", "result.json", "application/json", b"{\"ok\":true}")
            .unwrap();
        assert_eq!(artifact.sha256.len(), 64);
        let completed = history.finish("run-1", "succeeded").unwrap();
        assert_eq!(completed.seed, 42);
        assert_eq!(completed.artifact_count, 1);
        assert_eq!(history.artifacts("run-1").unwrap()[0], artifact);
    }

    #[test]
    fn retention_removes_old_runs_and_their_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let history = ExperimentHistory::open(directory.path().join("history.sqlite")).unwrap();
        for index in 0..3 {
            let id = format!("run-{index}");
            history.begin(&id, &id, index, 1, &id, &()).unwrap();
            history
                .attach_artifact(&id, "result.json", "application/json", b"result")
                .unwrap();
        }
        let removed = history
            .prune(RetentionPolicy {
                max_runs: 1,
                max_age_days: 30,
                max_artifact_bytes: 1024,
            })
            .unwrap();
        assert_eq!(removed, 2);
        assert_eq!(history.list(10).unwrap().len(), 1);
    }
}
