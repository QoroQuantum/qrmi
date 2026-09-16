# Maestro review: third-round plan and results

Date: 2026-09-16. Scope: the confirmed findings in the current `review.md`,
related regression coverage, documentation and native-request examples.

## Plan

1. **Preserve execution and cleanup diagnostics (review #3).** Retain bounded
   structured causes through the MPI supervisor, private worker protocol and
   public task logs. Cleanup uncertainty remains a failure, including repeated
   cancellation after the child is reaped. Test simultaneous native/protocol and
   cleanup failures, successful cleanup, and error-size bounds.
2. **Validate operation-specific shots (review #1).** Accept `execution.shots`
   only for `execute` and `checkpoint_batch`. The `validate` operation has no
   sampling semantics; validate sampling requests with the validation entry point
   while retaining their operation. Keep seed valid for other operations. Update
   shared fixtures and test validation/execution plus QRMI error propagation.
3. **Improve Python missing-field errors (review #8).** Check required fields
   explicitly and raise descriptive `ValueError`s before payload construction.
   Tighten tests for missing native/legacy fields and runner failure reporting.
4. **Cover and document qualified findings.** Add a checkpoint-suffix launch
   rejection regression (#4). Explain typed/native configuration paths (#2),
   standalone Rust keepalive versus QRMI polling/logs (#6), and the intentional
   compatibility-envelope exception (#7). Preserve the existing heartbeat policy,
   public QRMI trait, configuration representations and bounded scancel behavior.
5. **Audit examples and verify across projects.** Check existing examples against
   the stricter contract. Add useful examples for capabilities not represented by
   the existing noise/query/local-GPU/MPI-GPU examples, with automated validation
   and CPU execution checks. Rebuild and test in temporary directories; record
   hardware-dependent skips and update the relevant guides.

Production server/workers remain Rust/C/C++, without embedded or spawned Python.
Preserve unrelated edits, review reports and existing implementation records.
Do not install project libraries, restart system services or submit real jobs.

## Progress

- [x] Plan recorded.
- [x] Server diagnostics and regressions.
- [x] Native operation validation and regressions.
- [x] Python errors and cross-project regressions.
- [x] Documentation and example audit/additions.
- [x] Verification and final results.

## Implemented

- **Maestro:** rejects `execution.shots` outside `execute`/`checkpoint_batch`,
  including the standalone `validate` operation. Validation-only entry points
  still accept shots on sampling requests. Seeds retain their existing behavior.
  Updated native fixtures to carry shots only where used, and added a validation/
  execution rejection matrix and checkpoint-suffix launch regressions.
- **Local server:** a bounded error chain preserves native and I/O/protocol causes
  when supervisor cleanup also fails. The supervisor transmits the failure
  document before exiting with its unconfirmed-cleanup status, and the parent
  retains it when checking exit/cleanup. Public task logs expose nested
  `error.causes`, with `cleanup_unconfirmed` and `cleanup_confirmed:false` on the
  primary failure. Unknown cleanup remains a failure after reaping. A still-live
  child can be retried following a transient signalling error. Tests cover both
  cases, structured errors across the production worker boundary, and bounded
  Unicode/nested diagnostics.
- **QRMI:** Python helper checks missing required fields before indexing them and
  raises descriptive `ValueError`s. Tests require that exception class and verify
  that the runner reports a missing field without submitting a task. Native
  integration tests verify shots/suffix rejection before task-ID allocation and
  retention of both native and cleanup failures in public task logs.
- **Documentation:** clarified explicit standalone Rust keepalive versus QRMI
  polling/logs; the intentional compatibility-envelope exception; meaningful
  shots; circuit-only checkpoint suffixes; bounded diagnostic chains; and the
  two active configuration-construction paths. Kept the public QRMI trait,
  heartbeat policy, typed configuration fields and scancel polling unchanged.

## Example audit

The audit found an existing invalid example: `native-query.json` supplied
`observables` on a `diagnostics` operation. It passed the outer payload schema but
failed native validation. Removed that inapplicable field, and added tests against
the actual native validator to prevent a repeat.

Added four CPU examples:

| File | Coverage |
|---|---|
| `native-thermal-idle.json` | Exact thermal gate relaxation, elapsed idle time and detuning. |
| `native-correlated-noise.json` | Seeded OU phase noise, realization averaging and MPS options. |
| `native-checkpoint.json` | An ideal saved prefix with noisy measured suffixes. |
| `native-incremental.json` | Repeated exact amplitude-damping Kraus evolution at selected steps. |

All eight native examples now have schema/Python-helper, Rust-runner and native
validation coverage. All six CPU examples execute through QRMI with numerical or
count assertions; correlated-noise repeatability is also checked. Native validation
of the MPI example uses its computational document, matching the server's removal
of outer launch metadata. Hardware examples retain their documented prerequisites.
Usage and expected results are in the
[example README](../examples/task_runner/maestro_local/README.md).

## Verification

| Suite | Result |
|---|---|
| Maestro public native-API suite | 592 checks passed. |
| Native ABI/CLI/CPU-MPI/local-GPU smoke tests | 5 passed. |
| Local server | 49 unit tests and 1 production-worker integration test passed. |
| Rust task runner | 6 passed, including all native examples. |
| Focused Python/C/schema/native integration | 77 passed; 2 MPI GPU cases skipped because CUDA-aware MPI is unavailable. |
| Live legacy integration on a private daemon | 4 passed; 2 Aer cases skipped because this build omits Aer. |
| Final checks | Changed Rust/Python formatting and three-project diff checks passed. |

The public diagnostic integration case was also rerun after the final cleanup-retry
safeguard. Builds and test daemons used `/tmp/maestro-capabilities` and an isolated
Python package under `/tmp/maestro-review-fixes/python-package`; staging/patches
for this round are under `/tmp/maestro-review-round3`.

Local GPU verification uses two logical shards on one RTX 5090. Physical multi-GPU,
CUDA-aware MPI GPU, Windows and real Slurm execution remain unverified here; Slurm
failure tests use mock launchers and `/bin/false`, never real scheduler commands.
No project libraries were installed, no system services were restarted, and no
real jobs were submitted. Production workers remain native Rust/C/C++.
