"""Opt-in QRMI -> isolated native daemon -> C++ feature tests.

Set QRMI_TEST_MAESTRO_SERVER to the rebuilt daemon, and its native library search
path in LD_LIBRARY_PATH. The test harness starts no Python server or worker.
"""

import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time

import pytest

from qrmi import (
    QuantumResource,
    ResourceType,
    TaskStatus,
    InvalidInputError,
    ConfigError,
    UnsupportedFunctionError,
)
from qrmi.maestro import request_payload

EXAMPLES = Path(__file__).resolve().parents[4] / "examples/task_runner/maestro_local"


@pytest.mark.parametrize(
    "path", sorted(EXAMPLES.glob("native-*.json")), ids=lambda p: p.stem
)
def test_example_native_validation(path):
    """Validate every example's computational document with the native CLI."""
    worker = os.environ.get("QRMI_TEST_MAESTRO_REQUEST")
    if not worker:
        pytest.skip(
            "set QRMI_TEST_MAESTRO_REQUEST to the native maestro-request executable"
        )
    document = json.loads(path.read_text())
    # Server profile selection is tested separately; the C ABI accepts only the
    # computational document, including for hardware-dependent examples.
    document.pop("launch", None)
    result = subprocess.run(
        [worker, "--validate", "--framed-result"],
        input=json.dumps(document),
        text=True,
        capture_output=True,
        timeout=15,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    frames = [
        line.removeprefix("MAESTRO_RESULT_V2 ")
        for line in result.stdout.splitlines()
        if line.startswith("MAESTRO_RESULT_V2 ")
    ]
    assert len(frames) == 1
    assert json.loads(frames[0])["ok"] is True


@pytest.mark.parametrize(
    "name",
    ["noise", "query", "thermal-idle", "correlated-noise", "checkpoint", "incremental"],
)
def test_cpu_examples_through_qrmi(native_resource, name):
    """Check numerical results from the CPU examples through the full stack."""
    document = json.loads((EXAMPLES / f"native-{name}.json").read_text())
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Completed
    result = json.loads(native_resource.task_result(task).value)
    if name == "noise":
        assert result["expectation_values"] == pytest.approx([0.8, 0.36])
    elif name == "query":
        assert result["results"][0]["amplitudes"] == [[1, 0], [0, 0]]
        assert result["results"][1]["purity"] == pytest.approx(0.8848)
    elif name == "thermal-idle":
        expected = [
            math.exp(-0.4 / 0.6) * math.cos(2 * math.pi * 0.25 * 0.3),
            0.8 * (1 - math.exp(-0.4)),
        ]
        assert result["expectation_values"] == pytest.approx(expected, abs=1e-9)
    elif name == "correlated-noise":
        assert -1 <= result["expectation_values"][0] < 0.99
        assert result["expectation_values"][1] == pytest.approx(0, abs=1e-9)
        repeated = native_resource.task_start(request_payload(document))
        assert await_task(native_resource, repeated) == TaskStatus.Completed
        assert json.loads(native_resource.task_result(repeated).value)[
            "expectation_values"
        ] == pytest.approx(result["expectation_values"], abs=1e-12)
    elif name == "checkpoint":
        assert result["noise_scope"] == "suffix_only_ideal_prefix"
        assert [suffix["counts"] for suffix in result["results"]] == [
            {"1": 32},
            {"1": 32},
        ]
    elif name == "incremental":
        assert [values[0] for values in result["expectation_values"]] == pytest.approx(
            [-1, -0.5, -0.125, 0.3671875]
        )


@pytest.fixture(name="native_resource")
def fixture_native_resource(tmp_path, monkeypatch, request):
    """Start a private native daemon and clean up its acquired session."""
    binary = os.environ.get("QRMI_TEST_MAESTRO_SERVER")
    if not binary:
        pytest.skip("set QRMI_TEST_MAESTRO_SERVER to enable native end-to-end tests")
    socket = str(tmp_path / "maestro.sock")
    monkeypatch.setenv("QRMI_MAESTRO_SOCKET", socket)
    monkeypatch.delenv("native_test_QRMI_JOB_ACQUISITION_TOKEN", raising=False)
    env = {**os.environ, "MAESTRO_SOCKET": socket}
    scenario = getattr(request, "param", None)
    if scenario in {"invalid_profiles", "missing_scancel", "double_failure"}:
        profiles = tmp_path / "profiles.json"
        profiles.write_text(
            "{"
            if scenario == "invalid_profiles"
            else json.dumps(
                {
                    "version": 1,
                    "profiles": {
                        "broken": {
                            "kind": "srun",
                            "launcher": (
                                "/bin/sh"
                                if scenario == "double_failure"
                                else "/bin/true"
                            ),
                            "worker": "/bin/true",
                            "scancel": (
                                "/bin/false"
                                if scenario == "double_failure"
                                else str(tmp_path / "missing-scancel")
                            ),
                            "args": (
                                [
                                    "-c",
                                    "exec 0<&-; echo 'MAESTRO_STEP_V2 123.4'; "
                                    'echo \'MAESTRO_RESULT_V2 {"ok":false,"error":'
                                    '{"code":"native_test_failure",'
                                    '"message":"original native cause"}}\'; '
                                    "exec sleep 30",
                                ]
                                if scenario == "double_failure"
                                else []
                            ),
                            "max_ranks": 2,
                        }
                    },
                }
            )
        )
        env["MAESTRO_MPI_PROFILES"] = str(profiles)
    if scenario == "short_lease":
        env["MAESTRO_SESSION_LEASE_SECONDS"] = "1"
    with (
        (tmp_path / "daemon.log").open("w+") as log,
        subprocess.Popen([binary], env=env, stdout=log, stderr=log) as process,
    ):
        resource = None
        token = None
        try:
            deadline = time.monotonic() + 10
            while not Path(socket).exists():
                assert process.poll() is None, "Native daemon exited at startup"
                assert time.monotonic() < deadline, "Native daemon startup timed out"
                time.sleep(0.02)
            resource = QuantumResource("native_test", ResourceType.MaestroLocal)
            token = resource.acquire()
            yield resource
        finally:
            try:
                if token is not None:
                    resource.release(token)
            finally:
                process.terminate()
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)


