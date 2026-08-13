#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: run.sh <postgres|mysql|kafka|rabbitmq|nats|mqtt|s3>}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
compose="$root/tests/protocol-evidence/compose.yml"
project="chaos-evidence-${target}-${GITHUB_RUN_ID:-local}-$$"
binary="$root/target/debug/chaos"

case "$target" in
  postgres|mysql|kafka|rabbitmq|nats|mqtt) service="$target" ;;
  s3) service="minio" ;;
  *) echo "unknown protocol evidence target: $target" >&2; exit 2 ;;
esac

dc() {
  docker compose -p "$project" -f "$compose" --profile "$target" "$@"
}

dct() {
  local duration="$1"
  shift
  timeout "$duration" docker compose -p "$project" -f "$compose" --profile "$target" "$@"
}

container_id() {
  dc ps -q "$service"
}

probe() {
  case "$target" in
    postgres)
      dct 5 exec -T postgres-client psql -h postgres -U chaos -d evidence \
        -c "SELECT 1" >/dev/null
      ;;
    mysql)
      dct 5 exec -T mysql-client mysql -h mysql -uchaos -pchaos evidence \
        -e "SELECT 1" >/dev/null
      ;;
    kafka)
      dct 5 exec -T kafka-client /opt/kafka/bin/kafka-topics.sh \
        --bootstrap-server kafka:9092 --list >/dev/null
      ;;
    rabbitmq)
      dct 5 exec -T rabbitmq-client /bin/sh -c \
        "test \"\$(printf 'AMQP\\000\\000\\011\\001' | nc -w 3 rabbitmq 5672 | wc -c)\" -gt 0"
      ;;
    nats)
      dct 5 exec -T nats-client nats --server nats://nats:4222 \
        pub chaos.evidence baseline >/dev/null
      ;;
    mqtt)
      dct 5 exec -T mqtt-client mosquitto_pub -h mqtt \
        -t chaos/evidence -m baseline >/dev/null
      ;;
    s3)
      dct 5 exec -T minio-client /usr/bin/mc alias set local \
        http://minio:9000 chaosadmin chaos-password >/dev/null
      dct 5 exec -T minio-client /bin/sh -c \
        "mc mb --ignore-existing local/evidence >/dev/null && printf evidence | mc pipe local/evidence/probe >/dev/null && test \"\$(mc cat local/evidence/probe)\" = evidence"
      ;;
  esac
}

wait_for_probe() {
  for _ in $(seq 1 60); do
    if probe; then
      return 0
    fi
    sleep 2
  done
  dc logs "$service" >&2 || true
  return 1
}

cleanup() {
  if [[ -n "${fault_pid:-}" ]]; then
    wait "$fault_pid" 2>/dev/null || true
  fi
  dc down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

dc up -d
wait_for_probe
echo "evidence[$target]: baseline passed"

cargo build --locked -p chaos_cli --bin chaos
"$binary" container \
  --compose-service "$service" \
  --compose-file "$compose" \
  --compose-project "$project" \
  --action pause \
  --duration 15s >"$root/target/protocol-evidence-${target}.log" 2>&1 &
fault_pid=$!

for _ in $(seq 1 30); do
  id="$(container_id)"
  if [[ -n "$id" ]] && [[ "$(docker inspect --format '{{.State.Paused}}' "$id")" == "true" ]]; then
    break
  fi
  sleep 1
done

id="$(container_id)"
[[ -n "$id" ]]
[[ "$(docker inspect --format '{{.State.Paused}}' "$id")" == "true" ]]
if probe; then
  echo "evidence[$target]: protocol probe unexpectedly succeeded during disruption" >&2
  exit 1
fi
echo "evidence[$target]: disruption passed"

wait "$fault_pid"
unset fault_pid
wait_for_probe
[[ "$(docker inspect --format '{{.State.Paused}}' "$id")" == "false" ]]
echo "evidence[$target]: restoration passed"
