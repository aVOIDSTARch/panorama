Incomplete Integrations
Integration	Status	What's Left
YubiKey/FIDO2/WebAuthn	Password placeholder only	Need webauthn-rs crate, FIDO2 registration ceremony, credential storage in Datastore, session binding to hardware key
Telnyx Ed25519 verification	Checks signature presence, not validity	Implement actual Ed25519 verify against TELNYX_PUBLIC_KEY in inbound.rs
TOTP for owner commands	TODO comment in identity.rs	TOTP token extraction from SMS body for Owner-level auth
Recognized sender list	TODO in identity.rs	Datastore-backed list of previously-seen senders
Tailscale binding	Design only, no code	Bind admin-interface to Tailscale interface only (or IP allowlist)
cortex-mcp	4-line stub	Full MCP tool auto-generation from cortex-manifest.toml
Episteme setup	Submodule exists, no install	Python service needs venv setup, pip install, startup script
Cross-language token tests	Not started	Rust mint -> Python verify -> TypeScript verify
CI/CD	None	GitHub Actions for cargo test, npm test, pytest
Infisical	Client complete, needs deployment	Real Infisical instance URL + token in .env
Admin Interface — Missing Features
The current admin interface at crates/admin-interface/ has only two panels (health + registered services). The nav links to /services and /logs already exist in the HTML but aren't wired. Here's what's needed:

Feature	Priority	Description
Log Viewer	High	Query _system_logs table from panorama-logging. Filter by service, level, error code, time range. HTMX live tail. The DB and indexes already exist.
Error Report Browser	High	Query _error_reports table. Group by code, show frequency, last occurrence, full detail with suggestion.
Halt Controls	High	Buttons to POST /cloak/admin/halt (global) and /cloak/admin/halt/:service (per-service). Resume button. Show current halt state. The API endpoints already exist.
Wheelhouse Monitor	Medium	Job queue depth, active agents, agent states (Idle/Active/Retiring), recent resolutions. Needs Wheelhouse to expose a /status endpoint.
Service Config	Medium	View/edit cortex-manifest.toml, gateway config. Currently read-only via health.
Permission Manager	Medium	CRUD for Cloak permissions (endpoints exist at /cloak/admin/permissions). Show current rules, add/remove.
Identity Panel	Low	Analog-communications sender allowlist management. View quarantined messages.
YubiKey Auth	High	Replace password login with FIDO2/WebAuthn ceremonies.
Tailscale Status	Low	Show Tailscale node status, connected peers.
The highest-impact additions are log viewer (data already flows into SQLite via panorama-logging), error browser (panorama-errors persists everything), and halt controls (API exists, just needs buttons).
