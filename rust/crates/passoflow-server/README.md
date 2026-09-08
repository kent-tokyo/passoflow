# passoflow-server

The local-only HTTP boundary for PassoFlow's Web UI.

This crate currently exposes only the safe bootstrap endpoints needed to prove
the Rust server boundary:

- `GET /api/health`
- `GET /api/version`

It binds to an explicitly supplied address and does not listen on a public
interface by default. Scenario CRUD, execution streaming, and browser/desktop
operations remain on the existing Python API until their Rust contracts are
ported and verified.

