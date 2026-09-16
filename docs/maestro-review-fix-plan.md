# Maestro preliminary review: implementation plan and results

Date: 2026-09-16. Follows the source review of `preliminary_review.md`.

## Scope

Fix A1, A2, B1, B2, C1, C2, C3 and C4 in Maestro, maestro-local-server and QRMI.
Also close the confirmed operation-specific validation gap and isolate native
request validation from the daemon. Preserve the native Rust/C/C++ server path;
no embedded Python or Python production workers. Preserve unrelated changes.

A3 is a reproducibility contract: retain sequential random streams, document
that branch order and preceding branches affect later samples, and test complete
request repeatability. A4's present fidelity semantics and B3's nested deadlines
are correct; clarify them and protect relevant behavior with tests. Measure C4's
extra query and cache successful protocol negotiation if material. Keep explicit
capability discovery fresh, invalidate on transport/protocol/version errors and
never retry submissions automatically. Retain the client in the QRMI resource.

## Changes and order

1. **Maestro validation and diagnostics.** Classify instruction parser input
   failures correctly; give checkpoint prefix errors their actual context;
   reject fields that do not apply to the selected operation or instruction.
   Document checkpoint randomness and ideal-versus-noisy fidelity semantics.
2. **Server error propagation and isolation.** Preserve complete native failure
   frames after an early input-pipe closure, while preserving cancellation,
   framing and cleanup failures. Drain output with existing bounds; distinguish
   trailing diagnostics from truncated or duplicate protocol frames. Run native
   request validation in an isolated Rust worker before atomic submission.
   Document the ordinary worker and nested shutdown budgets accurately.
3. **QRMI error contract and schema.** Map unsupported capabilities,
   configuration errors and expired sessions to useful existing QRMI error kinds.
   Retain negotiated protocol support across submissions and client clones, with
   invalidation and fresh explicit discovery; verify actual exchange counts.
   Align the JSON schema and both runner parsers for native shorthand, wrapped
   requests and legacy payloads without weakening legacy requirements.
4. **Both task runners.** Fetch failure logs before cleanup, report diagnostics
   to stderr, and return nonzero on failure/cancellation/result errors. Diagnostic
   retrieval failures must not hide the original outcome; cleanup must still run.
5. **Verification and documentation.** Run focused native, server, Rust adapter,
   runner, schema and Python tests, then native end-to-end tests. Record actual
   results here and document intentional limitations. Do not change installed
   services/libraries or launch scheduler jobs.

## Regression coverage

- Maestro: wrong arities and unknown gates return input errors; checkpoint reset
  and Kraus prefix diagnostics; irrelevant request/instruction fields; repeated
  seeded checkpoint sampling; delay equivalence in supported fidelity modes.
- Server: early stdin closure with complete errors, success after incomplete
  upload, newline-free diagnostics, truncated/duplicate/malformed frames,
  cancellation precedence, and isolated validation success/failure/crash handling.
- QRMI: mapped Rust/Python error kinds; all supported payload representations and
  invalid legacy cases; runner success, failure, cancellation, failed log fetch,
  result errors, stderr diagnostics, exit status and cleanup; native integration.

## Progress

- [x] Plan recorded.
- [x] Maestro changes and tests.
- [x] Server changes and tests.
- [x] QRMI changes and tests.
- [x] Cross-project verification and final documentation.

## Completed changes

