# Transcripter

Transcripter is a Rust desktop application and CLI for inspecting, extracting, and adding subtitle tracks in MKV files. The interface uses the official [`gpui`](https://crates.io/crates/gpui) crate documented at [gpui.rs](https://gpui.rs/).

A release executable contains both interfaces:

- run it without arguments to open the GPUI desktop interface;
- pass `inspect`, `extract`, `add`, or `licenses` to use the CLI.

The program never overwrites an existing output. Adding a subtitle copies the original video, audio, attachments, chapters, metadata, and subtitle streams into a new MKV without re-encoding them.

## Portable releases

Tagged releases contain one downloadable application package per platform:

| Platform | Release file | Usage |
| --- | --- | --- |
| Linux x86_64 | `transcripter-linux-x86_64.run` | `chmod +x FILE && ./FILE` |
| Linux ARM64 | `transcripter-linux-arm64.run` | `chmod +x FILE && ./FILE` |
| Windows x86_64 | `transcripter-windows-x86_64.exe` | Open it normally or use it from PowerShell |
| Windows ARM64 | `transcripter-windows-arm64.exe` | Open it normally or use it from PowerShell |
| macOS Intel | `transcripter-macos-x86_64.tar.gz` | Extract and open `Transcripter.app` |
| macOS Apple Silicon | `transcripter-macos-arm64.tar.gz` | Extract and open `Transcripter.app` |

Release builds embed minimal `ffmpeg` and `ffprobe` helper programs. Users do not install Rust, FFmpeg, or development libraries. The application extracts the helpers into a private user cache and verifies their SHA-256 digest before execution.

The operating system still supplies its normal windowing, graphics-driver, and C runtime components. The Linux `.run` files carry the non-core GUI libraries linked by each native build. The Windows ARM64 application is native ARM64; its small x86_64 FFmpeg helpers run through Windows 11 on ARM's built-in x64 emulation and are smoke-tested on the ARM64 runner. macOS packages are ad-hoc signed; without Apple signing credentials, Gatekeeper may require using **Open** from the context menu on first launch.

## Desktop interface during development

Development builds use `ffmpeg` and `ffprobe` from `PATH` unless the `bundled-media-tools` feature is enabled.

On Fedora, install the local build requirements with:

```bash
sudo dnf install \
  ffmpeg \
  gcc-c++ \
  libxcb-devel \
  libxkbcommon-devel \
  libxkbcommon-x11-devel
```

Run the GUI:

```bash
cargo run --features gui --bin transcripter
```

## CLI

Inspect the tracks in both episodes:

```bash
cargo run --bin transcripter -- inspect \
  "/home/liam/Downloads/Friends S1E1.mkv" \
  "/home/liam/Downloads/Friends S1E2.mkv"
```

Extract the embedded track titled `English`:

```bash
cargo run --bin transcripter -- extract \
  "/home/liam/Downloads/Friends S1E1.mkv" \
  "/home/liam/Downloads/Friends S1E2.mkv" \
  --language English \
  --output-dir "/home/liam/Downloads/subtitles"
```

This produces:

```text
/home/liam/Downloads/subtitles/Friends S1E1.en.srt
/home/liam/Downloads/subtitles/Friends S1E2.en.srt
```

Add an external SRT as a selectable track:

```bash
cargo run --bin transcripter -- add \
  "/home/liam/Downloads/Friends S1E1.mkv" \
  "/path/to/subtitle.srt" \
  --language eng \
  --title English
```

The default output is `Friends S1E1.subtitled.mkv`. Use `--output FILE` to choose another destination.

## Automated releases

The workflow in `.github/workflows/release.yml` builds Linux and Windows applications for x86_64 and ARM64, plus Intel and Apple Silicon macOS applications. It runs automatically for tags matching `v*` and can also be started manually from the Actions page.

Create a release with:

```bash
git tag v0.1.5
git push origin v0.1.5
```

A tag run creates the GitHub Release and uploads all platform packages, `SHA256SUMS`, notices, and the exact FFmpeg source archive required for license compliance. A manual run builds downloadable workflow artifacts but does not create a GitHub Release.

## Embedded FFmpeg

`scripts/build-ffmpeg.sh` builds FFmpeg 8.0.1 with only the Matroska, SRT, file-protocol, SubRip, `ffmpeg`, and `ffprobe` features needed by this application. GPL and nonfree components are disabled.

See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). A release executable also prints its notices and complete license text:

```bash
transcripter licenses
```

## Tests

```bash
cargo test --lib --test cli
```

The integration test creates a temporary MKV, verifies inspection and extraction, adds an SRT, confirms every original stream remains present, and checks that existing outputs and the source video are not changed.
