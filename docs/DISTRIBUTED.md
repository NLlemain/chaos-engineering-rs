# Distributed Experiments

The v0.4 control plane coordinates existing scenarios across mutually authenticated agents. It uses a small length-delimited JSON protocol over TLS 1.2 or 1.3; both sides must present a certificate signed by the configured CA.

## Trust Model

- Give the orchestrator a client certificate and each agent a unique server certificate.
- Put the agent's DNS name in its certificate SAN and in the manifest `server_name` field.
- Keep the CA private key offline after provisioning.
- Give each agent only the operating-system and Kubernetes permissions its local policy allows.
- Use separate CAs for development and production control planes.

The agent validates policy during `prepare`, before any mutation. The orchestrator validates the same scenario independently. A failed prepare or execution sends `recover` to every prepared target.

## Certificates

The following OpenSSL 3 commands create a development CA and one agent identity. Repeat the server CSR and signing steps for each agent, changing the common name and SAN.

```bash
mkdir -p certs
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out certs/ca-key.pem
openssl req -x509 -new -key certs/ca-key.pem -sha256 -days 3650 \
  -subj "/CN=Chaos Engineering RS Development CA" -out certs/ca.pem

openssl req -new -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes \
  -keyout certs/ams-1-key.pem -out certs/ams-1.csr \
  -subj "/CN=ams-1.chaos.test" -addext "subjectAltName=DNS:ams-1.chaos.test"
openssl x509 -req -in certs/ams-1.csr -CA certs/ca.pem -CAkey certs/ca-key.pem \
  -CAcreateserial -days 365 -sha256 -copy_extensions copy -out certs/ams-1.pem

openssl req -new -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes \
  -keyout certs/orchestrator-key.pem -out certs/orchestrator.csr \
  -subj "/CN=orchestrator.chaos.test"
openssl x509 -req -in certs/orchestrator.csr -CA certs/ca.pem -CAkey certs/ca-key.pem \
  -CAcreateserial -days 365 -sha256 -out certs/orchestrator.pem
```

These commands are for local development. Use the organization's PKI, short-lived certificates, protected private-key storage, and rotation process for shared environments.

## Start Agents

```bash
chaos agent serve \
  --id ams-1 \
  --listen 0.0.0.0:9443 \
  --ca-cert certs/ca.pem \
  --cert certs/ams-1.pem \
  --key certs/ams-1-key.pem \
  --policy examples/distributed-policy.yaml
```

The server refuses an unauthenticated client during the TLS handshake. Agent IDs are checked against the manifest so a valid certificate on the wrong host cannot silently impersonate the intended assignment.

## Run And Inspect

```bash
chaos distributed examples/distributed-experiment.yaml \
  --ca-cert certs/ca.pem \
  --cert certs/orchestrator.pem \
  --key certs/orchestrator-key.pem \
  --policy examples/distributed-policy.yaml \
  --output distributed-result.json

chaos history list --json
chaos history show EXPERIMENT_ID
```

Phases are sequential. Assignments inside a phase are prepared together and executed in batches. The effective batch size is the lower of `max_parallel_targets` and the requested percentage of unique targets, bounded again by policy.

## History And Recovery

The default database is `~/.chaos-engineering/history.sqlite`. Each run stores its root seed, manifest digest, policy digest, status, target count, per-target result artifacts, and final distributed result. Artifacts carry SHA-256 hashes.

Each agent keeps one recovery journal per execution. `recover` cancels a running scenario, waits for its cleanup path, and replays any remaining journal entries. `stop_all` is available in the control protocol for emergency orchestration.