| Finding | Resolution |
| --- | --- |
| A1: instruction errors reported as internal failures | Catch legacy instruction-parser errors at the parsing boundary and return `invalid_input`. Tests cover unknown gates and incorrect qubit arities in validation and execution. |
| A2: checkpoint diagnostic mentions fidelity | Supply operation context to unitary validation; checkpoint prefix errors now identify `checkpoint_batch prefix`. Reset and Kraus prefixes are tested. |
| A3: sequential checkpoint random streams | Document the existing contract: a seed reproduces the complete ordered request, while earlier branches can affect later random draws. This is not evidence of biased sampling. Added seeded repeatability and count-total checks with noise and readout. |
| A4: delays omitted from the inverse | No mathematical defect was confirmed for the supported fidelity semantics: the inverse is ideal, coherent noise acts on the forward circuit, and idle channels are prohibited in these operations. Documented those semantics and tested delay equivalence and ideal mirror identity. |
| B1: BrokenPipe masks native error | Allow bounded output draining after early input closure and preserve a complete structured native failure. Cancellation, malformed/duplicate frames and cleanup failures retain precedence. An apparent success cannot override an incomplete request upload. |
| B2: diagnostics without final newline | Accept ordinary diagnostic tails, including a one-character tail. Reject incomplete reserved protocol frames even when an earlier result exists. |
| B3: nested cleanup deadlines | Keep the intentional 20-second launcher deadline and 25-second supervisor deadline; derive the outer deadline from the inner deadline plus five seconds. Document the cleanup margin. |
| C1: generic QRMI exceptions | Map unsupported capabilities/operations to `UnsupportedFunction`, and invalid configuration or missing sessions to `InvalidConfig`. A missing session requires reacquisition; it does not mean the resource is absent. Rust and Python mappings are tested. |
| C2: runner failures hide diagnostics and return success | Both runners fetch logs before cleanup, send diagnostics to stderr, and return nonzero for task failure, cancellation and result errors. Tests cover failed log retrieval and cleanup after errors, plus actual CLI exit status against the server. |
| C3: payload/schema mismatch | Align the schema and runner helpers for bare native documents, wrapped native object/string requests, and `job_type=request` object/string inputs. Preserve required legacy fields, positive qubit counts and string circuit input. |
| C4: repeated capability negotiation | Cache successful protocol negotiation in the socket client, share it across clones and retain the client in the QRMI resource. Explicit discovery stays fresh. Transport/protocol/version failures invalidate negotiation; submissions are never automatically replayed. Tests count actual socket exchanges. |

Additional confirmed gaps are closed:

- Maestro rejects operation-specific and instruction-specific fields that would
  otherwise be ignored. `keep_qubits` requires a requested partial trace.
- Native request validation runs in an isolated Rust worker before task
  allocation. Tests cover valid and invalid input, abnormal worker exit,
  truncated replies and subsequent successful validation. Capability discovery
  still calls the native library inside the daemon; documentation identifies
  that remaining boundary explicitly.
- Documentation describes bounded serialized transport accurately; the path is
  not zero-copy. Ordinary validation/execution reexecutes the Rust server worker;
  MPI profiles launch the separate native C++ executable.

The C4 decision followed a local debug-build measurement: 30 explicit capability
queries had a median latency of 10.185 ms and a 5,911-byte response; 10 submissions
with fresh negotiation had a median latency of 46.867 ms. These are local baseline
measurements, not a general performance guarantee. Regression tests verify the
removed round trip; no post-change speedup is claimed.

## Verification results

All results below are from the final implementation. These are focused suites for
the affected paths, not a claim that every test in all three repositories ran.

| Project / suite | Result |
| --- | --- |
| Maestro native request C ABI suite | 240 checks passed. |
| Maestro additional CTest smoke tests | 4 passed: native CLI, two-rank CPU MPI success, MPI failure, and local GPU execution. |
| Local server | 38 unit tests and 1 production-worker integration test passed; includes all 11 MPI supervision tests. |
| QRMI Maestro adapter | 17 tests passed. |
| Bundled Rust Maestro request client | 4 tests passed; 1 existing test requiring the live `/run/maestro.sock` service was ignored. |
| Rust task runner | 4 tests passed. |
| Python helpers, schema, runners, Python/C bindings and native integration | 33 tests passed, with no skips; optional C, MPI and local distributed-GPU coverage enabled. |
| Formatting and patch checks | Touched Rust and Python files checked; `git diff --check` passed in all three repositories. |

The integration run used newly built Maestro, server, QRMI extension and Rust
runner artifacts under `/tmp/maestro-capabilities` and
`/tmp/maestro-review-fixes`. GPU checks loaded the existing distributed-GPU plugin.
Dynamic dependency inspection found no `libpython` dependency in the Maestro
library, either native executable, or the server. Production server execution and
validation use Rust/C/C++ only.

Hardware and deployment limits remain explicit:

- GPU coverage used two logical shards on one physical RTX 5090. Physical
  multi-GPU and multi-node execution still require site verification.
- MPI coverage used two CPU ranks. CUDA-aware MPI was unavailable, so MPI GPU
  execution was not verified here.
- Slurm cancellation failure was covered with fake launchers; no real Slurm
  allocation or scheduler job was launched.
- Installed project libraries and services were not changed or restarted. The
  user-supplied preliminary review and unrelated Maestro changes were preserved.