def native_request(operation="estimate", method="density_matrix"):
    """Build a seeded one-qubit request with operation-specific fields."""
    return {
        "schema_version": 2,
        "operation": operation,
        "circuit": {
            "num_qubits": 1,
            "source": "OPENQASM 2.0;\n// preserved comment\nqreg q[1]; creg c[1]; h q[0]; measure q->c;",
        },
        "simulator": {"backend": "qcsim", "method": method},
        **({"observables": ["X", "Z"]} if operation == "estimate" else {}),
        "execution": {
            "seed": 123,
            **({"shots": 16} if operation in {"execute", "checkpoint_batch"} else {}),
        },
    }


def await_task(resource, task):
    """Wait at most thirty seconds for native execution to finish or fail."""
    deadline = time.monotonic() + 30
    while True:
        status = resource.task_status(task)
        if status in (TaskStatus.Completed, TaskStatus.Failed):
            return status
        assert time.monotonic() < deadline, "Native task timed out"
        time.sleep(0.02)


def test_exact_noise_capabilities_and_retained_logs(native_resource):
    """Check exact noise, capabilities, and logs after consuming the result."""
    resource = native_resource
    caps = json.loads(resource.target().value)
    assert caps["native"]["python_required"] is False
    assert "checkpoint_batch" in caps["native"]["operations"]
    document = native_request()
    document["noise"] = {
        "evaluation": "exact",
        "channels": [{"kind": "t1", "targets": [0], "gamma": 0.36}],
    }
    task = resource.task_start(request_payload(document))
    assert await_task(resource, task) == TaskStatus.Completed
    result = json.loads(resource.task_result(task).value)
    assert result["expectation_values"] == pytest.approx([0.8, 0.36])
    assert result["noise"]["evaluation"] == "exact_channels"
    diagnostics = json.loads(resource.task_logs(task))
    assert diagnostics["ok"] is True
    assert diagnostics["task_id"] == int(task)
    assert diagnostics["logs_truncated"] is False
    assert diagnostics["logs_omitted_bytes"] == 0
    assert [event["state"] for event in diagnostics["events"]] == [
        "queued",
        "started",
        "completed",
    ]
    assert all(event["timestamp_unix_ms"] > 0 for event in diagnostics["events"])
    assert diagnostics["context"]["simulator"] == {
        "backend": "qcsim",
        "method": "density_matrix",
    }
    document["simulator"]["options"] = {"unknown_option": True}
    with pytest.raises(InvalidInputError):
        resource.task_start(request_payload(document))


