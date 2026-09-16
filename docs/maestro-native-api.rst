.. _maestro_native_api:

Maestro native requests through QRMI
====================================

Maestro Local now supports a versioned native request through the existing
``Payload.MaestroLocal`` layout. Set ``job_type="request"`` and put the complete
schema-2 document in ``input``. Legacy execute/estimate payloads still work. Rebuild
Maestro, maestro-local-server, and QRMI together to use the new path. Older servers
produce an explicit unsupported-API error during capability negotiation.

The server uses only Rust/C/C++; Python is an optional client interface.

See the :download:`review fix plan and results <maestro-review-fix-plan.md>` for the completed
cross-project fixes, regression coverage and hardware verification limits. The :download:`second review plan and results <maestro-review-round2-plan.md>` records the subsequent
cancellation, transport, payload and test-coverage changes. The :download:`third review plan and results <maestro-review-round3-plan.md>` covers composed failure diagnostics,
operation-specific shots, Python input errors and examples.

Python client
-------------

.. code:: python

   import json
   import time
   from qrmi import QuantumResource, ResourceType, TaskStatus
   from qrmi.maestro import request_payload

   resource = QuantumResource("MAESTRO_LOCAL", ResourceType.MaestroLocal)
   print(json.loads(resource.target().value))
   token = resource.acquire()
   try:
       document = {
           "schema_version": 2,
           "operation": "estimate",
           "circuit": {
               "num_qubits": 1,
               "source": "OPENQASM 2.0;\nqreg q[1]; h q[0];",
           },
           "simulator": {"backend": "qcsim", "method": "density_matrix"},
           "observables": ["X", "Z"],
           "execution": {"seed": 123},
           "noise": {
               "evaluation": "exact",
               "channels": [{"kind": "t1", "targets": [0], "gamma": 0.36}],
           },
       }
       task = resource.task_start(request_payload(document))
       deadline = time.monotonic() + 60
       while True:
           status = resource.task_status(task)
           if status in (TaskStatus.Completed, TaskStatus.Failed):
               break
           if time.monotonic() >= deadline:
               resource.task_stop(task)
               raise TimeoutError("Maestro task exceeded 60 seconds")
           time.sleep(0.1)
       if status == TaskStatus.Failed:
           raise RuntimeError(resource.task_logs(task))
       result = json.loads(resource.task_result(task).value)
       print(result["expectation_values"])  # [0.8, 0.36]
       print(resource.task_logs(task))
   finally:
       resource.release(token)

``target().value`` contains the complete public/native capability document.
``task_logs()`` returns serialized JSON containing captured stdout/stderr, the native
error, and a failure message. Results retain native execution metadata, seeds, noise
approximations, timing, and distribution details. Result retrieval still consumes the
result; logs remain available until session cleanup. Observe failed status before
requesting a result; QRMI raises ``TaskNotReady`` for failed tasks. If execution and
cleanup both fail, ``error.code`` is ``cleanup_unconfirmed``, with
``cleanup_confirmed:false`` and a bounded ``causes`` array retaining earlier error
codes/messages. Causes may be nested; inspect them recursively for the original native
failure. An active cancellation still fails if cleanup cannot be confirmed. Submission
errors retain typed QRMI categories: unsupported capabilities map to
``UnsupportedFunction`` / Python ``UnsupportedFunctionError``; invalid configuration and
missing/expired sessions map to ``InvalidConfig`` / Python ``ConfigError``. A missing
session calls for acquiring a new session, not choosing a new backend. Malformed
launch/rank input maps to ``InvalidInputError``; a broken profile file or
missing/nonexecutable launcher, worker or Slurm cancellation tool maps to
``ConfigError``. Selecting an unknown profile maps to ``UnsupportedFunctionError``.

The server advertises ``server.session_lease_seconds`` (default 3600). Poll a running
task or retrieve its logs comfortably within that interval. Native submit and logs also
renew the lease. Explicit ``keepalive`` is available on the standalone bundled Rust
socket client, not through QRMI's ``QuantumResource`` interface or Python/C/Lua resource
bindings. QRMI does not start a background heartbeat. Cached terminal status and session
existence checks do not renew it; logs can renew it after result consumption. A client
that stops all task traffic can lose its session, and expiry cancels active work.

Rust, C, and Lua
----------------

The existing payload fields and C ABI layout are unchanged:

