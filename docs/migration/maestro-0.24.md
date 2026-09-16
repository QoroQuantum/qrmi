# Maestro Local with QRMI 0.24

The upstream merge registers `maestro-local` with the shared resource factory
and `QRMIService` in Rust, Python and C. A static resource definition is sufficient;
Maestro does not implement the dynamic `ResourceProvider` interface.

## Discovery and sessions

Set `QRMI_JOB_QPU_RESOURCES` to the backend name and `QRMI_JOB_QPU_TYPES` to
`maestro-local`. For example:

```python
import os
from qrmi import QRMIService

os.environ["QRMI_JOB_QPU_RESOURCES"] = "MAESTRO_LOCAL"
os.environ["QRMI_JOB_QPU_TYPES"] = "maestro-local"
service = QRMIService()
resource = service.resource("MAESTRO_LOCAL")
if resource is None:
    raise RuntimeError("Maestro Local is unavailable")
token = resource.acquire()
try:
    # Submit Payload.MaestroLocal tasks using this resource.
    print(resource.resource_id())
finally:
    resource.release(token)
```

The service checks accessibility and retains each accessible resource instance.
Direct construction with `QuantumResource(name, ResourceType.MaestroLocal)` is
also supported.

`<backend_name>_QRMI_JOB_ACQUISITION_TOKEN` optionally supplies an existing session.
Set it **before constructing** the resource. Invalid tokens produce a configuration
error. `acquire()` retains the session in the instance; no environment update is
needed to submit tasks through that instance. Export the token when sharing the
session with another process, such as the Python task runner. A stale session is
replaced by `acquire()` only when the server confirms that it does not exist;
communication failures retain their error reason.

`release(token)` deletes the explicitly named session. Successfully releasing the
current session clears it from the resource; an old environment value does not
reactivate it. Call `acquire()` again before submitting further work.

Maestro connects to `/run/maestro.sock` by default. The optional process-wide
`QRMI_MAESTRO_SOCKET` setting selects a different socket, including a temporary
mock server for tests. Keep the socket setting unchanged while resources are in use.

## Errors and task lifecycle

Maestro uses QRMI's shared error categories:

| Condition | Rust | Python | C return code |
| --- | --- | --- | --- |
| Malformed acquisition token | `ParseError` | `ConfigError` | 103 |
| No acquired/configured session | `InvalidConfig` | `ConfigError` | 114 |
| Invalid task/session argument or locally rejected task input | `InvalidInput` | `InvalidInputError` | 109 |
| Unsupported payload variant | `UnsupportedPayload` | `UnsupportedPayloadError` | 106 |
| Task does not exist and no terminal state is known | `TaskNotFound` | `TaskNotFoundError` | 112 |
| Result requested for an active, failed or cancelled task | `TaskNotReady` | `TaskNotReadyError` | 107 |
| Unavailable logs or target data | `UnsupportedFunction` | `UnsupportedFunctionError` | 105 |

Invalid JSON options are also classified as invalid input. Unclassified server
and socket failures remain `Other` / `QrmiError` / code 100 with a diagnostic
message. These should not be assumed to be authentication or input errors.

With native API v2, `target()` returns capabilities and `task_logs()` returns
structured diagnostics. Older servers return `UnsupportedFunction` for unsupported
API negotiation. See [the native request guide](../maestro-native-api.md). Session
acquisition and release retain their existing behavior.

`task_stop()` skips tasks already known to be terminal. Cancellation requires a
positive server acknowledgement. A negative acknowledgement or error is accepted
as a benign race only if a subsequent status check confirms a terminal state;
refusal while the task remains active is reported as an error.

Retrieving results removes the task on the Maestro server. The same QRMI resource
remembers observed completed/failed states and acknowledged cancellations, so a
completed task does not become cancelled after results are consumed. Retrieving
those results again reports `TaskNotFound`. A different resource instance cannot
infer the terminal status of an already removed task and reports `TaskNotFound`.

## Rebuild after the merge

A `git pull` does not update compiled extensions or plugins. Rebuild the Python
wheel/extension and the C library, then rebuild C/Lua consumers and the Slurm plugin
against the matching generated `qrmi.h`.

The inserted `IBMQuantumComputeService` enum variant shifts Maestro's C resource
type from 6 to 7 (6 now denotes IQM). Do not mix the previous header or consumer
binaries with the new library. Use symbolic enum names in source code.

From the QRMI repository root, with `maturin` available:

```sh
maturin build --locked --release --interpreter python3.11 --out target/wheels
# Install the resulting wheel in the Python environment that runs QRMI.
cargo build --locked --release --lib
cmake -S examples/qrmi/c/maestro_local -B target/maestro-c -DCMAKE_BUILD_TYPE=Release
cmake --build target/maestro-c
cmake -S lua -B target/lua -DQRMI_INCLUDE_DIR="$PWD" -DQRMI_LIBRARY="$PWD/target/release/libqrmi.so"
cmake --build target/lua
```

Building the C library after the wheel ensures that `target/release/libqrmi.so`
is the standalone C library, without Python extension dependencies. The Lua build
requires development headers for the Lua version used at runtime. Rebuild the
SPANK plugin with its `QRMI_ROOT` pointing at this checkout. Deployment locations
and active Slurm jobs should be handled by the normal cluster deployment process.

## Regression tests

The Maestro QRMI tests use a scripted Unix socket and do not need a running
Maestro daemon. They cover session reuse/replacement/release, service discovery,
classified errors, rejected submissions, cancellation races and consumed results.

```sh
cargo test --locked --lib maestro
cargo test --locked -p maestro-local-api --lib
PYTHONPATH=python QRMI_TEST_LIBRARY="$PWD/target/release/libqrmi.so" \
    python -m pytest python/tests/unit/quantum_resource
```

Run the Python tests with the rebuilt extension. `QRMI_TEST_LIBRARY` enables the
C binding checks; those tests are skipped when it is unset. The test environment
must allow binding temporary Unix sockets. Existing client integration tests
under `dependencies/maestro_rust_library/tests` still require a real Maestro daemon.

### Live Maestro tests

With a running local server and the rebuilt Python extension:

```sh
PYTHONPATH=python QRMI_TEST_MAESTRO_LIVE=1 \
    python -m pytest python/tests/integration/quantum_resource/test_maestro_live.py
cargo test --locked -p maestro-local-api --test session_test -- --test-threads=1
```

The opt-in Python suite checks two-qubit execution and Bell-state expectation
values on Aer/statevector and QCSim/matrix-product-state, cancellation of an
unsubmitted task, status after result consumption, discovery and session
release/replacement. Each test allocates fresh sessions, limits completion
polling to 60 seconds, and deletes and verifies removal of its sessions during
cleanup. It is skipped by default, so ordinary Python test runs do not submit
live jobs. Use `QRMI_MAESTRO_SOCKET` if the server uses a different socket path.
