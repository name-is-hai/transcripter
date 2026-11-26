# Third-party notices

## GPUI 0.2.2

Transcripter uses the official [GPUI](https://gpui.rs/) source under Apache-2.0. The release is vendored in `vendor/gpui` so the Windows build can replace one `u_strlen` call with an equivalent Rust UTF-16 terminator scan. This removes GPUI's startup dependency on the unresolved `icuuc.dll` entry point without changing its behavior.

- Original package: <https://crates.io/crates/gpui/0.2.2>
- Vendored source: [`vendor/gpui`](vendor/gpui)
- License text: [`licenses/GPUI-APACHE-2.0.txt`](licenses/GPUI-APACHE-2.0.txt)

## FFmpeg 8.0.1

Transcripter release executables contain separate `ffmpeg` and `ffprobe` helper programs from the [FFmpeg project](https://ffmpeg.org/). They are built without GPL or nonfree components and are used under LGPL-2.1-or-later.

- Exact source: <https://ffmpeg.org/releases/ffmpeg-8.0.1.tar.xz>
- SHA-256: `05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41`
- Reproducible configuration: [`scripts/build-ffmpeg.sh`](scripts/build-ffmpeg.sh)
- License text: [`licenses/FFmpeg-LGPL-2.1.txt`](licenses/FFmpeg-LGPL-2.1.txt)

Tagged GitHub Releases attach the exact FFmpeg source archive next to the application executables. Run `transcripter licenses` to print this notice and the complete license from a release executable.

FFmpeg remains a separate program. Transcripter writes the embedded helper bytes into a user-private cache, verifies their SHA-256 digest, and invokes them as child processes.
