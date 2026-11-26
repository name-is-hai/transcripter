use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn transcripter(cache: &Path) -> Command {
    #[cfg(feature = "bundled-media-tools")]
    {
        let mut command = Command::new(env!("CARGO_BIN_EXE_transcripter"));
        let empty_path = if cfg!(windows) {
            r"C:\transcripter-no-system-tools"
        } else {
            "/transcripter-no-system-tools"
        };
        command
            .env("PATH", empty_path)
            .env("HOME", cache)
            .env("XDG_CACHE_HOME", cache)
            .env("LOCALAPPDATA", cache);
        command
    }

    #[cfg(not(feature = "bundled-media-tools"))]
    {
        let _ = cache;
        Command::new(env!("CARGO_BIN_EXE_transcripter"))
    }
}

fn run(command: &mut Command) -> Output {
    command.output().expect("command should start")
}

fn make_fixture() -> (TempDir, PathBuf, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let embedded = directory.path().join("embedded.srt");
    let external = directory.path().join("external.srt");
    let video = directory.path().join("episode.mkv");

    fs::write(
        &embedded,
        "1\n00:00:00,000 --> 00:00:01,400\nExisting subtitle\n",
    )
    .unwrap();
    fs::write(
        &external,
        "1\n00:00:00,200 --> 00:00:01,200\nAdded subtitle\n",
    )
    .unwrap();

    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(["-f", "lavfi", "-i", "color=c=black:s=64x64:r=1:d=2"])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=8000:duration=2",
        ])
        .arg("-i")
        .arg(&embedded)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-map", "2:s:0"])
        .args(["-c:v", "ffv1", "-c:a", "pcm_s16le", "-c:s", "srt"])
        .args(["-metadata:s:s:0", "language=eng"])
        .args(["-metadata:s:s:0", "title=English"])
        .arg(&video)
        .status()
        .expect("ffmpeg should start");
    assert!(status.success(), "ffmpeg should create the fixture");

    (directory, video, external)
}

fn stream_types(path: &Path) -> Vec<String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("ffprobe should start");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn cli_inspects_extracts_and_adds_without_touching_source() {
    let (directory, video, external) = make_fixture();
    let source_before = fs::read(&video).unwrap();

    let licenses = run(transcripter(directory.path()).arg("licenses"));
    assert!(licenses.status.success());
    let license_text = String::from_utf8(licenses.stdout).unwrap();
    assert!(license_text.contains("Vu Dao Ngoc Hai"));
    assert!(license_text.contains("official GPUI 0.2.2"));
    assert!(license_text.contains("Apache License"));
    assert!(license_text.contains("FFmpeg 8.0.1"));
    assert!(license_text.contains("GNU LESSER GENERAL PUBLIC LICENSE"));

    let inspected = run(transcripter(directory.path()).arg("inspect").arg(&video));
    assert!(inspected.status.success());
    let stdout = String::from_utf8(inspected.stdout).unwrap();
    assert!(stdout.contains("language=eng"));
    assert!(stdout.contains("title=English"));

    let output_directory = directory.path().join("subtitles");
    let extracted = run(transcripter(directory.path())
        .arg("extract")
        .arg(&video)
        .args(["--language", "English", "--output-dir"])
        .arg(&output_directory));
    assert!(
        extracted.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted.stderr)
    );
    let extracted_path = output_directory.join("episode.en.srt");
    let extracted_text = fs::read_to_string(extracted_path).unwrap();
    assert!(extracted_text.contains("Existing subtitle"));

    let no_match = run(transcripter(directory.path())
        .arg("extract")
        .arg(&video)
        .args(["--language", "Klingon"]));
    assert!(!no_match.status.success());
    assert!(String::from_utf8_lossy(&no_match.stderr).contains("Available tracks"));

    let output_video = directory.path().join("episode.with-subtitle.mkv");
    let added = run(transcripter(directory.path())
        .arg("add")
        .arg(&video)
        .arg(&external)
        .args(["--language", "eng", "--title", "English added", "--output"])
        .arg(&output_video));
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    assert_eq!(
        stream_types(&output_video),
        vec!["video", "audio", "subtitle", "subtitle"]
    );
    let output_inspection = run(transcripter(directory.path())
        .arg("inspect")
        .arg(&output_video));
    let output_text = String::from_utf8(output_inspection.stdout).unwrap();
    assert!(output_text.contains("title=English added"));

    let completed_before = fs::read(&output_video).unwrap();
    let refused = run(transcripter(directory.path())
        .arg("add")
        .arg(&video)
        .arg(&external)
        .arg("--output")
        .arg(&output_video));
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("refusing to replace"));
    assert_eq!(fs::read(&output_video).unwrap(), completed_before);
    assert_eq!(fs::read(&video).unwrap(), source_before);
}
