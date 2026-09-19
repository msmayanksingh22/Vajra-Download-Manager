## ADDED Requirements

### Requirement: Spider crawls basic HTML links
The system SHALL recursively fetch HTML content up to a given depth limit and extract all valid URLs from `href` and `src` attributes.

#### Scenario: Crawl depth 1
- **WHEN** user requests a crawl with depth 1
- **THEN** the system returns a list of URLs discovered on the requested page, resolving relative paths.

### Requirement: Spider filters links
The system SHALL support filtering extracted links by extension and regex.

#### Scenario: Filter by extension
- **WHEN** user requests a crawl with extension filter `.mp4`
- **THEN** only links ending in `.mp4` are returned in the result set.

### Requirement: UI batch download
The system SHALL provide a UI window to input the base URL, start the spider, view the returned list, select multiple items, and add them to the download queue.

#### Scenario: Batch add
- **WHEN** user selects 3 links from the spider results and clicks "Download Selected"
- **THEN** 3 download tasks are added to the daemon's queue and appear in the main UI grid.
