# Third-party notices

## FFmpeg 8.0.1

Transcripter release executables contain separate `ffmpeg` and `ffprobe` helper programs from the [FFmpeg project](https://ffmpeg.org/). They are built without GPL or nonfree components and are used under LGPL-2.1-or-later.

- Exact source: <https://ffmpeg.org/releases/ffmpeg-8.0.1.tar.xz>
- SHA-256: `05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41`
- Reproducible configuration: [`scripts/build-ffmpeg.sh`](scripts/build-ffmpeg.sh)
- License text: [`licenses/FFmpeg-LGPL-2.1.txt`](licenses/FFmpeg-LGPL-2.1.txt)

Tagged GitHub Releases attach the exact FFmpeg source archive next to the application executables. Run `transcripter licenses` to print this notice and the complete license from a release executable.

FFmpeg remains a separate program. Transcripter writes the embedded helper bytes into a user-private cache, verifies their SHA-256 digest, and invokes them as child processes.
