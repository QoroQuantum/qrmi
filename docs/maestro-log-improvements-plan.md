# Maestro task log improvements

## Plan

- [x] Retain diagnostics for queued and running tasks cancelled successfully,
  including output drained during worker shutdown, until session deletion/expiry.
- [x] Bound captured stdout/stderr to a 32 KiB prefix and 32 KiB tail. Report
  omitted output with a text marker and an additive `logs_truncated` field.
- [x] Add timestamped task lifecycle events and bounded backend/launch context
  to the server's existing API-v2 diagnostic document.
- [x] Verify cancellation/result races, session cleanup, output boundaries,
  lifecycle ordering, and QRMI's lossless forwarding of the added fields.
- [x] Update server and QRMI documentation, run relevant checks, and apply the
  tested server patch to its checkout.

The server owns capture and lifecycle records. QRMI keeps forwarding the full
JSON document through `task_logs()` without changing its trait or bindings.
Lifecycle events describe server-observed transitions, not native progress.
Logs remain in memory and are removed on session cleanup or daemon restart.

The server changes were prepared and tested in a temporary copy because its
checkout is outside the writable workspace. The reviewed patch was applied to
`/home/adrian/slurm/maestro-local-server` with filesystem approval. All nine
applied files were verified to match the tested copy byte for byte.

## Validation

- Server: `MAESTRO_DIR=/home/adrian/maestro cargo test --offline --quiet`:
  67 passed (56 unit/process tests, 10 build configuration tests, 1 worker CLI test).
- Server: `cargo clippy --offline --all-targets -- -D warnings`: passed with the
  same native library configuration.
- QRMI: `cargo test --offline --lib maestro::`: 20 passed.
- Rust formatting and patch whitespace checks: passed.
- Python native integration tests: 4 passed (retained logs and event fields,
  native failure, composed native/cleanup failure, and lease renewal through logs).
  The pre-existing Python extension predates native requests; these checks used
  a fresh `cargo build --offline --lib --features pyo3,pyo3/extension-module`
  build with `PYO3_PYTHON` set to Python 3.12, loaded from a temporary package.

Socket tests ran outside the sandbox because local Unix sockets are blocked
inside it. Both repositories now contain the implementation, regression tests,
and updated documentation.