def test_native_failure_is_retained_in_task_logs(native_resource):
    """Retain a structured error when the requested GPU ordinal is unavailable."""
    # Backend support for a query can be narrower than the schema. Use an
    # unavailable GPU ordinal for a deterministic worker-side failure instead.
    document = native_request("execute", "statevector")
    document["simulator"] = {"backend": "gpu", "options": {"gpu_device": 2147483647}}
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Failed
    diagnostics = json.loads(native_resource.task_logs(task))
    assert diagnostics["failure_message"]
    assert diagnostics["error"]["code"]


def test_mpi_launch_profile_returns_one_result(native_resource):
    """Return one numerical result and rank metadata from a two-rank launch."""
    profile = os.environ.get("QRMI_TEST_MPI_PROFILE")
    if not profile:
        pytest.skip(
            "set QRMI_TEST_MPI_PROFILE for a configured two-rank CPU smoke test"
        )
    document = native_request("estimate", "statevector")
    document["launch"] = {"profile": profile, "ranks": 2}
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Completed
    result = json.loads(native_resource.task_result(task).value)
    assert result["mpi"]["ranks"] == 2
    assert result["expectation_values"] == pytest.approx([1, 0], abs=1e-12)


def test_mpi_rank_failure_keeps_native_error(native_resource):
    """Propagate a native rank failure through the MPI supervisor."""
    profile = os.environ.get("QRMI_TEST_MPI_PROFILE")
    if not profile:
        pytest.skip("set QRMI_TEST_MPI_PROFILE to test native rank failures")
    document = native_request("execute", "statevector")
    document["simulator"] = {"backend": "gpu", "options": {"gpu_device": 2147483647}}
    document["launch"] = {"profile": profile, "ranks": 2}
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Failed
    diagnostics = json.loads(native_resource.task_logs(task))
    assert diagnostics["error"]["code"]


def test_local_gpu_distribution_through_qrmi(native_resource):
    """Execute ideal and noisy circuits using the local distributed GPU plugin."""
    if os.environ.get("QRMI_TEST_DISTRIBUTED_GPU") != "1":
        pytest.skip("set QRMI_TEST_DISTRIBUTED_GPU=1 with a licensed local GPU plugin")
    document = native_request("execute", "statevector")
    document["circuit"] = {
        "num_qubits": 2,
        "source": "OPENQASM 2.0; qreg q[2]; creg c[2]; x q[0]; measure q->c;",
    }
    document.pop("observables", None)
    document["simulator"] = {
        "backend": "distributed_gpu",
        "distribution": {
            "backend": "conventional",
            "devices": [0, 0],
            "global_qubits": [1],
            "flags": 17,
        },
    }
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Completed
    result = json.loads(native_resource.task_result(task).value)
    assert result["counts"] == {"10": 16}
    assert result["execution_metadata"]["backend"] == "distributed_gpu"

    document["noise"] = {
        "mode": "pauli",
        "evaluation": "trajectories",
        "realizations": 2,
        "channels": [{"kind": "bit_flip", "targets": [0], "probability": 1}],
    }
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Completed
    noisy_result = json.loads(native_resource.task_result(task).value)
    assert noisy_result["counts"] == {"00": 16}


