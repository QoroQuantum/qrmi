"""Maestro binding regressions using a scripted Unix socket, without a daemon."""

import ctypes
import os
import socket
import tempfile
import threading
from collections import deque
from pathlib import Path

import pytest

from qrmi import (
    ConfigError,
    InvalidInputError,
    Payload,
    QRMIService,
    QuantumResource,
    ResourceType,
    TaskNotFoundError,
    TaskNotReadyError,
    UnsupportedFunctionError,
    UnsupportedPayloadError,
)


@pytest.fixture(name="server")
def fixture_server(monkeypatch):
    """Serve exact command/reply pairs; fail on missing or unexpected traffic."""
    pending = deque()
    failures = []
    stopping = threading.Event()
    with tempfile.TemporaryDirectory(prefix="qrmi-") as directory:
        path = str(Path(directory) / "maestro.sock")
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
            listener.bind(path)
            listener.listen()
            listener.settimeout(0.05)
            monkeypatch.setenv("QRMI_MAESTRO_SOCKET", path)

            def serve():
                while not stopping.is_set():
                    try:
                        connection, _ = listener.accept()
                    except socket.timeout:
                        continue
                    with connection:
                        connection.settimeout(2)
                        try:
                            with connection.makefile("rb") as stream:
                                command = stream.readline().decode().rstrip("\r\n")
                            expected, reply = pending.popleft()
                            if command != expected:
                                failures.append(
                                    f"Expected {expected!r}, received {command!r}"
                                )
                        except (OSError, IndexError) as error:
                            failures.append(str(error))
                            reply = "ERROR unexpected request"
                        connection.sendall((reply + "\n").encode())

            worker = threading.Thread(target=serve)
            worker.start()
            try:
                yield pending
            finally:
                stopping.set()
                worker.join(timeout=3)
                assert not worker.is_alive(), "mock server did not stop"
                assert not failures, failures
                assert not pending, f"Unconsumed requests: {list(pending)}"


@pytest.fixture(autouse=True)
def fixture_clean_job_environment(monkeypatch):
    """Do not inherit scheduler state from the machine running the tests."""
    for name in (
        "QRMI_PLUGIN_ERROR",
        "py_maestro_QRMI_JOB_ACQUISITION_TOKEN",
        "offline_maestro_QRMI_JOB_ACQUISITION_TOKEN",  # pragma: allowlist secret
    ):
        monkeypatch.delenv(name, raising=False)
    monkeypatch.setenv("QRMI_LIST_DELIMITER", ",")


def test_python_errors_are_classified_without_a_session(monkeypatch):
    """Expose QRMI error categories before contacting a Maestro session."""
    resource = QuantumResource("py_maestro", ResourceType.MaestroLocal)
    with pytest.raises(InvalidInputError):
        resource.task_status("invalid")
    with pytest.raises(ConfigError):
        resource.task_status("1")
    with pytest.raises(InvalidInputError):
        resource.release("invalid")
    with pytest.raises(UnsupportedFunctionError):
        resource.target()
    with pytest.raises(UnsupportedFunctionError):
        resource.task_logs("1")
    with pytest.raises(UnsupportedPayloadError):
        resource.task_start(Payload.QiskitPrimitive(input="{}", program_id="sampler"))
    with pytest.raises(InvalidInputError):
        resource.task_start(
            Payload.MaestroLocal(
                input="OPENQASM 2.0; qreg q[1];",
                job_type="execute",
                qubits=1,
                simulator_type=0,
                simulation_method=0,
                observables="",
                config="{",
            )
        )
    monkeypatch.setenv("py_maestro_QRMI_JOB_ACQUISITION_TOKEN", "invalid")
    with pytest.raises(ConfigError):
        QuantumResource("py_maestro", ResourceType.MaestroLocal)


def test_service_preserves_maestro_session(server, monkeypatch):
    """Discover accessible resources and retain their acquired sessions."""
    monkeypatch.setenv("QRMI_JOB_QPU_RESOURCES", "py_maestro,offline_maestro")
    monkeypatch.setenv("QRMI_JOB_QPU_TYPES", "maestro-local,maestro-local")
    server.extend(
        [
            ("PING", "OK YES"),
            ("PING", "OK NO"),
            ("SESSION CREATE", "OK 42"),
            ("SESSION 42 EXISTS", "OK YES"),
            ("SESSION 42 DELETE", "OK YES"),
        ]
    )
    service = QRMIService()
    resources = service.resources()
    assert len(resources) == 1
    resource = service.resource("py_maestro")
    assert resource is resources[0]
    assert service.resource("offline_maestro") is None
    assert resource.resource_id() == "py_maestro"
    assert resource.resource_type() == ResourceType.MaestroLocal
    assert resource.acquire() == "42"
    assert service.resource("py_maestro").acquire() == "42"
    resource.release("42")
    with pytest.raises(ConfigError):
        resource.task_status("1")


