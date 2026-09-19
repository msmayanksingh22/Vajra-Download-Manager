## ADDED Requirements

### Requirement: Download Engine Orchestrator
The engine SHALL coordinate multiple concurrent HTTP requests to fetch segments of a file and assemble them into a contiguous final file on disk.

#### Scenario: Orchestrator spawns dynamic threads
- **WHEN** a file is queued for download
- **THEN** the engine establishes an initial set of static chunks based on max connections, but explicitly passes shared references to the connection pool and the global active chunk list to support dynamic splitting.
