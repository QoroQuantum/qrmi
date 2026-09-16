# Maestro capability implementation plan

Started: 2026-09-16. Implements the computational exposure described in [the capability report](maestro-qrmi-capability-report.md).

## Scope and constraints

- Changes are confined to Maestro, maestro-local-server, and QRMI (including its bundled Maestro socket client).
- The daemon, ordinary workers, and MPI workers use native Rust/C/C++. No embedded Python or Python worker processes.
- Preserve existing payload layouts, legacy commands, and execute/estimate behavior. Add a versioned JSON request through the existing Maestro payload using `job_type="request"`.
- Expose supported configuration, noise, local distributed GPU execution, MPI execution, and additional computational operations through ordinary tasks. Use batches for stateful evolution/checkpoint workflows.
- Reject unsupported combinations explicitly; do not promise new distributed density-matrix/MPO engines, a general pure-state Kraus trajectory engine, Composer network orchestration, or a native Sinter/decoder implementation. Interactive state shared across separate tasks and external artifact storage remain separate API projects; bounded batch/query results cover the current computational exposure.
- Preserve the pre-existing Maestro `build.sh` modification and `validation_scratch/` directory.

## Sequence

1. **Native contract and shared configuration (Maestro).** Extract reusable C++ configuration from bindings. Add versioned request, validation, capabilities, errors, buffer ownership, stable backend/method names, explicit fixed selection, complete option handling, parameterized circuit parsing, and output bounds. Keep Python bindings optional.
2. **Native computational coverage (Maestro).** Expose ideal/noisy execute and estimate, exact versus sampled noise, all supported noise model settings, probabilities/amplitudes/statevector, state probability, overlap/mirror, mixed-state diagnostics, evolution, and checkpoint batches. Correct relevant existing helper/formatting issues and add focused numerical/contract tests.
3. **Lossless server API (local server).** Add atomic JSON submission and capabilities, retain legacy commands, forward native requests through isolated workers, retain structured errors/logs, bound transport sizes, and make session leases configurable. Make native header/library paths configurable for reproducible builds.
4. **Native MPI orchestration (Maestro + local server).** Add an optional C++ MPI worker and server-controlled launcher profiles. Broadcast identical requests, return one result, synchronize failure/lifecycle handling, finalize in order, and cancel the entire launcher group/job step. Keep local multi-GPU execution in an ordinary worker.
5. **QRMI integration (QRMI + bundled client).** Negotiate capabilities, submit versioned requests without changing the existing C payload layout, implement target/log retrieval, retain result metadata/errors, update both task runners, and supply examples/documentation for configuration, noise, and distribution.
6. **Integration and review (all three).** Build without Python in the server path, run focused native/server/client/adapter tests and isolated end-to-end tasks, exercise legacy compatibility and malformed requests, inspect dependencies, and record actual GPU/MPI validation limits. Update this file with completed work and evidence.

## Verification principles

- Numerical tests use small independently predictable circuits, asymmetric bit layouts, exact mixed-state channels, reproducible seeds, and batch-versus-independent equivalence.
- Protocol tests cover multiline/commented QASM, escaped content, unsupported versions/options, framing limits, atomic failure, diagnostics, cancellation, and old-server negotiation.
- GPU/MPI tests distinguish hardware execution from capability/negative-path tests. Run physical-device tests only when the required plugins, licenses, and allocation are available; record missing prerequisites explicitly.
- Use isolated sockets/build locations. Do not replace installed libraries, restart the system service, or submit external scheduler jobs as part of ordinary verification.

## Progress

- [x] Source analysis and capability report.
- [x] Implementation plan recorded.
- [x] Native contract and shared configuration.
- [x] Native computational and noise coverage.
- [x] Lossless server API and diagnostics.
- [x] Native MPI worker and launch integration.
- [x] QRMI client/adapter/runners/examples.
- [x] Focused tests, end-to-end verification, and final review.

## Implemented changes

