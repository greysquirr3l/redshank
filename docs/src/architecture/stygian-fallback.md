# Stygian MCP Fallback

Redshank fetches from two broad categories of data sources:

- **JSON/REST sources** — respond to plain HTTP requests; fetched with `reqwest`.
- **JS-heavy sources** — government portals, SOS registries, and social platforms that require a real browser to render state before content is extractable.

For JS-heavy sources, redshank uses an optional **stygian fallback**: when the
[stygian-mcp](https://github.com/stygian-labs/stygian) server is running and healthy,
fetch operations are delegated to it. When it is unavailable, the fetcher
**fails-soft** (logs a warning and returns an empty result set) instead of hard-crashing
the investigation.

## Decision flow

```
fetch(source)
  ├── is_js_heavy? ──NO──▶ NativeHttp (reqwest)
  └── YES
       ├── stygian reachable? ──YES──▶ StygianMcpFallback
       └── NO──▶ FailSoft (warn + return empty)
```

This logic lives in `redshank-fetchers/src/fallback.rs::select_execution_mode`.
Each JS-heavy fetcher module exposes a thin wrapper:

| Function | Module |
|----------|--------|
| `execution_mode_for_state_sos(availability)` | `fetchers/state_sos.rs` |
| `execution_mode_for_county(availability, has_json_api)` | `fetchers/county_property.rs` |
| `execution_mode_for_profile(requires_browser, availability)` | `fetchers/social_profiles.rs` |

All three delegate immediately to `select_execution_mode`. The policy stays in one place;
the per-source wrapper communicates domain intent (whether that source is JS-heavy).

## Feature flag

Stygian is an **opt-in** feature. The `redshank-fetchers` crate compiles without it by
default:

```bash
# Without stygian (default) — JS-heavy fetchers always fail-soft
cargo build --workspace

# With stygian
cargo build --workspace --features redshank-fetchers/stygian
```

When the `stygian` feature is absent at compile time,
`detect_stygian_availability` short-circuits and returns
`StygianAvailability::Unavailable(FeatureDisabled)` without attempting any
network connection. No stale connection errors; no silent DNS hits.

## Runtime probing

On startup the engine calls `detect_stygian_availability(&StygianProbeConfig)`.
The probe config drives the health check:

| Field | Default | Purpose |
|-------|---------|---------|
| `endpoint_url` | `http://127.0.0.1:8787/health` | Health endpoint to GET |
| `timeout_ms` | `1500` | Per-attempt request timeout |
| `retries` | `1` | Retry count after the first failure |

The probe accepts any HTTP 2xx response whose body contains one of:
`"ok"`, `"healthy"`, `status:ok`, or the bare string `ok`.

The result is broadcast as `AppEvent::FetcherHealthChanged(FetcherHealth)` so
the TUI can update the footer indicator without the probe code knowing anything
about ratatui.

## TUI health indicator

The footer of the interactive TUI shows the current stygian health:

| Glyph | Color | Meaning |
|-------|-------|---------|
| `▲` | Green | Available — probe succeeded |
| `▼` | Red | Down — probe failed or endpoint unhealthy |
| `?` | Gray | Unknown — probe has not run yet |

## Setup

### Local development

1. Install stygian-mcp:

   ```bash
   cargo install stygian-mcp --locked
   ```

2. Start the server on the default port:

   ```bash
   stygian-mcp --port 8787
   ```

3. Build redshank with stygian enabled:

   ```bash
   cargo build --workspace --features redshank-fetchers/stygian
   ```

4. Confirm the TUI footer shows `stygian: ▲`.

### Production / headless

For server deployments, run stygian-mcp as a managed service (systemd example):

```ini
[Unit]
Description=stygian-mcp browser automation server
After=network.target

[Service]
ExecStart=/usr/local/bin/stygian-mcp --port 8787
Restart=on-failure
RestartSec=5s
User=stygian

[Install]
WantedBy=multi-user.target
```

Point redshank at a non-default host with `REDSHANK_STYGIAN_ENDPOINT`:

```bash
export REDSHANK_STYGIAN_ENDPOINT=http://10.0.0.42:8787/health
```

> **Note:** The endpoint URL must include the `/health` path.

### Override probe config in settings.json

```json
{
  "stygian": {
    "endpoint_url": "http://10.0.0.42:8787/health",
    "timeout_ms": 3000,
    "retries": 2
  }
}
```

## Troubleshooting

| Symptom | Likely cause | Remediation |
|---------|-------------|-------------|
| TUI shows `stygian: ▼` | Server not running | `stygian-mcp --port 8787` |
| TUI shows `stygian: ?` | Probe hasn't fired yet | Wait for first fetch or restart |
| JS-heavy fetcher returns empty | stygian unavailable | Start server or rebuild without `stygian` feature to suppress warnings |
| `FeatureDisabled` in logs | Binary built without feature | Rebuild with `--features redshank-fetchers/stygian` |
| Probe times out | Wrong host/port or firewall | Check `endpoint_url`, verify stygian is listening |
| Endpoint unhealthy (non-2xx) | stygian starting up or misconfigured | Check stygian logs; increase `timeout_ms` |
| `stygian: ▲` but fetch still empty | Source-side anti-bot block | Configure proxy via stygian-mcp proxy pool |

## Licensing boundary

Stygian is used via **MCP server** rather than as a direct Cargo crate dependency.
This boundary is intentional even though both redshank and stygian share the same author.

**Rationale:**

- `stygian-browser` requires a running Chrome/Chromium instance accessed via CDP.
  Linking it directly into redshank would force every user to install Chrome regardless
  of whether they need JS-heavy fetchers. The MCP boundary keeps the Chrome dependency
  in a separate process that operators can opt into.
- Process isolation: a crash or OOM in the browser automation process cannot corrupt the
  investigation agent's in-memory state or SQLite session store.
- Independent restarts: the stygian-mcp server can be restarted, updated, or swapped
  out without recompiling or restarting redshank.

**When to use MCP indirection (general criteria):**

1. The dependency requires a native runtime or process that should be optional
   (Chrome, GPU driver, external daemon).
2. Process-boundary isolation is operationally valuable (crash containment, independent
   restarts, separate resource limits).
3. The capability is genuinely optional — callers should work without it.

**When direct Cargo crate linking is fine:**

1. Pure Rust or C library with no separate runtime process.
2. No binary-size or isolation concern.
3. All downstream users can reasonably be expected to have its native dependencies.
