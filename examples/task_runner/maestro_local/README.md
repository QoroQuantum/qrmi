# Tools to generate task runner input for Maestro Local

The tools demonstrate the generation of `task_runner` input from a Qiskit circuit,
targeting a Maestro Local resource.

## Prerequisites
* Python 3.11 or above

## Install dependencies

```shell-session
pip install -r requirements.txt
```

## Tools

### gen_sampler_input.py

Builds a GHZ-state circuit, converts it to QASM, and writes a `task_runner`
input file for an `execute` job (i.e. measurement counts).

Usage:
```shell-session
usage: gen_sampler_input.py [-h] [--qubits QUBITS] [--shots SHOTS]
                             [--simulator-type SIMULATOR_TYPE]
                             [--simulation-method SIMULATION_METHOD]
                             [-o OUTPUT]

options:
  -h, --help            show this help message and exit
  --qubits QUBITS       Number of qubits
  --shots SHOTS         Number of shots
  --simulator-type SIMULATOR_TYPE
                        Simulator type (0 = aer, 1 = qcsim)
  --simulation-method SIMULATION_METHOD
                        Simulation method (0 = statevector, 1 = matrix_product_state)
  -o OUTPUT, --output OUTPUT
                        Output filename
```

Example:
```bash
python gen_sampler_input.py --qubits 3 -o sampler_input.json
```

### gen_estimator_input.py

Builds a Bell-state circuit and a pair of Pauli observables (`ZZ`, `XX`),
converts the circuit to QASM, and writes a `task_runner` input file for an
`estimate` job (i.e. expectation values).

Usage:
```shell-session
usage: gen_estimator_input.py [-h] [--simulator-type SIMULATOR_TYPE]
                               [--simulation-method SIMULATION_METHOD]
                               [-o OUTPUT]
```

Example:
```bash
python gen_estimator_input.py -o estimator_input.json
```

## Output

Both tools write a single JSON file matching the `maestro-local` entry of the
[QRMI payload schema](../../../qrmi_payload_v1_schema.json):

| Field | Description |
| ---- | ---- |
| `input` | QASM string for the circuit |
| `job_type` | `execute` or `estimate` |
| `qubits` | Number of qubits |
| `simulator_type` | Simulator type (0 = aer, 1 = qcsim) |
| `simulation_method` | Simulation method (0 = statevector, 1 = matrix_product_state) |
| `observables` | Pauli strings separated by `;` (required for `estimate`) |
| `config` | Task configuration, as a JSON-formatted string (e.g. `{"shots": 1000}`) |

## Running with task_runner

```shell-session
export QRMI_JOB_QPU_RESOURCES=MAESTRO_LOCAL
export QRMI_JOB_QPU_TYPES=maestro-local

task_runner MAESTRO_LOCAL sampler_input.json
```

See the [task_runner documentation](../../../python/qrmi/tools/task_runner/README.md)
for more details.