Completed in the working trees of the three planned repositories. No additional
project was modified, no changes were committed, and no installed library/service
was replaced. Maestro's pre-existing `build.sh` change and `validation_scratch/`
were preserved.

### Maestro

- Extracted native configuration and noise models from the Python binding layer;
  both bindings and the new C ABI reuse these implementations.
- Added schema-2 capability, validation, execution, and MPI-finalization functions
  with explicit ownership, structured errors, symbolic backend selection,
  configuration validation, accurate threshold serialization, and output bounds.
- Added native noise, state/query, diagnostic, overlap/fidelity, parameter binding,
  incremental evolution, checkpoint suffix and independent batch operations.
  Explicit channel/reset/delay instructions support exact noisy evolution.
- Added ordinary/MPI C++ request executables and opt-in native, MPI and GPU tests.
  Fixed the distributed noise helper's incorrect repeated-execution call and
  exposed the existing conventional plugin's host-staged transfer flag.
- Fixed selections verify the actual backend/method/device. Native requests reject
  unavailable selections rather than returning substituted computations.

Native reference: `/home/adrian/maestro/docs/native-request-api.md`.

### Local server

- Added lossless versioned commands for capabilities, atomic submission, logs,
  and keepalive alongside the legacy protocol.
- Native JSON tasks run in isolated native workers. Structured failures and a
  bounded log prefix are retained, including after result consumption; session
  cleanup removes retained diagnostics.
- Added frame bounds, configurable session lease/socket/build paths, and native
  MPI launch profiles controlled by the administrator. Launch arguments do not
  pass through a shell. MPI input/output and cancellation use nonblocking I/O;
  cleanup failures are reported instead of acknowledged as successful.
- MPI native failures remain structured after the launcher/supervisor exits.
  Nested launch directives and malformed profile settings are rejected.

Server reference: `/home/adrian/slurm/maestro-local-server/docs/native-api-v2.md`.

### QRMI and bundled client

- Added capability negotiation, native JSON submission, target discovery and
  structured logs without changing the existing Maestro payload or C ABI layout.
- Added client-side Python serialization helpers and updated both task runners,
  payload schema, migration guidance, and four runnable request examples.
- Fixed a polling race in which failure could become visible between the legacy
  FAILED and FINISHED queries and be cached as Completed.
- Added Rust, Python, C binding, mock transport and opt-in native integration tests.

User guide: [maestro-native-api.rst](maestro-native-api.rst).

## Verification performed

All builds and daemons used isolated paths under `/tmp/maestro-capabilities`.
No external scheduler jobs were submitted. CPU/native tests use the local QCSim
build with Qiskit Aer disabled. Tests requiring Unix sockets, MPI or CUDA were
run outside the filesystem/network sandbox after tool approval.

| Area | Result |
|---|---|
| Maestro C ABI numerical/contract suite | Passed; 147 checks, including exact T1/Pauli/Kraus behavior, sampled-noise convergence and repeatability, asymmetric bit ordering, register sizes, partial trace, instructions, parameter binding, selection, batch/evolution/checkpoint/fidelity, direct C header/ownership, and invalid requests |
| Native CLI/MPI/GPU CTest | All five tests passed: C ABI, ordinary CLI, two-rank CPU MPI success, collective failure, and local GPU distribution with shared-device/host-staging flags |
| Optional Maestro Python bindings | Rebuilt successfully after extraction; 301 CPU binding/circuit tests passed, 9 skipped, 8 GPU tests deselected |
| Local server | Full suite passed (31 unit tests plus production worker test at that checkpoint); after the MPI error addition, all 30 Maestro-module tests passed; final six launcher/profile tests passed |
| QRMI Maestro Rust adapter | 15 tests passed, including the completion/failure race regression |
| Bundled Maestro Rust client | 3 passed; 1 pre-existing live-daemon test ignored |
| Rust task runner | Native/wrapped/legacy input test passed |
| QRMI Python/C binding and native integration suite | 18 passed, including five full native integration tests with MPI success/failure and real local GPU distribution |
| Noisy GPU request through QRMI | Additional GPU integration run passed after adding the deterministic noise assertion |
| Request examples | All four documents passed native validation |
| Native runtime dependencies | `libmaestro`, both C++ executables, and the Rust daemon resolved their dependencies without `libpython` |
| Working-tree checks | `git diff --check` passed in all three repositories |