def test_missing_and_failed_tasks_raise_distinct_errors(server, monkeypatch):
    """Keep missing tasks distinct from known failures in Python."""
    monkeypatch.setenv("py_maestro_QRMI_JOB_ACQUISITION_TOKEN", "8")
    server.extend(
        [
            ("SESSION 8 TASK 1 EXISTS", "OK NO"),
            ("SESSION 8 TASK 2 EXISTS", "OK YES"),
            ("SESSION 8 TASK 2 FAILED", "OK YES"),
        ]
    )
    resource = QuantumResource("py_maestro", ResourceType.MaestroLocal)
    with pytest.raises(TaskNotFoundError):
        resource.task_status("1")
    with pytest.raises(TaskNotReadyError, match="Failed"):
        resource.task_result("2")
    resource.task_stop("2")


class _Resources(ctypes.Structure):
    _fields_ = [
        ("resources", ctypes.POINTER(ctypes.c_void_p)),
        ("length", ctypes.c_size_t),
    ]


@pytest.fixture(name="c_api")
def fixture_c_api():
    """Opt in with QRMI_TEST_LIBRARY pointing to the matching C library."""
    path = os.environ.get("QRMI_TEST_LIBRARY")
    if path is None:
        pytest.skip(
            "set QRMI_TEST_LIBRARY to the rebuilt libqrmi.so to check the C ABI"
        )
    api = ctypes.CDLL(path)
    signatures = {
        "qrmi_resource_new": ([ctypes.c_char_p, ctypes.c_int], ctypes.c_void_p),
        "qrmi_resource_free": ([ctypes.c_void_p], ctypes.c_int),
        "qrmi_resource_type": (
            [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)],
            ctypes.c_int,
        ),
        "qrmi_resource_id": (
            [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)],
            ctypes.c_int,
        ),
        "qrmi_resource_target": (
            [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)],
            ctypes.c_int,
        ),
        "qrmi_resource_task_status": (
            [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_int)],
            ctypes.c_int,
        ),
        "qrmi_get_last_error": ([], ctypes.c_void_p),
        "qrmi_get_last_error_kind": ([], ctypes.c_int),
        "qrmi_string_free": ([ctypes.c_void_p], ctypes.c_int),
        "qrmi_service_resources": ([ctypes.POINTER(_Resources)], ctypes.c_int),
        "qrmi_service_resources_free": ([ctypes.POINTER(_Resources)], ctypes.c_int),
    }
    for name, (arguments, result) in signatures.items():
        function = getattr(api, name)
        function.argtypes = arguments
        function.restype = result
    return api


def test_c_error_codes_and_maestro_enum(c_api, server, monkeypatch):
    """Keep C error codes and the Maestro enum aligned with Python."""
    monkeypatch.setenv("py_maestro_QRMI_JOB_ACQUISITION_TOKEN", "invalid")
    assert not c_api.qrmi_resource_new(b"py_maestro", int(ResourceType.MaestroLocal))
    assert c_api.qrmi_get_last_error_kind() == 103  # ParseError
    message = c_api.qrmi_get_last_error()
    assert message
    try:
        assert b"ACQUISITION_TOKEN" in ctypes.string_at(message)
    finally:
        c_api.qrmi_string_free(message)

    monkeypatch.setenv("py_maestro_QRMI_JOB_ACQUISITION_TOKEN", "8")
    server.append(("SESSION 8 TASK 1 EXISTS", "OK NO"))
    resource = c_api.qrmi_resource_new(b"py_maestro", int(ResourceType.MaestroLocal))
    assert resource
    try:
        kind = ctypes.c_int()
        assert c_api.qrmi_resource_type(resource, ctypes.byref(kind)) == 0
        assert kind.value == int(ResourceType.MaestroLocal)
        value = ctypes.c_void_p()
        assert c_api.qrmi_resource_target(resource, ctypes.byref(value)) == 105
        assert (
            c_api.qrmi_resource_task_status(resource, b"invalid", ctypes.byref(kind))
            == 109
        )
        assert (
            c_api.qrmi_resource_task_status(resource, b"1", ctypes.byref(kind)) == 112
        )
        assert c_api.qrmi_get_last_error_kind() == 112
    finally:
        c_api.qrmi_resource_free(resource)


def test_c_service_discovers_maestro(c_api, server, monkeypatch):
    """Return accessible Maestro resources through the shared C service."""
    monkeypatch.setenv("QRMI_JOB_QPU_RESOURCES", "py_maestro,offline_maestro")
    monkeypatch.setenv("QRMI_JOB_QPU_TYPES", "maestro-local,maestro-local")
    server.extend([("PING", "OK YES"), ("PING", "OK NO")])
    resources = _Resources()
    assert c_api.qrmi_service_resources(ctypes.byref(resources)) == 0
    try:
        assert resources.length == 1
        resource = resources.resources[0]
        identifier = ctypes.c_void_p()
        assert c_api.qrmi_resource_id(resource, ctypes.byref(identifier)) == 0
        try:
            assert ctypes.string_at(identifier) == b"py_maestro"
        finally:
            c_api.qrmi_string_free(identifier)
        kind = ctypes.c_int()
        assert c_api.qrmi_resource_type(resource, ctypes.byref(kind)) == 0
        assert kind.value == int(ResourceType.MaestroLocal)
    finally:
        c_api.qrmi_service_resources_free(ctypes.byref(resources))
