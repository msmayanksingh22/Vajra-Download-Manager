## ADDED Requirements

### Requirement: Global Connection Pooling
The system SHALL use a shared HTTP Client with Keep-Alive explicitly enabled across all thread chunk requests for a given download.

#### Scenario: Successive chunk requests
- **WHEN** a thread finishes a chunk and immediately requests a new dynamic chunk from the same server
- **THEN** the system reuses the existing TCP connection, skipping the TLS handshake phase, thereby decreasing time-to-first-byte (TTFB).

#### Scenario: Server forcefully closes idle connection
- **WHEN** the remote server closes the connection due to an idle timeout
- **THEN** the HTTP Client automatically negotiates a fresh connection for the next chunk request without crashing the thread.
