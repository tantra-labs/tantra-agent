# `tantra-agent` — Universal Result Protocol Reference

Documents `tantra-agent-core/src/result.rs`: the `tantra/result/v1` envelope returned by
every quantitative tool in `tantra-agent-mcp` and `tantra-agent-chat-tools`. The envelope
is the single rendering contract for tearsheets, metric grids, tables, and charts across MCP
clients (Claude Desktop, Cursor, Gemini CLI, internal chat loops).

## `TantraResultEnvelope<T>`

```rust
pub struct TantraResultEnvelope<T> {
    pub schema: String,          // always "tantra/result/v1"
    pub status: ResultStatus,    // Ok | Error
    pub meta: ExecutionMeta,     // elapsed_ms, timestamp_ist, version, precision
    pub summary: String,         // one-line natural-language summary
    pub data: T,                 // exact typed machine-readable payload
    pub blocks: Vec<DisplayBlock>, // typed visual presentation blocks
    pub refs: HashMap<String, String>, // $tantra/ref/<uuid> URIs for large series
}
```

### `ResultStatus`

| Variant | Meaning |
|---------|---------|
| `Ok` | Execution completed successfully |
| `Error` | Execution failed; `data` carries the error payload |

### `ExecutionMeta`

| Field | Type | Meaning |
|-------|------|---------|
| `elapsed_ms` | `u64` | Wall-clock elapsed time in ms |
| `timestamp_ist` | `String` | RFC3339 timestamp in Asia/Kolkata (IST) |
| `version` | `String` | Engine crate version (`CARGO_PKG_VERSION`) |
| `precision` | `String` | Always `"exact_decimal_zero_floats"` |

### `DisplayBlock` (typed visual presentation)

| Variant | Fields | Use |
|---------|--------|-----|
| `Table` | `title`, `columns`, `rows` | Titled table of string rows |
| `MetricGrid` | `items: Vec<MetricItem>` | Grid of named metric values |
| `Chart` | `title`, `chart_type`, `series_ref`, `series` | Chart ref into `$tantra/ref/...` series plus optional bounded `(ts_ms, decimal_str)` `series` resolved by chat hosts via `RefReader` (`MAX_CHART_SERIES_POINTS = 500`, omitted when empty) |
| `Alert` | `level`, `message` | info / warning / critical alert |

### Builder Methods

- `TantraResultEnvelope::ok(summary, data)` — success envelope.
- `TantraResultEnvelope::error(summary, message)` — error envelope (`T: From<String>`).
- `.with_block(block)` / `.with_blocks(iter)` — append presentation blocks.
- `.with_ref(key, uri)` — insert a large-dataset reference URI.

## Precision Contract

Every envelope's `ExecutionMeta.precision` is `"exact_decimal_zero_floats"`. Any
`Decimal`-derived numbers serialised to JSON are rendered as strings so precision survives
the wire (see `tantra-agent-core/src/ref_store.rs` zero-floats note).

## Related

- `tantra-agent-core/src/ref_store.rs` — `$tantra/ref/<uuid>` ephemeral store for large
  series referenced (not inlined) via `refs`, plus `RefReader` / `extract_chart_series`.
- `tantra-agent-mcp/src/tools.rs` — MCP tool handlers all return `TantraResultEnvelope`.
