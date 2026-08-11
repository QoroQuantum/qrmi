# Python examples for Qiskit Primitives with Maestro Local QRMI

## Prerequisites

* Rust 1.85.1 or above
* Python 3.11 or 3.12
* [Installation of QRMI python package](../../../README.md)
* A running Maestro Local server, reachable at the Unix domain socket `/run/maestro.sock`

## Install dependencies

Assuming your python virtual environment is located at `~/py311venv_qrmi_primitives/bin/activate`,

```shell-session
$ source ~/py311venv_qrmi_primitives/bin/activate
$ pip install -r requirements.txt
```

## Environment variables

Maestro Local connects to a fixed local Unix socket, so no endpoint/token
environment variables are required, and it is not looked up through
`QRMIService` like the cloud-based providers. Instead, each example
instantiates the QRMI directly for a fixed backend name (`MAESTRO_LOCAL`).

The QRMI wrapper only reads (and each example writes)
`<backend_name>_QRMI_JOB_ACQUISITION_TOKEN`, which holds the session ID
returned by `acquire()`.

## How it works

Each example builds a circuit (and, for the estimator, an observable) using
Qiskit, converts the circuit to QASM with `qiskit.qasm2.dumps()`, and submits
it as a `MaestroLocal` task payload to be executed by the local server.

### SamplerV2-style

[`sampler.py`](./sampler.py) builds a 3-qubit GHZ-state circuit, executes it,
and prints the measurement counts.

```shell-session
$ python sampler.py
```

### EstimatorV2-style

[`estimator.py`](./estimator.py) builds a Bell-state circuit and a pair of
Pauli observables (`ZZ`, `XX`), estimates their expectation values, and
prints the results.

```shell-session
$ python estimator.py
```
