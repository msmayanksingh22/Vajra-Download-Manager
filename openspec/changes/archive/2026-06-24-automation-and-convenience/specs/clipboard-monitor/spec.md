## ADDED Requirements

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
