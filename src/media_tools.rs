use anyhow::Result;
use std::process::Command;

#[cfg(feature = "bundled-media-tools")]
use anyhow::{Context as _, bail};
#[cfg(feature = "bundled-media-tools")]
use sha2::{Digest as _, Sha256};
#[cfg(feature = "bundled-media-tools")]
use std::ffi::OsString;
#[cfg(feature = "bundled-media-tools")]
use std::fs::{self, File, OpenOptions};
#[cfg(feature = "bundled-media-tools")]
use std::io::{Read as _, Write as _};
#[cfg(feature = "bundled-media-tools")]
use std::path::{Path, PathBuf};
#[cfg(feature = "bundled-media-tools")]
use std::sync::OnceLock;
#[cfg(feature = "bundled-media-tools")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "bundled-media-tools")]
const FFMPEG_BYTES: &[u8] = include_bytes!(env!("TRANSCRIPTER_EMBEDDED_FFMPEG"));
#[cfg(feature = "bundled-media-tools")]
const FFPROBE_BYTES: &[u8] = include_bytes!(env!("TRANSCRIPTER_EMBEDDED_FFPROBE"));

#[cfg(feature = "bundled-media-tools")]
static FFMPEG_PATH: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();
#[cfg(feature = "bundled-media-tools")]
static FFPROBE_PATH: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();

pub fn ffmpeg_command() -> Result<Command> {
    command("ffmpeg")
}

pub fn ffprobe_command() -> Result<Command> {
    command("ffprobe")
}

#[cfg(not(feature = "bundled-media-tools"))]
fn command(name: &str) -> Result<Command> {
    Ok(Command::new(name))
}

#[cfg(feature = "bundled-media-tools")]
fn command(name: &str) -> Result<Command> {
    let installed = match name {
        "ffmpeg" => {
            FFMPEG_PATH.get_or_init(|| install(name, FFMPEG_BYTES).map_err(|e| e.to_string()))
        }
        "ffprobe" => {
            FFPROBE_PATH.get_or_init(|| install(name, FFPROBE_BYTES).map_err(|e| e.to_string()))
        }
        _ => unreachable!("unknown bundled media tool"),
    };

    match installed {
        Ok(path) => Ok(Command::new(path)),
        Err(error) => bail!("could not prepare bundled {name}: {error}"),
    }
}

#[cfg(feature = "bundled-media-tools")]
fn install(name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let digest = Sha256::digest(bytes);
    let digest_hex = format!("{digest:x}");
    let directory = cache_directory().join(env!("TRANSCRIPTER_FFMPEG_VERSION"));
    create_private_directory(&directory)?;

    let mut filename = OsString::from(format!("{name}-{}", &digest_hex[..16]));
    if cfg!(windows) {
        filename.push(".exe");
    }
    let destination = directory.join(filename);

    if destination.is_file() && file_matches(&destination, &digest)? {
        ensure_executable(&destination)?;
        return Ok(destination);
    }

    if destination.exists() {
        fs::remove_file(&destination).with_context(|| {
            format!(
                "could not replace invalid cached helper {}",
                destination.display()
            )
        })?;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = directory.join(format!(".{name}-{}-{timestamp}.tmp", std::process::id()));
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("could not create {}", temporary.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("could not write {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("could not sync {}", temporary.display()))?;
        ensure_executable(&temporary)?;
        fs::rename(&temporary, &destination).with_context(|| {
            format!(
                "could not publish bundled helper as {}",
                destination.display()
            )
        })?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result?;

    if !file_matches(&destination, &digest)? {
        let _ = fs::remove_file(&destination);
        bail!(
            "cached helper failed verification: {}",
            destination.display()
        );
    }
    Ok(destination)
}

#[cfg(feature = "bundled-media-tools")]
fn cache_directory() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(path).join("Transcripter").join("tools");
    }

    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("Transcripter")
            .join("tools");
    }

    #[cfg(not(target_os = "windows"))]
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(path).join("transcripter").join("tools");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("transcripter")
            .join("tools");
    }

    std::env::temp_dir().join("transcripter").join("tools")
}

#[cfg(feature = "bundled-media-tools")]
fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("could not create helper cache {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("could not secure helper cache {}", path.display()))?;
    }
    Ok(())
}

#[cfg(feature = "bundled-media-tools")]
fn ensure_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("could not make {} executable", path.display()))?;
    }
    Ok(())
}

#[cfg(feature = "bundled-media-tools")]
fn file_matches(path: &Path, expected: &[u8]) -> Result<bool> {
    let mut file = File::open(path)
        .with_context(|| format!("could not verify cached helper {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("could not read cached helper {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().as_slice() == expected)
}
