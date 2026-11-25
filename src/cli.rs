use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Inspect, extract, and add subtitle tracks without re-encoding video.
#[derive(Debug, Parser)]
#[command(name = "transcripter", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List every embedded subtitle track.
    Inspect {
        /// MKV files to inspect.
        #[arg(required = true, value_name = "VIDEO")]
        videos: Vec<PathBuf>,
    },

    /// Extract an embedded subtitle track to an SRT file.
    Extract {
        /// MKV files from which to extract subtitles.
        #[arg(required = true, value_name = "VIDEO")]
        videos: Vec<PathBuf>,

        /// Track title, language name, or language code.
        #[arg(short, long, default_value = "English")]
        language: String,

        /// Put all generated SRT files in this directory.
        #[arg(short, long, value_name = "DIRECTORY")]
        output_dir: Option<PathBuf>,
    },

    /// Add an external SRT as a selectable track in a new MKV.
    Add {
        /// Source MKV file.
        #[arg(value_name = "VIDEO")]
        video: PathBuf,

        /// External SRT file to add.
        #[arg(value_name = "SUBTITLE")]
        subtitle: PathBuf,

        /// Destination MKV. Defaults to VIDEO.subtitled.mkv.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Matroska language tag for the new track.
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Display title for the new track.
        #[arg(short, long, default_value = "English")]
        title: String,
    },

    /// Print licenses and source information for bundled components.
    Licenses,
}
