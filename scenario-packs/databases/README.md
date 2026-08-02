# Database Failure Pack

Local DuckDB and SQLite profiles provide byte-preserving unavailability, restored read-only permissions, advisory locking, neighboring read/write pressure, and controlled inode pressure. These modes never corrupt the original database file.

PostgreSQL and MySQL profiles use the rootless dependency proxy for disconnects, delayed query responses, and connection-pool exhaustion. Point the application at the profile's `listen` port. Server-wide read-only mode is intentionally not automated because it requires privileged database credentials and changes shared server state; use the local read-only profile or a disposable database container.