def test_validation_errors_leave_daemon_usable(native_resource):
    """Classify invalid native requests and accept a valid request afterward."""
    document = native_request("execute", "statevector")
    document["circuit"] = {
        "format": "instructions",
        "num_qubits": 2,
        "source": [{"name": "cx", "qubits": [0]}],
    }
    with pytest.raises(InvalidInputError, match="number of qubits"):
        native_resource.task_start(request_payload(document))
    document = native_request("execute", "statevector")
    document["other_circuit"] = {"unknown_nested_field": True}
    with pytest.raises(InvalidInputError, match="other_circuit"):
        native_resource.task_start(request_payload(document))
    document = native_request("unknown_operation", "statevector")
    with pytest.raises(UnsupportedFunctionError, match="Unknown operation"):
        native_resource.task_start(request_payload(document))
    task = native_resource.task_start(request_payload(native_request()))
    assert await_task(native_resource, task) == TaskStatus.Completed
    assert json.loads(native_resource.task_result(task).value)[
        "expectation_values"
    ] == pytest.approx([1, 0], abs=1e-12)


@pytest.mark.parametrize(
    "operation", ["estimate", "statevector", "probabilities", "validate"]
)
def test_irrelevant_shots_do_not_allocate_tasks(native_resource, operation):
    """Reject shots on nonsampling operations without consuming a task ID."""
    document = native_request(operation, "statevector")
    document["execution"]["shots"] = 9
    with pytest.raises(
        InvalidInputError, match=f"execution.shots does not apply to {operation}"
    ):
        native_resource.task_start(request_payload(document))
    del document["execution"]["shots"]
    task = native_resource.task_start(request_payload(document))
    assert task == "0"
    assert await_task(native_resource, task) == TaskStatus.Completed


@pytest.mark.parametrize("launch", [None, {"profile": "absent", "ranks": 2}])
def test_checkpoint_suffix_launch_is_rejected(native_resource, launch):
    """Reject nested launch metadata before allocating a checkpoint task."""
    document = native_request("checkpoint_batch", "statevector")
    document["circuit"]["source"] = "OPENQASM 2.0; qreg q[1]; x q[0];"
    document["suffixes"] = [{**native_request("execute")["circuit"], "launch": launch}]
    with pytest.raises(InvalidInputError, match="Unknown field: launch"):
        native_resource.task_start(request_payload(document))
    del document["suffixes"][0]["launch"]
    task = native_resource.task_start(request_payload(document))
    assert task == "0"
    assert await_task(native_resource, task) == TaskStatus.Completed


@pytest.mark.parametrize("native_resource", ["double_failure"], indirect=True)
def test_native_and_cleanup_failures_reach_logs(native_resource):
    """Preserve the original native error when Slurm cleanup also fails."""
    document = native_request("execute", "statevector")
    document["launch"] = {"profile": "broken", "ranks": 2}
    # Valid input larger than a pipe forces the fake launcher to close mid-upload.
    document["circuit"]["source"] = (
        "//" + "x" * (1024 * 1024) + "\n" + document["circuit"]["source"]
    )
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Failed
    error = json.loads(native_resource.task_logs(task))["error"]
    assert error["code"] == "cleanup_unconfirmed"
    assert error["cleanup_confirmed"] is False
    supervisor = error["causes"][0]
    assert "scancel rejected" in supervisor["message"]
    assert supervisor["causes"][0] == {
        "code": "native_test_failure",
        "message": "original native cause",
    }


