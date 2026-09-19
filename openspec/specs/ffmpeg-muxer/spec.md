# FFmpeg Muxer

## Summary

The FFmpeg Muxer is used internally to seamlessly stitch together complex streams (such as DASH, where video and audio are retrieved as separate files) into a single unified media file, directly matching Vajra's goal of advanced stream capture.

## Architecture

1. **Integration Trigger**:
   - Downloads flagged with `use_ytdlp: true` (often identified by the Media Sniffer) may produce separated streams (e.g., `video.mp4` and `audio.m4a`).
   - The daemon automatically detects these split streams upon successful fetch.

2. **Automated Muxing**:
   - Vajra invokes a bundled or system-available `ffmpeg` executable.
   - It runs a lossless stream copy (`-c copy`) to mux the separated audio and video into the final container (e.g., `.mkv` or `.mp4`) without re-encoding, ensuring maximum performance and zero quality loss.

3. **Cleanup**:
   - Post-muxing, Vajra safely deletes the intermediate separated stream files from the `temp_dir` to maintain disk hygiene.