.. code:: rust

   let payload = Payload::MaestroLocal {
       input: serde_json::to_string(&document)?,
       job_type: "request".into(),
       qubits: 0,
       simulator_type: 0,
       simulation_method: 0,
       observables: String::new(),
       config: "{}".into(),
   };

C and Lua use the same existing Maestro payload construction with ``job_type`` set to
``request``, ``input`` set to serialized JSON, and unused legacy fields set to the
values above. Backend/method selection is inside the document and uses symbolic names,
avoiding build-dependent legacy enum numbers. There is no new payload enum or Slurm
plugin ABI requirement for this feature. Rebuild QRMI itself; existing consumers must
still match the repository's existing QRMI ABI version.

The bundled Rust socket client exposes ``maestro_local_api::maestro::request::Client``
with ``capabilities``, ``submit``, ``logs``, and ``keepalive``. QRMI uses
capabilities/submit/logs and moves their blocking exchanges off its asynchronous
executor; applications using the socket client directly can also call
``keepalive(session_id)``. Successful schema negotiation is shared across client clones
and retained by the QRMI resource. Transport, malformed-response and
incompatible-version failures invalidate it; failed submissions are never automatically
retried. Explicit capability queries always fetch the current catalog. This is not
feature-by-feature preflight validation: Maestro and the server validate every submitted
request. The resource captures one socket path at construction for native requests and
all legacy session/task operations, including acquisition, status, results and cleanup.
``QRMI_MAESTRO_SOCKET`` chooses that socket; the default remains ``/run/maestro.sock``.
Construct a new resource after changing the variable to use a different daemon.

Task runners and examples
-------------------------

Both Rust and Python task runners accept a raw schema-2 request, ``{ "request": ... }``,
or the existing payload wrapper with ``job_type:"request"``. They also retain the legacy
execute/estimate form and accept a configuration object or serialized string. An omitted
or null legacy ``config`` becomes ``{}`` in both runners. Job types are
case-insensitive. A ``{ "request": ... }`` wrapper must contain only that field. Bare
native documents reject legacy envelope fields, and every Maestro form rejects fields
belonging to another backend. The explicitly selected ``job_type:"request"``
compatibility envelope accepts object or serialized JSON ``input`` and permits the known
legacy fields (``qubits``, simulator IDs, ``observables``, ``config``) outside it; those
outer fields are ignored. All native configuration belongs inside ``input``. The two
runners and schema apply these same rules to each form. Legacy jobs require positive
``qubits`` and nonnegative ``simulator_type`` and ``simulation_method``, all within
uint32 range. These forms are covered by ``qrmi_payload_v1_schema.json``. Missing
required Python helper fields raise descriptive ``ValueError``\ s before task
submission. For schema-2 requests, put ``execution.shots`` only on ``execute`` or
``checkpoint_batch``; estimators and other queries reject it. ``execution.seed`` remains
supported. To validate a sampling request, retain its operation and use the native
validation entry point rather than changing it to ``validate``.

Both runners retrieve available diagnostics before cleanup on failure/cancellation,
write those diagnostics to stderr, and exit nonzero. Result retrieval and output writing
failures also produce nonzero exits. Missing log support does not hide the original
failure. The Python runner retrieves results before returning, so errors are reflected
in its exit code rather than deferred to an exit callback.

The ready-to-run documents in
:doc:`Maestro task runner examples <examples/task_runner/TASK_RUNNER_MAESTRO>`
include:

-  ``native-noise.json``: exact amplitude damping and observable estimation on CPU.
-  ``native-query.json``: a batch of state and mixed-state queries.
-  ``native-thermal-idle.json``: exact thermal relaxation, elapsed idle time and
   detuning.
-  ``native-correlated-noise.json``: seeded OU phase noise and MPS options.
-  ``native-checkpoint.json``: restore an ideal prefix and execute noisy suffixes.
-  ``native-incremental.json``: repeated exact amplitude-damping Kraus steps.
-  ``native-distributed-gpu.json``: local distribution across two physical GPU ordinals.
-  ``native-mpi-gpu.json``: MPI distribution through an administrator-defined profile.

The GPU documents require the named resources and installed plugins. Set the backend
acquisition token as for existing runner tasks:

.. code:: sh

   export QRMI_JOB_QPU_RESOURCES=MAESTRO_LOCAL
   export QRMI_JOB_QPU_TYPES=maestro-local
   export MAESTRO_LOCAL_QRMI_JOB_ACQUISITION_TOKEN=<existing-session-id>
   task_runner MAESTRO_LOCAL examples/task_runner/maestro_local/native-noise.json