@pytest.mark.parametrize("runner", ["python", "rust"])
def test_cli_failure_exits_nonzero_and_logs_to_stderr(
    native_resource, tmp_path, runner
):
    """Make both CLIs exit unsuccessfully and write diagnostics to stderr."""
    if runner == "rust":
        binary = os.environ.get("QRMI_TEST_RUST_RUNNER")
        if not binary:
            pytest.skip("set QRMI_TEST_RUST_RUNNER to test the Rust CLI")
        command = [binary]
    else:
        command = [sys.executable, "-m", "qrmi.tools.task_runner.main"]
    document = native_request("execute", "statevector")
    document["simulator"] = {"backend": "gpu", "options": {"gpu_device": 2147483647}}
    source = tmp_path / "failure.json"
    source.write_text(json.dumps(document))
    env = {
        **os.environ,
        "QRMI_JOB_QPU_RESOURCES": "native_test",
        "QRMI_JOB_QPU_TYPES": "maestro-local",
        "native_test_QRMI_JOB_ACQUISITION_TOKEN": native_resource.acquire(),
    }
    result = subprocess.run(
        [*command, "native_test", str(source)],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert result.returncode != 0, result.stdout
    assert "Failed" in result.stderr
    assert "failure_message" in result.stderr
    assert "failure_message" not in result.stdout


@pytest.mark.parametrize(
    "native_resource", ["invalid_profiles", "missing_scancel"], indirect=True
)
def test_mpi_configuration_errors_are_typed(native_resource):
    """Classify malformed profiles and missing cancellation tools as config errors."""
    document = native_request("execute", "statevector")
    document["launch"] = {"profile": "broken", "ranks": 2}
    with pytest.raises(ConfigError):
        native_resource.task_start(request_payload(document))


def test_launch_validation_and_batch_metadata_contract(native_resource):
    """Validate launch routing and retain MPI metadata on a batch result."""
    profile = os.environ.get("QRMI_TEST_MPI_PROFILE")
    if not profile:
        pytest.skip("set QRMI_TEST_MPI_PROFILE for the launch contract test")
    document = native_request("execute", "statevector")
    for launch, error in [
        ({"profile": profile, "ranks": 3}, InvalidInputError),
        ({"profile": "not-a-configured-profile", "ranks": 2}, UnsupportedFunctionError),
        (None, InvalidInputError),
    ]:
        document["launch"] = launch
        with pytest.raises(error):
            native_resource.task_start(request_payload(document))
    document["launch"] = {"profile": profile, "ranks": 2}
    nested = {"schema_version": 2, "operation": "batch", "requests": [document]}
    with pytest.raises(InvalidInputError):
        native_resource.task_start(request_payload(nested))
    document.pop("launch")
    nested["launch"] = {"profile": profile, "ranks": 2}
    task = native_resource.task_start(request_payload(nested))
    assert await_task(native_resource, task) == TaskStatus.Completed
    result = json.loads(native_resource.task_result(task).value)
    assert result["mpi"]["ranks"] == 2
    assert result["results"][0]["shots"] == 16


@pytest.mark.parametrize("native_resource", ["short_lease"], indirect=True)
def test_task_logs_renew_the_documented_session_lease(native_resource):
    """Keep a short-lived session alive by retrieving retained task logs."""
    assert (
        json.loads(native_resource.target().value)["server"]["session_lease_seconds"]
        == 1
    )
    task = native_resource.task_start(
        request_payload(native_request("execute", "statevector"))
    )
    assert await_task(native_resource, task) == TaskStatus.Completed
    native_resource.task_result(task)
    deadline = time.monotonic() + 2.4
    while time.monotonic() < deadline:
        assert json.loads(native_resource.task_logs(task))["ok"] is True
        time.sleep(0.2)


@pytest.mark.parametrize("noisy", [False, True])
def test_real_mpi_gpu_execution(native_resource, noisy):
    """Check ideal and noisy execution on actual MPI GPU ranks when configured."""
    profile = os.environ.get("QRMI_TEST_MPI_GPU_PROFILE")
    if not profile:
        pytest.skip(
            "set QRMI_TEST_MPI_GPU_PROFILE with rank-local GPUs and CUDA-aware MPI"
        )
    document = native_request("execute", "statevector")
    document["circuit"] = {
        "num_qubits": 2,
        "source": "OPENQASM 2.0; qreg q[2]; creg c[2]; x q[0]; measure q->c;",
    }
    document["simulator"] = {
        "backend": "distributed_mpi_gpu",
        "options": {"gpu_device": 0},
        "distribution": {
            "backend": "conventional",
            "global_qubits": [1],
            "mpi_p2p_bits": 0,
        },
    }
    document["launch"] = {"profile": profile, "ranks": 2}
    if noisy:
        document["noise"] = {
            "mode": "pauli",
            "evaluation": "trajectories",
            "realizations": 2,
            "channels": [{"kind": "bit_flip", "targets": [0], "probability": 1}],
        }
    task = native_resource.task_start(request_payload(document))
    assert await_task(native_resource, task) == TaskStatus.Completed
    result = json.loads(native_resource.task_result(task).value)
    assert result["execution_metadata"]["backend"] == "distributed_mpi_gpu"
    assert result["mpi"]["ranks"] == 2
    assert result["counts"] == {("00" if noisy else "10"): 16}
