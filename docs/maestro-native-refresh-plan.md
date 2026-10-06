# Maestro native integration refresh

## Scope

Implement the recommendations from the October 2026 cross-repository review in
QRMI, maestro-local-server and Maestro. Preserve unrelated local modifications.
QCSim and the GPU plugin require no source changes for this integration.

## Implementation sequence

1. Reject OQTOPUS `job_spec` in every Maestro payload entry point; extend the
   shared payload fixtures and regression tests.
2. Add current native tensor examples and QRMI integration assertions covering
   complex/operator results, repeated observables, routing, wide bitstrings,
   gate-fusion metadata and option migration. Update current guidance and label
   historical documentation.
3. Add an additive API v2 `status` command without session acquisition or renewal.
   Report a consistent queue/active-work snapshot and runner lifecycle health.
   Consume it through QRMI's blocking executor; fall back only when the command
   or legacy API is unsupported. Preserve transport/protocol/server failures.
   Leave acquisition capacity and device readiness unspecified.
4. Extend native capabilities with diagnostic/backend-method support, option
   choices/constraints and build identification; expose server build information.
   Keep existing fields compatible and validation authoritative.
5. Rebuild Maestro, the server and QRMI bindings. Run Rust formatting, Clippy,
   Rust tests, Python Black/Pylint/unit and native integration tests, C++ format
   checks and relevant CTest suites, documentation checks and diff review.

## Validation and completion

Record commands, outcomes, skipped hardware-dependent checks and implementation
details below as work completes. Tests must use the rebuilt native library and
server rather than an unrelated installed version. Use isolated test daemons;
do not restart the user's production service.

## Completed implementation

All five steps are complete.

- QRMI rejects `job_spec` in Maestro's Python helper, Rust runner and native JSON
  schema. Shared fixtures include bare, wrapped and serialized forms. JSON Schema
  cannot inspect a serialized JSON string; those fixtures explicitly distinguish
  schema acceptance from the runner's rejection after decoding.
- Six CPU-native examples cover MPS/MPO operators, batched estimates, incremental
  evolution, bulk queries and 65-qubit results. Integration tests also execute
  them on the GPU, checking numerical values, scale, duplicates and ordering.
- The server advertises API v2 `status`. Its snapshot follows the existing queue
  lock order, excludes active work from the pending count and detects stopped or
  poisoned runner state. The runner's liveness guard clears on exit/unwinding.
  Status creates/renews no sessions and does not claim GPU readiness or capacity.
- QRMI queries status on its existing blocking executor. Explicitly unsupported
  commands fall back to PING; malformed responses and other failures remain
  errors. Older-server tests and both language bindings cover this behavior.
- Maestro advertises diagnostic configurations and option applicability, enums
  and bounds, with explicit conditional constraints/exceptions. The original
  capability fields remain intact and request validation stays authoritative.
  Native/server build identifiers tolerate source archives without accidentally
  identifying an unrelated parent repository.
- Migration and deployment guidance is updated in all three repositories.
  Existing Maestro `build.sh` edits and prior native documentation additions were
  preserved; unrelated untracked files were left alone.

## Verification results (2026-10-06)

| Check | Result |
| --- | --- |
| QRMI Rust library (`cargo test --locked -p qrmi --lib --features build-binary`) | 104 passed |
| Rust task runner (`cargo test --locked -p qrmi --bin task_runner --features build-binary`) | 6 passed |
| Bundled Maestro client | 6 library tests passed with `--include-ignored`, including the live `/run/maestro.sock` smoke test; 2 session integration tests passed |
| QRMI Rust doctests | 17 passed |
| Python unit/native integration suite, including C ABI, both runners and GPU | 333 passed; 8 distributed/MPI tests skipped |
| Local server (`cargo test --locked`) | 83 passed: 64 unit, 10 build-configuration, 8 native API, 1 worker CLI |
| Native CTest (`maestro_request_api`, `maestro_request_cli`, `maestro_request_abi`) | 3 passed |
| Rust formatting and strict Clippy, both Rust repositories | Passed |
| Black over Python sources; Pylint over Python sources | Passed; Pylint 10.00/10 |
| Maestro clang-format 16.0.6, changed C++ files | Passed |
| Sphinx HTML with warnings treated as errors | Passed |
| QRMI configured detect-secrets hook; all three `git diff --check` checks | Passed |

The Python suite used rebuilt editable bindings, `target/debug/libqrmi.so`,
`target/debug/task_runner`, the rebuilt local-server daemon and
`/home/adrian/maestro/build/bin/maestro-request`. Native tests explicitly prepended
`/home/adrian/maestro/build` to `LD_LIBRARY_PATH`; the default environment otherwise
resolved an older installed library. GPU tests used the available RTX 5090 and
existing GPU plugin. The remaining distributed/MPI tests need configured launch
profiles and allocations and were not represented as verified.

CMake was configured with `MAESTRO_BUILD_REQUEST_TESTS=ON`, then built the
`maestro_request_worker`, `maestro_request_tests` and
`maestro_request_abi_consumer` targets. No GPU/QCSim sources were modified.
Pylint used the existing compatibility environment for optional IQM dependencies
and excluded generated `.pyi` stubs, which are not hand-written Python sources.
Black was checked one file at a time because sandbox process-pool execution hung;
the stalled formatter processes were stopped. Socket/GPU tests and Sphinx's
GitHub contributor query required execution outside the restricted sandbox.

No production daemon was restarted or installed library replaced. Deployment
requires installing the reviewed compatible artifacts and restarting the service;
source changes and passing isolated tests do not update an already running daemon.


## Pre-commit verification

The final publication checks also passed the default-member Rust targets with
`cargo test --locked --all-targets --features build-binary`, strict Clippy for
those targets and for `build-binary,pyo3`, and formatting, Clippy and tests in
`examples/qrmi/rust`. The Python compatibility suite passed all 32 tests using the
rebuilt extension. The configured detect-secrets hook checked all 20 intended
files. The server-backed library smoke test was explicitly enabled and passed;
Rust's `#[ignore]` does not automatically detect a running server.

An additional `--workspace` test attempt tried linking OQTOPUS's Python extension
as a standalone Rust test and failed on Python symbols. That crate is excluded
from the repository's default test members; the documented/default test targets
and the Python binding checks above passed without changing unrelated code.

Related changes are on Maestro main (`216397b`, `aca67a5`) and local-server main
(`8ff5a75`). The wrapper-test library-selection fix also passed both affected
CPU/GPU CTest cases. The earlier deployment note records the implementation run;
the installed Maestro service was subsequently verified running, and its QRMI
smoke test passed before this commit.
