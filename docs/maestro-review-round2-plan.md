# Maestro review.md: implementation plan and results

Date: 2026-09-16. This implements the confirmed findings from `review.md` and
the additional tests identified during its source review.

## Scope and decisions

Change only Maestro, maestro-local-server and QRMI. Preserve unrelated edits,
including Maestro's `build.sh`, and keep production workers in Rust/C/C++.
Do not install libraries, restart system services or submit scheduler jobs.

- Fix supervisor cancellation races without accepting unconfirmed rank cleanup.
- Fix oversized API error framing, MPI validation error categories, executable
  prerequisites, malformed validation envelopes and ignored launch metadata.
- Capture one socket per QRMI resource for both native and legacy commands;
  clear stale terminal status when native submission reuses a task ID.
- Align runner/schema handling of null configuration and ambiguous payloads;
  update the live integration expectations.
- Broaden native noise/option/optimization tests and add opt-in real MPI GPU
  coverage. Enable CPU request contract tests in CI and exercise the public C
  symbols from an external consumer, including on Windows when available.
- Document QRMI lease renewal using existing task traffic. Preserve the lease
  policy; no implicit background renewal or capability probes are required.
- Retain defensive version-error mappings, current compiled/readiness semantics
  and the optimization default. The review did not establish defects there.

## Implementation order

1. **Server lifecycle and boundaries:** add a worker readiness handshake and
   explicit cleanup-confirmed exits; install signal handling before accepting a
   task. Keep timeouts and failed Slurm cleanup authoritative. Add deterministic
   startup, in-flight and post-cleanup cancellation tests. Separate launch
   validation from the native computational document; validate error envelopes,
   executable prerequisites and transport error framing.
2. **QRMI transport and payloads:** route all resource operations through the
   captured socket, test two daemons and environment changes, and invalidate
   reused task status. Reject ambiguous wrappers consistently; normalize null
   legacy configuration and update the schema and both runner tests.
3. **Maestro contract and tests:** reject server-only launch metadata at native
   entry points. Add deterministic numerical/channel parsing checks, option
   effects and optimization comparisons. Add optional MPI GPU tests and enable
   the native contract suite in CI. Make the existing export declarations
   explicit and test all public request symbols without changing their ABI.
4. **Cross-project verification:** rebuild in isolated temporary directories;
   run native, server, Rust client/adapter/runner, Python/C/schema/live and native
   integration suites. Run available CPU MPI/local GPU checks, and clearly
   record unavailable Windows, CUDA-aware MPI, multi-GPU and Slurm verification.

## Progress

- [x] Plan recorded.
- [x] Server fixes and regression tests.
- [x] QRMI fixes and regression tests.
- [x] Maestro contract, coverage and CI changes.
- [x] Cross-project verification and final results.

## Implemented changes

| Review item | Resolution |
|---|---|
| 1: MPI cancellation races | Added a private worker readiness handshake after signal registration. The parent sends no task before readiness. Cancellation distinguishes no launch, confirmed cleanup, and unknown/failed cleanup. Response loss after successful cleanup is accepted; arbitrary nonzero exits and cleanup timeouts still fail. |
| 2: Oversized API error framing | The socket reader now emits a versioned `invalid_input` envelope for oversized `API ` frames. Tests exercise the exact limit, over-limit API input and legacy framing. The current client also rejects oversized frames before connecting. |
| 4: Stale live expectations | Live tests check capabilities, missing-task errors and retained diagnostics. They discover build-dependent legacy backend IDs from capabilities and skip absent backends. |
| 5: Socket changes during a resource lifetime | Added explicit-socket session/task client operations and routed every QRMI operation through the resource's captured socket. A two-daemon regression changes the environment mid-lifetime and checks acquisition, queries, submission, status, logs, cancellation, results and release. |
| 6–7: MPI validation and cancellation prerequisites | Malformed launch/rank input maps to `invalid_input`; profile-file or missing/nonexecutable tool problems map to `invalid_config`; unavailable profile selection maps to `unsupported_capability`. `srun` requires executable `scancel` before submission. |
| 8, 10: Runner/schema parity | Both runners normalize omitted/null legacy configuration to `{}` and reject ambiguous wrappers or mixed-backend documents. The schema matches these rules. A common 22-case fixture runs through Rust, Python and JSON Schema validation. |
| 11: Ignored native launch metadata | The server validates the outer profile then strips `launch` before native validation/execution. Direct native C/CLI entry points reject it, including nested batch fields. Tests exercise valid MPI batches and rejected nesting. |
| 14: Native noise, options and MPI GPU coverage | Added numerical public-API tests for thermal/idle relaxation, phase/generalized amplitude damping, correlated AR1/OU/OU-band/multiband/one-over-f noise, crosstalk, analytical damping, invalid parameters, seeded repeatability, optimization comparisons and MPS bond-limit effects. Added opt-in real `distributed_mpi_gpu` tests in native CTest and QRMI integration. |
| Reused terminal task IDs | Successful native submission evicts any cached terminal status for the returned ID; the two-daemon test also checks reuse. |
| Malformed validation envelopes | Native responses must have the expected schema, boolean status and complete string error fields on failure. Invalid envelopes become `native_failure`. Valid batch-validation envelopes remain supported. |
| Contract tests missing from CI | Enabled request tests in Maestro Linux CI and the server's Maestro dependency build. Added an independent C consumer exercising all request symbols and `FreeResult`; the Windows wheel dry-run workflow runs it alongside API/CLI tests. |

