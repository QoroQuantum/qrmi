# Maestro Rust client

A client for the Maestro Local Unix socket protocol. Connections use
`/run/maestro.sock` by default; `QRMI_MAESTRO_SOCKET` optionally selects a different
socket. The setting is process-wide and should stay fixed while sessions are used.

The QRMI adapter, lifecycle semantics and mocked tests are described in the
[Maestro migration notes](../../docs/migration/maestro-0.24.md).
