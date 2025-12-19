# Security Considerations

## TL;DR

This tool can break things on purpose. Use it responsibly. Test in dev/staging, not production (unless you know what you're doing).

## Why This Matters

Chaos engineering deliberately breaks services to find weaknesses. That means this tool needs elevated privileges for some operations, and if misconfigured, could cause actual problems.

## What Needs Privileges

| Type | Needs | Why |
|------|-------|-----|
| Network latency/loss | root/sudo | Modifies kernel network stack (Linux tc/netem) |
| TCP resets | root/sudo | Raw socket access (iptables) |
| CPU/memory/disk | normal user | Regular process stuff |
| Process management | normal user* | Can kill your own processes |

\* Need sudo to mess with other users' processes

## Safe Usage

### Start Small

Don't go straight to "kill everything" mode:

1. Test in dev environment first
2. Use short durations initially (1-2 minutes)
3. Low intensity to start (50% CPU, not 95%)
4. One type of chaos at a time
5. Have a way to stop it quickly

### Check Your YAML

Always validate before running:

```bash
./target/release/chaos validate scenarios/my_test.yaml
```

Catches config errors before you break anything.

### Monitor While Testing

Keep an eye on:
- CPU/memory usage (Task Manager, htop, top)
- Network traffic
- Service logs
- Error rates

If something looks really wrong, Ctrl+C stops the chaos.

### Network Chaos (Platform-Specific)

Network operations work differently on each platform:

**Linux** - Uses kernel tools (requires sudo):
```bash
sudo tc qdisc add dev eth0 root netem delay 100ms
sudo iptables -A OUTPUT -p tcp --dport 8080 -j DROP
```

**macOS** - Uses dummynet/pfctl (requires sudo):
```bash
sudo dnctl pipe 1 config delay 100ms
sudo pfctl -a chaos -f /path/to/rules
```

**Windows** - Application-level simulation (no admin needed):
- Simulates delays and packet loss at application layer
- Safer, but less realistic than kernel-level manipulation

The framework handles platform differences automatically.

## Security Features

### Input Validation

All YAML configs are validated:

```rust
// Intensity must be 0.0-1.0
if self.intensity < 0.0 || self.intensity > 1.0 {
    return Err("Intensity out of range");
}

// Durations can't be crazy long
if self.duration.as_secs() > MAX_DURATION {
    return Err("Duration too long");
}
```

### Safe Command Execution

No shell injection risks:

```rust
// GOOD - explicit args
Command::new("tc")
    .args(&["qdisc", "add", "dev", interface])
    .arg(format!("delay {}ms", delay))
    .output()?;

// BAD - don't do this
// Command::new("sh").arg("-c").arg(user_input)  // NOPE
```

### Audit Logs

Everything is logged:

```rust
tracing::info!(
    injection = "network_latency",
    target = target_name,
    delay_ms = delay,
    "Applying chaos injection"
);
```

Check logs if something unexpected happens.

## Common Mistakes

### Running as Root Unnecessarily

Network chaos needs sudo on Linux/macOS only. Windows and all CPU/memory/disk operations work as normal user.

```bash
# Don't do this for CPU testing
sudo ./chaos run cpu_test.yaml  # Unnecessary

# Do this
./chaos run cpu_test.yaml  # Fine for CPU/memory/disk
```

### Testing in Production Without Planning

Have a rollback plan:
- Know how to stop the test (Ctrl+C)
- Have monitoring/alerts set up
- Test during low-traffic times
- Start with non-critical services

### Platform Differences

Network chaos works differently on each platform:

```yaml
# Works everywhere now!
injections:
  - type: "network_latency"  # Linux: tc/netem, macOS: dnctl, Windows: simulated
    delay: "100ms"
  - type: "packet_loss"      # Cross-platform support
    rate: 0.05
```

**Note:** Linux/macOS use kernel-level tools (requires sudo), Windows uses application-level simulation (no admin needed).

Check the platform support table in the README.

## Reporting Security Issues

Found a vulnerability? Please email [maintainer email] instead of opening a public issue. I'll respond within a few days.

Include:
- What you found
- How to reproduce it
- Potential impact
- Suggested fix (if you have one)

## Best Practices

### For Development Testing

```yaml
# Safe dev test
phases:
  - name: "light_test"
    duration: "1m"  # Short
    injections:
      - type: "cpu_starvation"
        intensity: 0.3  # Low intensity
```

### For Staging/Production

```yaml
# More realistic but still safe
phases:
  - name: "baseline"
    duration: "5m"  # Longer baseline
  - name: "stress"
    duration: "3m"
    injections:
      - type: "cpu_starvation"
        intensity: 0.7  # Higher but not maxed
  - name: "recovery"
    duration: "5m"  # Verify recovery
```

### Using in CI/CD

```bash
# In your pipeline
./chaos run scenarios/integration_test.yaml --output-json results.json

# Check exit code
if [ $? -ne 0 ]; then
    echo "Chaos test failed!"
    cat results.json
    exit 1
fi
```

## Linux-Specific Notes

### Network Namespace Isolation

For safer testing, use network namespaces:

```bash
# Create isolated network
sudo ip netns add chaos-test
sudo ip netns exec chaos-test ./my_service

# Run chaos in that namespace
sudo ip netns exec chaos-test ./chaos run test.yaml
```

### Capabilities Instead of Full Root

Use Linux capabilities for network chaos without full root:

```bash
# Give binary network admin capability
sudo setcap cap_net_admin+ep ./target/release/chaos

# Now run without sudo
./chaos run network_test.yaml
```

## Windows Notes

Network chaos now works on Windows using application-level simulation! No admin privileges required.

```powershell
# Everything works on Windows now, including network chaos
.\scripts\simple_test.ps1   # CPU + network
.\scripts\stress_test.ps1   # Full chaos including network latency/packet loss

# Network stuff will fail gracefully
# .\chaos.exe run network_test.yaml  # Shows "not supported" error
```

## Recovery

If chaos gets stuck or things go wrong:

### Stop Everything

```bash
# Kill chaos
pkill -9 chaos

# On Linux, clean up network rules
sudo tc qdisc del dev eth0 root 2>/dev/null
sudo iptables -F 2>/dev/null

# Restart your service if needed
systemctl restart my-service
```

### Check What Changed

```bash
# Linux - see active network rules
tc qdisc show
iptables -L

# Check processes
ps aux | grep chaos
ps aux | grep my_service

# Check logs
tail -f /var/log/my_service.log
```

## License

This tool is MIT licensed - use it however you want, but at your own risk. No warranties, no guarantees. If you break production, that's on you. 😅

## Questions?

- Read the README
- Check the docs/ folder
- Open an issue on GitHub
- Email [maintainer email]

---

**Remember:** Chaos engineering is powerful. With great power comes great responsibility to not break prod. 🦀
