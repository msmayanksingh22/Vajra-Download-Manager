# automation-convenience Specification

## Purpose
Introduce QoL features such as Clipboard Monitoring, Auto-Extraction, and Custom Post-Processing Scripts to further automate the user's download workflows.

## Requirements

### Requirement: Clipboard listener
The system SHALL monitor the OS clipboard for strings matching URLs (http/https/magnet/urn) and trigger a download event.

#### Scenario: Valid URL copied
- **WHEN** user copies an `http://` or `magnet:?` string to clipboard
- **THEN** system detects the URL and triggers a prompt or auto-download

### Requirement: Configurable clipboard monitor
The system SHALL allow users to enable or disable the clipboard monitor via settings.

#### Scenario: Disable clipboard monitor
- **WHEN** user disables clipboard monitor in settings
- **THEN** copying a URL does not trigger a download

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
