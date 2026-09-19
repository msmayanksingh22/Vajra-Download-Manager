## ADDED Requirements

### Requirement: Dynamic Thread Reallocation
The system SHALL reassign threads that have finished their assigned byte ranges to assist slower threads by bisecting their remaining workload.

#### Scenario: Fast thread finishes before slow thread
- **WHEN** Thread A completes its designated chunk but Thread B still has > 1MB remaining
- **THEN** Thread A cuts Thread B's remaining bytes in half, claims ownership of the latter half, and immediately begins downloading it.

#### Scenario: Minimum threshold prevention
- **WHEN** Thread A completes but Thread B has less than 1MB remaining
- **THEN** Thread A does NOT split the chunk and instead gracefully exits or waits for next assignment.
