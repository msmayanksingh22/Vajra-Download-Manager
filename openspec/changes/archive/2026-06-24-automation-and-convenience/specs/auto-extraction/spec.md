## ADDED Requirements

### Requirement: Auto-extract downloads
The system SHALL extract downloaded files ending in `.zip`, `.rar`, or `.7z` automatically upon successful completion, if the user setting is enabled.

#### Scenario: Successful extraction
- **WHEN** a `.zip` download completes and auto-extract is enabled
- **THEN** system decompresses the contents into a folder with the archive's name

### Requirement: Configuration toggle
The system SHALL allow users to toggle auto-extraction globally.

#### Scenario: Disable auto-extract
- **WHEN** a `.zip` download completes and auto-extract is disabled
- **THEN** system does nothing and leaves the `.zip` file alone
