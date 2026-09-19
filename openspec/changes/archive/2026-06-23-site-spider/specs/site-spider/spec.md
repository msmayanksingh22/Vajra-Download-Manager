## ADDED Requirements

### Requirement: Spider Base URL parsing
The system SHALL parse the HTML from a given base URL and extract all relevant `href` and `src` links.

#### Scenario: User triggers a spider scan
- **WHEN** the user provides a valid URL to the spider endpoint
- **THEN** the system fetches the HTML, parses it, and returns a list of absolute URLs found within the page.

### Requirement: Spider File Filtering
The system SHALL support filtering the discovered links by file extension or regex pattern.

#### Scenario: User filters for mp4 files
- **WHEN** the user sets a filter for `.mp4`
- **THEN** only links ending in `.mp4` are displayed/returned.

### Requirement: Batch Task Queueing
The system SHALL allow batch enqueuing of selected links from the spider results to the download engine without blocking the main event loop.

#### Scenario: User selects multiple files for download
- **WHEN** the user selects multiple links and clicks "Download Selected"
- **THEN** the system enqueues all selected links into the Vajra core engine asynchronously.