The final native test count includes the batch-level option rejection regression;
individual successful test groups were not repeatedly rerun unless their code or
coverage changed. The opt-in integration tests start and stop private native
daemons and use the Python extension only as a client/test harness.

## Runtime findings and remaining validation limits

1. **GPU hardware:** this machine exposes one RTX 5090. Local distribution was
   exercised with two logical shards on that GPU, including forced host staging
   and sampled Pauli noise. This verifies execution/correctness, not physical
   multi-GPU capacity, performance, or scaling.
2. **MPI:** the native two-rank CPU worker and full QRMI launch path passed. The
   installed MPI rejects the GPU path with `invalid MPI topology/device or MPI
   lacks CUDA support`. `ompi_info` also reports the read-only build flag
   `mpi_built_with_cuda_support:false`. CUDA-aware MPI, physical multi-GPU, multi-node execution,
   and a real Slurm allocation/job-step cancellation still require site testing.
   Launcher validation, bounded I/O, rank failure, and cancellation are covered by
   native/local process tests.
3. **Local runtime configuration:** hwloc's graphical probing hung on unavailable X
   displays; the isolated MPI profile used `HWLOC_COMPONENTS=-gl,-opencl`. The WSL
   CUDA test initially loaded an incompatible system PTX JIT library; selecting
   the matching WSL driver directory and CUDA library directory for the test
   resolved that crash. Neither setting was applied globally.
4. **Optional Python GPU regressions:** the initial unfiltered legacy binding suite
   aborted in its GPU fallback test under the incomplete GPU environment. CPU
   regressions subsequently passed, and GPU coverage for the new path was run
   through the native CLI/server/QRMI with the corrected runtime environment.
5. **Scope boundaries:** the exclusions listed at the start remain deliberate.
   Exact noisy evolution uses mixed-state methods; a general pure-state Kraus
   trajectory engine and distributed mixed-state engines were not implemented.
   Checkpoint noise applies to suffixes after an ideal prefix. Composer/Sinter,
   cross-task state handles and an artifact service remain separate work.

## Reproducing the checks

Native build options:

```sh
cmake -S /home/adrian/maestro -B BUILD \
  -DBUILD_PYTHON_BINDINGS=OFF -DCOMPILE_TESTS=OFF \
  -DMAESTRO_BUILD_REQUEST_TESTS=ON -DMAESTRO_BUILD_MPI_WORKER=ON
cmake --build BUILD --target maestro_request_tests maestro_request_worker maestro_mpi_worker
ctest --test-dir BUILD --output-on-failure
```

Supply the project's QCSim/Eigen/Boost dependency paths as appropriate. Enable
`MAESTRO_RUN_GPU_REQUEST_TESTS` only with a working licensed GPU plugin/runtime.
This session used the existing dependencies under `/home/adrian/maestro/build`,
the existing distributed GPU plugin build, and explicitly selected the new native
library with `LD_LIBRARY_PATH`.

Server:

```sh
MAESTRO_HEADER=/home/adrian/maestro/maestrolib/Interface.h \
MAESTRO_LIBRARY_DIR=BUILD CARGO_TARGET_DIR=SERVER_BUILD cargo test --offline
```

Set the matching runtime library search path, run from the server repository,
and allow temporary Unix sockets. QRMI unit and opt-in integration commands are
listed in [the user guide](maestro-native-api.rst). Set
`QRMI_TEST_DISTRIBUTED_GPU=1` for the real local GPU integration test;
`QRMI_TEST_MPI_PROFILE` names the administrator-owned test launch profile.
