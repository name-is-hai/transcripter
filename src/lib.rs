pub mod cli;
#[cfg(feature = "gui-core")]
pub mod gui;
mod media;
mod media_tools;

pub const THIRD_PARTY_NOTICE: &str = concat!(
    "Transcripter is distributed under the MIT License:\n\n",
    include_str!("../LICENSE"),
    "\nGPUI notice:\n",
    "Transcripter uses a minimally patched vendored copy of official GPUI 0.2.2 under Apache-2.0.\n",
    "Original package: https://crates.io/crates/gpui/0.2.2\n",
    "Patch: replace the Windows u_strlen import with an equivalent Rust UTF-16 terminator scan.\n\n",
    include_str!("../licenses/GPUI-APACHE-2.0.txt"),
    "\nFFmpeg notice:\n",
    "Transcripter release builds include FFmpeg 8.0.1, used as separate helper programs under LGPL-2.1-or-later.\n",
    "FFmpeg source: https://ffmpeg.org/releases/ffmpeg-8.0.1.tar.xz\n",
    "Source SHA-256: 05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41\n",
    "Build configuration: scripts/build-ffmpeg.sh\n",
    "FFmpeg project: https://ffmpeg.org/\n\n",
    include_str!("../licenses/FFmpeg-LGPL-2.1.txt")
);

use anyhow::{Context as _, Result, bail};
use cli::{Cli, Command};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub use media::{MediaInfo, SubtitleTrack};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Extraction {
    pub video: PathBuf,
    pub subtitle: PathBuf,
    pub stream_index: usize,
    pub track_title: String,
}

pub fn inspect_video(path: impl AsRef<Path>) -> Result<MediaInfo> {
    media::inspect(path.as_ref())
}

pub fn inspect_videos(paths: &[PathBuf]) -> Result<Vec<MediaInfo>> {
    paths
        .iter()
        .map(|path| media::inspect(path))
        .collect::<Result<Vec<_>>>()
}

pub fn extract_subtitles(
    videos: &[PathBuf],
    language: &str,
    output_dir: Option<&Path>,
) -> Result<Vec<Extraction>> {
    if videos.is_empty() {
        bail!("at least one video is required");
    }
    if let Some(directory) = output_dir
        && directory.exists()
        && !directory.is_dir()
    {
        bail!(
            "output directory is not a directory: {}",
            directory.display()
        );
    }

    struct Job {
        video: PathBuf,
        track: SubtitleTrack,
        output: PathBuf,
    }

    let mut jobs = Vec::with_capacity(videos.len());
    let mut destinations = HashSet::new();
    for video in videos {
        let info = media::inspect(video)?;
        let track = media::select_subtitle(&info.subtitle_tracks, language)?.clone();
        let output = media::default_extract_path(video, language, output_dir);
        if output.exists() {
            bail!(
                "refusing to replace existing subtitle: {}",
                output.display()
            );
        }
        if !destinations.insert(output.clone()) {
            bail!(
                "more than one input would produce the same subtitle: {}",
                output.display()
            );
        }
        jobs.push(Job {
            video: video.clone(),
            track,
            output,
        });
    }

    let mut completed = Vec::with_capacity(jobs.len());
    for job in jobs {
        media::extract_subtitle(&job.video, &job.track, &job.output)
            .with_context(|| format!("could not extract subtitles from {}", job.video.display()))?;
        completed.push(Extraction {
            video: job.video,
            subtitle: job.output,
            stream_index: job.track.stream_index,
            track_title: job.track.display_title().to_owned(),
        });
    }
    Ok(completed)
}

pub fn add_subtitle(
    video: impl AsRef<Path>,
    subtitle: impl AsRef<Path>,
    output: Option<&Path>,
    language: &str,
    title: &str,
) -> Result<PathBuf> {
    let video = video.as_ref();
    let subtitle = subtitle.as_ref();
    let destination = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| media::default_added_path(video));

    media::add_external_subtitle(video, subtitle, &destination, language, title)?;
    Ok(destination)
}

pub fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Inspect { videos } => {
            for (position, video) in videos.iter().enumerate() {
                let info = inspect_video(video)?;
                if position > 0 {
                    println!();
                }
                println!("{}", info.path.display());
                if info.subtitle_tracks.is_empty() {
                    println!("  No embedded subtitle tracks");
                    continue;
                }
                for track in info.subtitle_tracks {
                    println!(
                        "  stream {:>2}  codec={:<8} language={:<8} title={}",
                        track.stream_index,
                        track.codec,
                        track.display_language(),
                        track.display_title()
                    );
                }
            }
        }
        Command::Extract {
            videos,
            language,
            output_dir,
        } => {
            for result in extract_subtitles(&videos, &language, output_dir.as_deref())? {
                println!(
                    "Extracted '{}' (stream {}) from {}\n  -> {}",
                    result.track_title,
                    result.stream_index,
                    result.video.display(),
                    result.subtitle.display()
                );
            }
        }
        Command::Add {
            video,
            subtitle,
            output,
            language,
            title,
        } => {
            let destination =
                add_subtitle(&video, &subtitle, output.as_deref(), &language, &title)?;
            println!(
                "Added {} to {}\n  -> {}",
                subtitle.display(),
                video.display(),
                destination.display()
            );
        }
        Command::Licenses => print!("{THIRD_PARTY_NOTICE}"),
    }
    Ok(())
}