The new cancellation tests also exposed a late Slurm step-marker race: cancellation
could start before the supervisor received the marker, preventing `scancel` from
ever being invoked. The supervisor now drains output and checks for a newly
available step before processing launcher exit. Tests cover both successful and
failed cleanup in that sequence.

## Qualified findings and retained behavior

- **3, Windows exports:** the definitions already received export declarations
  through an indirect include, so missing exports were not established. Added an
  explicit public-header include and an external C consumer test; no ABI redesign.
  The Windows workflow is configured but was not executed on this Linux host.
- **9, leases:** retained the existing lease policy. Documentation now distinguishes
  real task/log traffic and explicit keepalive from cached status/session existence
  checks. A one-second-lease integration test repeatedly retrieves consumed-task
  logs for longer than the lease and proves renewal. No background heartbeat added.
- **12, noise optimization:** source inspection showed injection occurs before
  network optimization. On/off regressions for exact relaxation, Pauli and coherent
  noise pass; the default remains unchanged. Documentation distinguishes those
  checks from guarantees about approximate methods and truncation.
- **13, capabilities:** `compiled` continues to identify build support, with runtime
  prerequisites/readiness documented separately. No automatic GPU/license probe.
- The `unsupported_version` mapping is retained defensively and has a direct
  server-error regression. Capability catalog queries still run inside the daemon,
  as documented. Invalid Slurm step identifiers already fail the protocol; the
  review's claim that they were silently skipped did not justify widening them.

## Verification results

All builds and daemon instances used temporary directories and private sockets.
No installed library, system daemon or Slurm allocation was changed.

| Suite | Result |
|---|---|
| Maestro native public-API suite | 492 checks passed, including the added noise/option numerical regressions. |
| Maestro CTest smoke tests | 5 passed: standalone C ABI consumer, CLI, two-rank CPU MPI success/failure and local distributed GPU. |
| Local-server Rust suite | 44 unit tests plus 1 production-worker integration test passed. |
| QRMI Maestro Rust adapter | 18 tests passed. |
| Bundled Rust socket client | 5 passed; 1 existing default-system-socket test intentionally ignored. |
| Rust task runner | 5 tests passed, including the common payload fixtures. |
| Focused Python/C/schema/native integration | 38 passed; 2 real MPI GPU cases skipped because no CUDA-aware MPI profile is available. Includes both CLI runners, CPU MPI, local GPU ideal/noisy execution, typed errors and lease renewal. |
| Live legacy QRMI integration on a private daemon | 4 passed; 2 Aer cases skipped because the tested Maestro build omits Aer. |
| Static checks | Changed Rust/Python formatting, workflow YAML parsing and `git diff --check` passed. Native executable/library dependencies resolve without `libpython`. |

The local GPU tests use two logical shards on one RTX 5090; they do not establish
physical multi-GPU or cross-node behavior. Actual CUDA-aware MPI GPU, Windows and
real Slurm cancellation remain environment-dependent verification. Slurm launch and
cleanup paths were exercised with deterministic mock executables. The opt-in MPI
GPU tests assert the actual distributed backend and rank count, rather than
accepting CPU orchestration as equivalent coverage.

Native build/test artifacts are in `/tmp/maestro-capabilities/build`; server and
QRMI Cargo outputs are under `/tmp/maestro-capabilities/{server-target,qrmi-target}`.
The Python suite used an isolated package containing the rebuilt QRMI extension
under `/tmp/maestro-review-fixes/python-package`. Temporary test data and external
project patches are under `/tmp/maestro-review-round2`.

Updated usage and test instructions are in [the QRMI native API guide](maestro-native-api.md),
Maestro's `docs/native-request-api.md`, and the server's `docs/native-api-v2.md`.
The original `review.md`, unrelated edits and Maestro's `build.sh` were preserved.
