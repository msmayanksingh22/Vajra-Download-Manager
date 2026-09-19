# cli-commands Specification

## Purpose
TBD - created by archiving change cli-enhancements. Update Purpose after archive.
## Requirements
### Requirement: Vajra CLI Add Command
The system SHALL provide a command to add new downloads directly from the command line interface.

#### Scenario: User adds a download via URL
- **WHEN** the user executes `vajra-cli add "http://example.com/file.zip"`
- **THEN** the CLI sends a request to the daemon to enqueue the download and returns the new task ID.

### Requirement: Vajra CLI Pause Command
The system SHALL provide a command to pause active downloads using their ID.

#### Scenario: User pauses a download
- **WHEN** the user executes `vajra-cli pause <id>`
- **THEN** the CLI requests the daemon to pause the download with the specified ID and outputs a success confirmation.

### Requirement: Vajra CLI Resume Command
The system SHALL provide a command to resume paused downloads using their ID.

#### Scenario: User resumes a download
- **WHEN** the user executes `vajra-cli resume <id>`
- **THEN** the CLI requests the daemon to resume the download with the specified ID and outputs a success confirmation.

### Requirement: Vajra CLI List Command
The system SHALL provide a command to list all current downloads and their statuses.

#### Scenario: User lists downloads
- **WHEN** the user executes `vajra-cli list`
- **THEN** the CLI requests the current state from the daemon and prints a formatted table of downloads (ID, Name, Status, Progress, Speed).

### Requirement: Vajra CLI Remove Command
The system SHALL provide a command to remove downloads using their ID.

#### Scenario: User removes a download
- **WHEN** the user executes `vajra-cli remove <id>`
- **THEN** the CLI requests the daemon to remove the download and outputs a confirmation.