Supported computation and limits
--------------------------------

Native operations include execution, expectation estimation, selected/full amplitudes
and probabilities, state probability, unitary overlap/mirror fidelity, coherent noisy
fidelity, mixed-state diagnostics/partial trace/maintenance, incremental evolution,
checkpoint suffix batches, and independent request batches. All share the native
configuration and declarative noise model. Incremental exact noise uses explicit Kraus
instructions; checkpoint noise applies to suffixes after an ideal prefix. Checkpoint
randomness is reproducible for the complete ordered request: changing/reordering earlier
suffixes may change later samples. Fidelity queries use ideal inverse preparation;
coherent noisy fidelity is not a physical mirror experiment with noise on both halves.
Runtime support can be narrower for a particular method. Operation-specific fields on
other operations are rejected. Native request validation and numerical execution each
run in isolated Rust workers calling Maestro's C ABI; capability catalog queries remain
in the daemon. An outer ``launch`` object is server routing metadata. The server
validates it and removes it before native validation/execution. Direct Maestro C ABI and
``maestro-request`` calls reject ``launch``; use the server for profile selection, or
launch ``maestro-mpi-worker`` yourself with a computational document.

The native API defaults to fixed backend selection and rejects unavailable selections.
Automatic ideal execute/estimate can use explicit candidate lists. Exact quantum
channels require density-matrix/MPO methods; pure-state sampled noise follows Maestro's
existing helper approximations. Distributed GPU engines currently support statevectors.
Noise results identify approximations; an MPO result is not automatically free of
truncation error.

Counts use classical bit 0 first. Pauli strings and target-state strings use qubit 0
first. Integer amplitude/probability indices use qubit 0 as the least significant bit.
Complex entries are ``[real, imaginary]``.

Requests are bounded to 16 MiB and serialized results to 64 MiB. Use selected
``basis_states`` and ``max_output_elements`` for state queries. Batches have at most 256
members and four nested levels. No interactive state persists across tasks. Composer
orchestration, decoder/Sinter execution, general pure-state Kraus trajectories, new
distributed mixed-state engines, and artifact storage remain outside this change.

Authoritative native field/option/channel details are in Maestro's
``docs/native-request-api.md``; server framing, leases and MPI profile configuration are
in maestro-local-server's ``docs/native-api-v2.md``. The original investigation is
preserved in :download:`the capability report <maestro-qrmi-capability-report.md>`, and
implementation/test evidence is in :download:`the plan <maestro-implementation-plan.md>`.

Tests
-----

.. code:: sh

   cargo test --locked --lib maestro::
   cargo test --locked -p maestro-local-api --lib
   cargo test --locked --features build-binary --bin task_runner
   python -m pytest python/tests/unit/quantum_resource/test_maestro_request.py \
       python/tests/unit/quantum_resource/test_maestro_local.py \
       python/tests/unit/quantum_resource/test_maestro_runner.py

Use the rebuilt Python extension. Set ``QRMI_TEST_LIBRARY`` to a matching QRMI shared
library to enable C ABI checks. For end-to-end tests, set ``QRMI_TEST_MAESTRO_SERVER``
to a freshly built native daemon, select its matching Maestro library with
``LD_LIBRARY_PATH``, and run ``test_maestro_native_integration.py`` in that directory.
Tests start a private socket and daemon. Set ``QRMI_TEST_MAESTRO_REQUEST`` to the
matching ``maestro-request`` executable to validate all native example documents. CPU
examples are also executed through QRMI and checked numerically; hardware examples
require separate execution resources. For CPU MPI tests, ``QRMI_TEST_MPI_PROFILE`` names
a configured two-rank profile; set ``MAESTRO_MPI_PROFILES`` to its profile file for that
daemon. Set ``QRMI_TEST_DISTRIBUTED_GPU=1`` with a working licensed plugin/runtime to
include ideal and noisy local GPU distribution through QRMI. No system service is
restarted. Set ``QRMI_TEST_MPI_GPU_PROFILE`` to a profile with working rank-local GPU
binding, the MPI GPU plugin/license and CUDA-aware MPI to run ideal and noisy
``distributed_mpi_gpu`` cases. Those tests assert the actual backend and rank count; the
CPU MPI test alone does not verify GPU distribution. The live legacy suite in
``python/tests/integration/quantum_resource/test_maestro_live.py`` requires
``QRMI_TEST_MAESTRO_LIVE=1`` and skips backend cases absent from the capability catalog.
