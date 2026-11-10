# Security and responsible disclosure

This project is not yet released. Until a private reporting channel is established, avoid publishing credential leaks, malicious file payloads or exploit details in a public issue. Contact the repository maintainers through a private GitHub contact mechanism where available.

The binary parser must treat `.vseph` as **untrusted input**: checked arithmetic, bounded allocations, validated offsets/counts/lengths, resource limits, no unvalidated memory mapping, and distinct errors for unsupported version, checksum failure, invalid data, missing coverage and malformed file.

Never put production PostgreSQL credentials or database exports in issues, logs, test artifacts, PR descriptions, examples, CI configurations or the repository. Importers must be read-only with least-privilege access. Never test against production tables with writes.
