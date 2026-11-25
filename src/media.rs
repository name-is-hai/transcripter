use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubtitleTrack {
    pub stream_index: usize,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

impl SubtitleTrack {
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or("Untitled")
    }

    pub fn display_language(&self) -> &str {
        self.language.as_deref().unwrap_or("unknown")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub streams: usize,
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    index: usize,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    tags: ProbeTags,
}

#[derive(Debug, Default, Deserialize)]
struct ProbeTags {
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

pub fn inspect(path: &Path) -> Result<MediaInfo> {
    require_regular_file(path, "video")?;

    let output = crate::media_tools::ffprobe_command()?
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(path)
        .output()
        .with_context(
            || "could not start ffprobe; install FFmpeg and ensure ffprobe is available in PATH",
        )?;

    ensure_success(output, "ffprobe", &format!("inspect {}", path.display()))
        .and_then(|stdout| parse_probe(path, &stdout))
}

fn parse_probe(path: &Path, stdout: &[u8]) -> Result<MediaInfo> {
    let probe: ProbeOutput = serde_json::from_slice(stdout)
        .with_context(|| format!("ffprobe returned invalid JSON for {}", path.display()))?;
    let stream_count = probe.streams.len();
    let subtitle_tracks = probe
        .streams
        .into_iter()
        .filter(|stream| stream.codec_type.as_deref() == Some("subtitle"))
        .map(|stream| SubtitleTrack {
            stream_index: stream.index,
            codec: stream.codec_name.unwrap_or_else(|| "unknown".to_owned()),
            language: stream.tags.language,
            title: stream.tags.title,
        })
        .collect();

    Ok(MediaInfo {
        path: path.to_path_buf(),
        streams: stream_count,
        subtitle_tracks,
    })
}

pub fn select_subtitle<'a>(
    tracks: &'a [SubtitleTrack],
    requested: &str,
) -> Result<&'a SubtitleTrack> {
    let requested = requested.trim();
    if requested.is_empty() {
        bail!("subtitle language cannot be empty");
    }

    let mut scored = tracks
        .iter()
        .filter_map(|track| match_score(track, requested).map(|score| (score, track)))
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));

    let Some((best_score, best_track)) = scored.first().copied() else {
        let available = available_tracks(tracks);
        bail!("no subtitle track matches '{requested}'. Available tracks: {available}");
    };

    let ties = scored
        .iter()
        .filter(|(score, _)| *score == best_score)
        .map(|(_, track)| *track)
        .collect::<Vec<_>>();
    if ties.len() > 1 {
        let choices = ties
            .iter()
            .map(|track| {
                format!(
                    "stream {} ({}, {})",
                    track.stream_index,
                    track.display_title(),
                    track.display_language()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        bail!("'{requested}' matches more than one subtitle track: {choices}. Use the exact title");
    }

    Ok(best_track)
}

pub fn extract_subtitle(input: &Path, track: &SubtitleTrack, destination: &Path) -> Result<()> {
    require_regular_file(input, "video")?;
    prepare_destination(destination, "subtitle")?;

    with_temporary_output(destination, |temporary| {
        let output = crate::media_tools::ffmpeg_command()?
            .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
            .arg("-i")
            .arg(input)
            .arg("-map")
            .arg(format!("0:{}", track.stream_index))
            .args(["-c:s", "subrip"])
            .arg(temporary)
            .output()
            .with_context(
                || "could not start ffmpeg; install FFmpeg and ensure ffmpeg is available in PATH",
            )?;
        ensure_success(
            output,
            "ffmpeg",
            &format!("extract subtitles from {}", input.display()),
        )?;

        let size = fs::metadata(temporary)
            .with_context(|| format!("could not inspect {}", temporary.display()))?
            .len();
        if size == 0 {
            bail!("ffmpeg produced an empty subtitle file");
        }
        Ok(())
    })
}

pub fn add_external_subtitle(
    input: &Path,
    subtitle: &Path,
    destination: &Path,
    language: &str,
    title: &str,
) -> Result<()> {
    require_regular_file(input, "video")?;
    require_regular_file(subtitle, "subtitle")?;
    require_extension(subtitle, "srt", "subtitle")?;
    require_extension(destination, "mkv", "output")?;

    let language = language.trim();
    let title = title.trim();
    if language.is_empty() {
        bail!("subtitle language tag cannot be empty");
    }
    if title.is_empty() {
        bail!("subtitle title cannot be empty");
    }

    let source = inspect(input)?;
    let existing_subtitles = source.subtitle_tracks.len();
    prepare_destination(destination, "output video")?;

    with_temporary_output(destination, |temporary| {
        let new_track = existing_subtitles.to_string();
        let output = crate::media_tools::ffmpeg_command()?
            .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
            .arg("-i")
            .arg(input)
            .arg("-i")
            .arg(subtitle)
            .args(["-map", "0", "-map", "1:s:0", "-map_metadata", "0"])
            .args(["-map_chapters", "0", "-c", "copy"])
            .arg(format!("-metadata:s:s:{new_track}"))
            .arg(format!("language={language}"))
            .arg(format!("-metadata:s:s:{new_track}"))
            .arg(format!("title={title}"))
            .arg(format!("-disposition:s:{new_track}"))
            .arg("0")
            .arg(temporary)
            .output()
            .with_context(
                || "could not start ffmpeg; install FFmpeg and ensure ffmpeg is available in PATH",
            )?;
        ensure_success(
            output,
            "ffmpeg",
            &format!("add {} to {}", subtitle.display(), input.display()),
        )?;

        let result = inspect(temporary)?;
        if result.streams != source.streams + 1 {
            bail!(
                "output validation failed: expected {} streams, found {}",
                source.streams + 1,
                result.streams
            );
        }
        if result.subtitle_tracks.len() != existing_subtitles + 1 {
            bail!(
                "output validation failed: expected {} subtitle tracks, found {}",
                existing_subtitles + 1,
                result.subtitle_tracks.len()
            );
        }
        let added = result
            .subtitle_tracks
            .last()
            .context("output validation failed: the added subtitle track is missing")?;
        if added.title.as_deref() != Some(title) {
            bail!("output validation failed: the new subtitle title was not preserved");
        }
        if added.language.as_deref() != Some(language) {
            bail!("output validation failed: the new subtitle language was not preserved");
        }
        Ok(())
    })
}

pub fn default_extract_path(input: &Path, language: &str, output_dir: Option<&Path>) -> PathBuf {
    let directory = output_dir
        .map(Path::to_path_buf)
        .or_else(|| input.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = input.file_stem().unwrap_or_else(|| OsStr::new("subtitle"));
    let mut name = OsString::from(stem);
    name.push(".");
    name.push(language_file_tag(language));
    name.push(".srt");
    directory.join(name)
}

pub fn default_added_path(input: &Path) -> PathBuf {
    let directory = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input.file_stem().unwrap_or_else(|| OsStr::new("video"));
    let mut name = OsString::from(stem);
    name.push(".subtitled.mkv");
    directory.join(name)
}

fn match_score(track: &SubtitleTrack, requested: &str) -> Option<u8> {
    if track
        .title
        .as_deref()
        .is_some_and(|title| title.eq_ignore_ascii_case(requested))
    {
        return Some(100);
    }
    if track
        .language
        .as_deref()
        .is_some_and(|language| language.eq_ignore_ascii_case(requested))
    {
        return Some(90);
    }

    let requested_code = canonical_language(requested)?;
    if track
        .title
        .as_deref()
        .and_then(canonical_language)
        .is_some_and(|code| code == requested_code)
    {
        return Some(80);
    }
    if track
        .language
        .as_deref()
        .and_then(canonical_language)
        .is_some_and(|code| code == requested_code)
    {
        return Some(70);
    }
    None
}

fn canonical_language(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "ar" | "ara" | "arabic" => Some("ara"),
        "bn" | "ben" | "bengali" => Some("ben"),
        "bg" | "bul" | "bulgarian" => Some("bul"),
        "zh" | "zho" | "chi" | "chinese" => Some("zho"),
        "hr" | "hrv" | "croatian" => Some("hrv"),
        "cs" | "ces" | "cze" | "czech" => Some("ces"),
        "da" | "dan" | "danish" => Some("dan"),
        "nl" | "nld" | "dut" | "dutch" => Some("nld"),
        "en" | "eng" | "english" => Some("eng"),
        "fil" | "filipino" => Some("fil"),
        "fi" | "fin" | "finnish" => Some("fin"),
        "fr" | "fra" | "fre" | "french" => Some("fra"),
        "hi" | "hin" | "hindi" => Some("hin"),
        "hu" | "hun" | "hungarian" => Some("hun"),
        "id" | "ind" | "indonesian" => Some("ind"),
        "mk" | "mkd" | "mac" | "macedonian" => Some("mkd"),
        "no" | "nor" | "norwegian" => Some("nor"),
        "pl" | "pol" | "polish" => Some("pol"),
        "pt" | "por" | "portuguese" => Some("por"),
        "ro" | "ron" | "rum" | "romanian" => Some("ron"),
        "ru" | "rus" | "russian" => Some("rus"),
        "sr" | "srp" | "serbian" => Some("srp"),
        "sl" | "slv" | "slovene" | "slovenian" => Some("slv"),
        "es" | "spa" | "spanish" => Some("spa"),
        "sv" | "swe" | "swedish" => Some("swe"),
        "ur" | "urd" | "urdu" => Some("urd"),
        _ => None,
    }
}

fn language_file_tag(language: &str) -> String {
    if canonical_language(language) == Some("eng") {
        return "en".to_owned();
    }

    let slug = language
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "subtitle".to_owned()
    } else {
        slug
    }
}

fn available_tracks(tracks: &[SubtitleTrack]) -> String {
    if tracks.is_empty() {
        return "none".to_owned();
    }
    tracks
        .iter()
        .map(|track| {
            format!(
                "{} (stream {}, language {})",
                track.display_title(),
                track.stream_index,
                track.display_language()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn require_regular_file(path: &Path, kind: &str) -> Result<()> {
    let metadata =
        fs::metadata(path).with_context(|| format!("{kind} does not exist: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("{kind} is not a regular file: {}", path.display());
    }
    Ok(())
}

fn require_extension(path: &Path, expected: &str, kind: &str) -> Result<()> {
    if !path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
    {
        bail!(
            "{kind} must use the .{expected} extension: {}",
            path.display()
        );
    }
    Ok(())
}

fn prepare_destination(path: &Path, kind: &str) -> Result<()> {
    if path.exists() {
        bail!("refusing to replace existing {kind}: {}", path.display());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create directory {}", parent.display()))?;
    }
    Ok(())
}

fn with_temporary_output(
    destination: &Path,
    build: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let temporary = temporary_path(destination);
    let result = build(&temporary);
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    if destination.exists() {
        let _ = fs::remove_file(&temporary);
        bail!(
            "refusing to replace a file created while processing: {}",
            destination.display()
        );
    }

    if let Err(error) = fs::rename(&temporary, destination) {
        let _ = fs::remove_file(&temporary);
        return Err(error).with_context(|| {
            format!(
                "could not publish temporary output as {}",
                destination.display()
            )
        });
    }
    Ok(())
}

fn temporary_path(destination: &Path) -> PathBuf {
    let directory = destination.parent().unwrap_or_else(|| Path::new("."));
    let stem = destination
        .file_stem()
        .unwrap_or_else(|| OsStr::new("output"));
    let extension = destination.extension();
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);

    let mut name = OsString::from(".");
    name.push(stem);
    name.push(format!(".transcripter-{}-{id}", std::process::id()));
    if let Some(extension) = extension {
        name.push(".");
        name.push(extension);
    }
    directory.join(name)
}

fn ensure_success(output: Output, program: &str, action: &str) -> Result<Vec<u8>> {
    if output.status.success() {
        return Ok(output.stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    if detail.is_empty() {
        bail!("{program} failed to {action} with status {}", output.status);
    }
    bail!("{program} failed to {action}: {detail}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(index: usize, title: &str, language: Option<&str>) -> SubtitleTrack {
        SubtitleTrack {
            stream_index: index,
            codec: "subrip".to_owned(),
            language: language.map(str::to_owned),
            title: Some(title.to_owned()),
        }
    }

    #[test]
    fn exact_title_beats_language_aliases() {
        let tracks = vec![
            track(9, "English", Some("eng")),
            track(10, "English 2", Some("eng")),
        ];
        let selected = select_subtitle(&tracks, "English").unwrap();
        assert_eq!(selected.stream_index, 9);
    }

    #[test]
    fn language_code_selects_named_track() {
        let tracks = vec![track(9, "English", None), track(13, "French", None)];
        assert_eq!(select_subtitle(&tracks, "eng").unwrap().stream_index, 9);
        assert_eq!(select_subtitle(&tracks, "fr").unwrap().stream_index, 13);
    }

    #[test]
    fn default_paths_do_not_replace_input() {
        let input = Path::new("/media/Friends S1E1.mkv");
        assert_eq!(
            default_extract_path(input, "English", None),
            PathBuf::from("/media/Friends S1E1.en.srt")
        );
        assert_eq!(
            default_added_path(input),
            PathBuf::from("/media/Friends S1E1.subtitled.mkv")
        );
    }

    #[test]
    fn parses_subtitle_streams_from_ffprobe() {
        let json = br#"{
            "streams": [
                {"index": 0, "codec_name": "h264", "codec_type": "video"},
                {"index": 3, "codec_name": "subrip", "codec_type": "subtitle",
                 "tags": {"language": "eng", "title": "English"}}
            ]
        }"#;
        let info = parse_probe(Path::new("episode.mkv"), json).unwrap();
        assert_eq!(info.streams, 2);
        assert_eq!(info.subtitle_tracks, vec![track(3, "English", Some("eng"))]);
    }
}
