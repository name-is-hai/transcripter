use std::env;
use std::path::{Path, PathBuf};

const FFMPEG_VERSION: &str = "8.0.1";

fn main() {
    println!("cargo:rerun-if-env-changed=TRANSCRIPTER_FFMPEG_BIN");
    println!("cargo:rerun-if-env-changed=TRANSCRIPTER_FFPROBE_BIN");

    if env::var_os("CARGO_FEATURE_BUNDLED_MEDIA_TOOLS").is_none() {
        return;
    }

    export_tool("TRANSCRIPTER_FFMPEG_BIN", "TRANSCRIPTER_EMBEDDED_FFMPEG");
    export_tool("TRANSCRIPTER_FFPROBE_BIN", "TRANSCRIPTER_EMBEDDED_FFPROBE");
    println!("cargo:rustc-env=TRANSCRIPTER_FFMPEG_VERSION={FFMPEG_VERSION}");
}

fn export_tool(input_variable: &str, output_variable: &str) {
    let path = env::var_os(input_variable).unwrap_or_else(|| {
        panic!("feature 'bundled-media-tools' requires the {input_variable} environment variable")
    });
    let path = absolute_path(Path::new(&path));
    let metadata = path
        .metadata()
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    assert!(
        metadata.is_file(),
        "embedded media tool is not a file: {}",
        path.display()
    );

    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rustc-env={output_variable}={}", path.display());
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    env::current_dir()
        .expect("current build directory should be available")
        .join(path)
}
