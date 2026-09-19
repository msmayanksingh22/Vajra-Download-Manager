## ADDED Requirements

### Requirement: Vajra CLI Daemon Start Command
The system SHALL provide a command to start the background daemon without launching the graphical user interface.

#### Scenario: User starts the daemon headlessly
- **WHEN** the user executes `vajra-cli daemon start`
- **THEN** the CLI spawns the `vajrad` process in the background and outputs the process ID or confirmation.

### Requirement: Vajra CLI Daemon Stop Command
The system SHALL provide a command to gracefully shut down the background daemon.

#### Scenario: User stops the daemon
- **WHEN** the user executes `vajra-cli daemon stop`
- **THEN** the CLI connects to the daemon and issues a shutdown signal, causing `vajrad` to exit safely.

### Requirement: Vajra CLI Daemon Status Command
The system SHALL provide a command to check if the daemon is currently running.

#### Scenario: User checks daemon status
- **WHEN** the user executes `vajra-cli daemon status`
- **THEN** the CLI attempts to ping the daemon's API port and reports whether it is running or offline.
