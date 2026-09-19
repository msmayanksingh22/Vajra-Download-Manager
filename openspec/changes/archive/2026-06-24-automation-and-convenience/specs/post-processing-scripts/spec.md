## ADDED Requirements

### Requirement: Execute post-processing scripts
The system SHALL execute a user-configured OS script (bash, batch, powershell) when a download successfully completes.

#### Scenario: Script execution
- **WHEN** a download finishes and a script is configured
- **THEN** system spawns the script and passes the downloaded file path as an argument

### Requirement: Configure script path
The system SHALL allow users to define the path to the post-processing script in settings.

#### Scenario: Empty script path
- **WHEN** the script path is empty
- **THEN** no post-processing action occurs
