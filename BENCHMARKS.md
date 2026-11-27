# Benchmarks

Machine: primo — 13th Gen Intel i7-13620H, 16 threads, 62 GiB RAM, cargo 1.100.0 (2026-11-05).
Method: hand-rolled `std::time` harness in `benches/hot_paths.rs` (harness = false), named-constant workloads, warmup before timed loops, results printed as ns/op and ops/s.
Note: primo was under multi-lane load tonight (load average ~40+ at times), so these numbers are baselines, not peaks; expect substantial run-to-run variance on cold start in particular.

Server: `verse-ls --stdio` (LSP, Content-Length framing).

Workloads:
- `cold_start_initialize`: spawn server, send `initialize` + `initialized` + `didOpen` on a 4-line Verse fixture, 10 reps.
- `document_symbol_request`: one warm session, 20 warmup + 200 timed `textDocument/documentSymbol` requests. Rung 0 advertises text-document sync only, so each request is answered `-32601 Method not found`; this measures the request round-trip, not symbol computation.

Results (2026-11-27):
- cold_start_initialize: 12,522,029 ns/op (~12.5 ms) over 10 reps (an earlier run measured 4.76 ms)
- document_symbol_request: 52,264 ns/op (~0.052 ms), 19,133.5 ops/s over 200 requests (earlier run: 101,835 ns/op, 9,819.8 ops/s)

Example: `cargo run -p verse-ls --example lsp_session` — initialize (capabilities + serverInfo verse-ls 0.1.0) → didOpen → documentSymbol (honest -32601 at rung 0) → shutdown → clean exit 0.
