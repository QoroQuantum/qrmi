Maestro Local with QRMI 0.26
============================

QRMI 0.26 retains Maestro's local-server transport, native API v2 requests,
legacy payloads and session lifecycle alongside upstream's OQTOPUS provider.

Rebuild consumers
-----------------

This merge preserves upstream's numeric provider values. OQTOPUS uses resource
type 7 and C payload tag 4; Maestro now uses resource type 8 and C payload tag 5
(previously 7 and 4 respectively). Rebuild the QRMI Python extension, C library,
Lua module and Slurm SPANK plugin together against the regenerated ``qrmi.h``.
Do not load an old Maestro consumer with the new library. Use symbolic enum
names in source code rather than numeric constants.

See :doc:`maestro-0.24` for build commands and the existing session lifecycle.
The Maestro local server itself needs no protocol change for this merge.

Explicit resource configuration
-------------------------------

Maestro supports upstream's new configuration-map constructor in Rust, Python,
C and Lua. For example:

.. code:: python

   from qrmi import QuantumResource, ResourceType

   resource = QuantumResource.from_config(
       "MAESTRO_LOCAL",
       ResourceType.MaestroLocal,
       {"QRMI_MAESTRO_SOCKET": "/run/maestro.sock"},
   )
   token = resource.acquire()
   try:
       print(resource.status().to_dict())
   finally:
       resource.release(token)

``QRMI_MAESTRO_SOCKET`` defaults to ``/run/maestro.sock``. The optional
``QRMI_JOB_ACQUISITION_TOKEN`` selects an existing session and must be an
unsigned 32-bit integer string. Keys have no backend-name prefix. Lowercase
keys are accepted when the uppercase key is absent. Explicit configuration
does not read or modify process environment variables, including when a key
is omitted. The existing environment-based constructor remains available.

Resource status
---------------

``status()`` uses the server's additive API v2 ``status`` command without
creating or renewing a session. It reports Online when the runner accepts jobs
and Offline otherwise, with ``status_reason``, ``healthy``, ``busy`` and
``pending_job_count`` from the server. Capacity is unspecified. Only an older
server that rejects the command as unsupported falls back to PING, with health,
occupancy and queue length unknown; transport errors and malformed replies
remain errors. The deprecated ``is_accessible()`` retains its PING behavior.

Validation
----------

The payload schema accepts both OQTOPUS and Maestro requests. Maestro-specific
fields cannot bypass native or legacy validation by adding an OQTOPUS
``job_spec`` field. Tests cover explicit configuration isolation, session reuse,
status replies, provider enum values and schema dispatch.
