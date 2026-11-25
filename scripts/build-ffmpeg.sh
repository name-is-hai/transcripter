#!/usr/bin/env bash
set -euo pipefail

FFMPEG_VERSION=8.0.1
FFMPEG_SHA256=05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"

if [[ $# -ne 1 ]]; then
  echo "usage: $0 OUTPUT_PREFIX" >&2
  exit 2
fi

prefix=$(mkdir -p "$1" && cd "$1" && pwd)
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
archive="$work/ffmpeg.tar.xz"

curl --fail --location --silent --show-error "$FFMPEG_URL" --output "$archive"
if command -v sha256sum >/dev/null 2>&1; then
  printf '%s  %s\n' "$FFMPEG_SHA256" "$archive" | sha256sum --check --status
else
  actual=$(shasum -a 256 "$archive" | awk '{print $1}')
  [[ "$actual" == "$FFMPEG_SHA256" ]]
fi

tar -xf "$archive" -C "$work"
cd "$work/ffmpeg-${FFMPEG_VERSION}"

configure_flags=(
  "--prefix=$prefix"
  --disable-autodetect
  --disable-debug
  --disable-doc
  --disable-everything
  --disable-ffplay
  --disable-gpl
  --disable-iconv
  --disable-network
  --disable-nonfree
  --disable-shared
  --disable-x86asm
  --enable-static
  --enable-small
  --enable-ffmpeg
  --enable-ffprobe
  --enable-protocol=file
  --enable-demuxer=matroska
  --enable-demuxer=srt
  --enable-muxer=matroska
  --enable-muxer=srt
  --enable-decoder=subrip
  --enable-encoder=subrip
  --extra-version=transcripter
)

if [[ "${MSYSTEM:-}" == MINGW* || "${MSYSTEM:-}" == UCRT* ]]; then
  configure_flags+=(--extra-ldflags=-static)
fi

./configure "${configure_flags[@]}"

if command -v nproc >/dev/null 2>&1; then
  jobs=$(nproc)
else
  jobs=$(sysctl -n hw.logicalcpu 2>/dev/null || echo 2)
fi
make -j"$jobs"
make install

suffix=""
if [[ -f "$prefix/bin/ffmpeg.exe" ]]; then
  suffix=.exe
fi
"$prefix/bin/ffmpeg$suffix" -version | head -1
"$prefix/bin/ffprobe$suffix" -version | head -1
