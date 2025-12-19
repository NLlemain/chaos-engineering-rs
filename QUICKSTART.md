# Quick Start Guide

## Build & Test (5 minutes)

### 1. Build

```bash
cargo build --release
```

Takes ~30 seconds. Binaries go in `target/release/`.

### 2. Launch Web Dashboard

```bash
./target/release/chaos serve --port 8080
# Open http://127.0.0.1:8080
```

### 3. Quick Test (3 minutes)

```bash
# Windows
.\scripts\simple_test.ps1

# macOS/Linux (with PowerShell Core)
pwsh ./scripts/simple_test.ps1

# Or manually
./target/release/chaos run scenarios/quick_test.yaml --verbose
```

### 4. Full Stress Test (15 minutes)

```bash
# Windows
.\scripts\stress_test.ps1

# macOS/Linux (with PowerShell Core)
pwsh ./scripts/stress_test.ps1

# Or manually
./target/release/chaos run scenarios/stress_test.yaml --verbose
```

## Web Dashboard

Access at http://127.0.0.1:8080 after running `chaos serve`:

- **Dashboard**: Test status overview
- **Scenarios**: Browse and run YAML scenarios
- **Load Test**: Stress test your HTTP/WebSocket/TCP endpoints
- **Targets**: Save frequently tested endpoints
- **Results**: View test history

### Load Test Your App

1. Go to Load Test page
2. Enter your URL (e.g., `http://localhost:3000/api`)
3. Configure concurrent users, RPS, duration
4. Click Start
5. Watch real-time metrics

## Common Commands

```bash
# List all chaos types
./target/release/chaos list

# Validate a scenario before running
./target/release/chaos validate scenarios/my_test.yaml

# Run with reports
./target/release/chaos run test.yaml \
  --output-json results.json \
  --output-markdown report.md \
  --verbose

# Start web dashboard
./target/release/chaos serve --port 8080
```

## Platform Notes

### Linux
- Everything works
- Network chaos needs: `sudo apt install iproute2 iptables`
- Run network tests with sudo: `sudo ./chaos run test.yaml`

### Windows
- CPU, memory, disk work great
- Network chaos doesn't work (needs Linux kernel)
- PowerShell scripts work out of the box

### macOS
- CPU, memory, disk work
- Network chaos doesn't work
- Need PowerShell Core: `brew install powershell`

## Creating Your Own Tests

Copy and modify a scenario:

```yaml
name: "My Custom Test"
targets:
  - name: "my_service"
    type: "process"
    process_name: "my_app"  # Your process name

phases:
  - name: "baseline"
    duration: "1m"
    
  - name: "stress"
    duration: "2m"
    injections:
      - type: "cpu_starvation"
        target: "my_service"
        intensity: 0.7  # 70% CPU
        
  - name: "recovery"
    duration: "1m"
```

Save as `scenarios/my_test.yaml` and run:

```bash
./target/release/chaos run scenarios/my_test.yaml
```

## Troubleshooting

### "Binary not found"
Run `cargo build --release` first.

### "Service not running"
The test scripts auto-start the example service. For your own service, start it manually first.

### "Permission denied" (Linux network chaos)
Run with sudo: `sudo ./chaos run test.yaml`

### "Network chaos not supported" (Windows/macOS)
Expected. Use CPU/memory tests instead.

## Getting Help

- Check the [README](README.md)
- Look at example scenarios in `scenarios/`
- Read [SECURITY.md](SECURITY.md) for safety tips
- Open an issue on GitHub

## Quick Reference

| Command | What it does |
|---------|-------------|
| `chaos list` | Show all chaos types |
| `chaos validate FILE` | Check YAML syntax |
| `chaos run FILE` | Run a test |
| `chaos run FILE --verbose` | Run with detailed logs |
| `chaos run FILE --dry-run` | Validate without running |

## Example Scenarios

- `scenarios/quick_test.yaml` - 3-min CPU stress test
- `scenarios/stress_test.yaml` - 15-min comprehensive test

Both work on all platforms (Linux gets extra network chaos).

---

**That's it!** Start with `.\scripts\simple_test.ps1` and go from there. 🦀
