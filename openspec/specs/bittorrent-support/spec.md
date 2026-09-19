# bittorrent-support Specification

## Purpose
Introduce native BitTorrent downloading capabilities directly into Vajra Engine using a crate like `librqbit` to allow decentralized P2P downloads alongside standard HTTP multi-segment downloads.

## Requirements
### Requirement: Parse .torrent files
The system SHALL accept and parse standard `.torrent` files.

### Requirement: Magnet Links
The system SHALL accept magnet links and resolve their metadata.

### Requirement: P2P Download
The system SHALL connect to trackers and peers, download pieces concurrently, verify piece hashes, and construct the final files without corrupting data.

### Requirement: UI Integration
The UI SHALL display BitTorrent-specific metrics such as Seeds, Peers, Upload/Download Ratio, and Piece maps.
