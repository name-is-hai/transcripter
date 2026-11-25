#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 TRANSCRIPTER_BINARY OUTPUT.run" >&2
  exit 2
fi

binary=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
output=$(mkdir -p "$(dirname "$2")" && cd "$(dirname "$2")" && pwd)/$(basename "$2")
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
root="$work/root"
mkdir -p "$root/usr/bin" "$root/usr/lib"
cp "$binary" "$root/usr/bin/transcripter"
chmod 755 "$root/usr/bin/transcripter"

declare -A copied=()
queue=("$root/usr/bin/transcripter")
while ((${#queue[@]})); do
  object=${queue[0]}
  queue=("${queue[@]:1}")
  while IFS= read -r library; do
    [[ -n "$library" && -f "$library" ]] || continue
    base=$(basename "$library")
    case "$base" in
      ld-linux*.so*|libc.so.*|libdl.so.*|libgcc_s.so.*|libm.so.*|libpthread.so.*|libresolv.so.*|librt.so.*|libstdc++.so.*)
        continue
        ;;
    esac
    [[ -z "${copied[$base]:-}" ]] || continue
    copied[$base]=1
    cp -L "$library" "$root/usr/lib/$base"
    chmod 755 "$root/usr/lib/$base"
    queue+=("$root/usr/lib/$base")
  done < <(
    ldd "$object" 2>/dev/null \
      | awk '/=> \// { print $3 } /^[[:space:]]*\// { print $1 }' \
      | sort -u
  )
done

cat > "$root/AppRun" <<'APP_RUN'
#!/bin/sh
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$HERE/usr/bin/transcripter" "$@"
APP_RUN
chmod 755 "$root/AppRun"

tar -C "$root" -czf "$work/payload.tar.gz" .
payload_sha=$(sha256sum "$work/payload.tar.gz" | awk '{print $1}')

cat > "$output" <<EOF
#!/bin/sh
set -eu
PAYLOAD_SHA='$payload_sha'
archive_line=\$(awk '/^__TRANSCRIPTER_PAYLOAD_BELOW__\$/ { print NR + 1; exit }' "\$0")
if [ -z "\$archive_line" ]; then
  echo 'Transcripter payload marker is missing.' >&2
  exit 1
fi
cache_root=\${XDG_CACHE_HOME:-"\${HOME:-/tmp}/.cache"}
destination="\$cache_root/transcripter/releases/\$PAYLOAD_SHA"
if [ ! -x "\$destination/AppRun" ]; then
  umask 077
  mkdir -p "\$(dirname "\$destination")"
  if [ -e "\$destination" ]; then
    rm -rf -- "\$destination"
  fi
  temporary="\$destination.tmp.\$\$"
  rm -rf -- "\$temporary"
  mkdir -p "\$temporary"
  if command -v sha256sum >/dev/null 2>&1; then
    actual=\$(tail -n +"\$archive_line" "\$0" | sha256sum | awk '{print \$1}')
    if [ "\$actual" != "\$PAYLOAD_SHA" ]; then
      echo 'Transcripter payload verification failed.' >&2
      rm -rf -- "\$temporary"
      exit 1
    fi
  fi
  tail -n +"\$archive_line" "\$0" | tar -xzf - -C "\$temporary"
  if ! mv -T "\$temporary" "\$destination" 2>/dev/null; then
    rm -rf -- "\$temporary"
  fi
fi
exec "\$destination/AppRun" "\$@"
__TRANSCRIPTER_PAYLOAD_BELOW__
EOF
cat "$work/payload.tar.gz" >> "$output"
chmod 755 "$output"

printf 'Created %s (%s bytes, %s bundled libraries)\n' \
  "$output" "$(wc -c < "$output")" "${#copied[@]}"
