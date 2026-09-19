# cli-enhancements Specification

## Purpose
Expand the `vajra-cli` headless argument support to allow programmatic interaction with the Vajra daemon from the command line, enabling integration with other tools, scripts, and CI/CD pipelines.

## Requirements
### Requirement: Daemon Status Verification
The system SHALL provide a CLI command to query the status and health of the daemon.

#### Scenario: User checks daemon status
- **WHEN** the user runs `vajra-cli status`
- **THEN** the system outputs the current daemon status, queue size, global speed limits, and uptime.

### Requirement: Add Download Headless
The system SHALL provide a CLI command to add a URL to the download queue without invoking the UI.

#### Scenario: User adds a file
- **WHEN** the user runs `vajra-cli add https://example.com/file.iso --output /tmp/`
- **THEN** the system successfully enqueues the file and returns the Job ID.

### Requirement: Queue Management Headless
The system SHALL provide CLI commands to pause, resume, cancel, and remove downloads.

#### Scenario: User pauses a download
- **WHEN** the user runs `vajra-cli pause <id>`
- **THEN** the system pauses the specific download and returns success.
