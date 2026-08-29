#!/bin/sh
# Checksum-verified installer for the rune CLI. Downloads the release
# archive for this platform, verifies its SHA-256 before unpacking, and
# installs into ~/.local/bin (override with RUNE_INSTALL_DIR).
set -eu

REPO="runedeck/cli"
INSTALL_DIR="${RUNE_INSTALL_DIR:-$HOME/.local/bin}"

log()  { printf '  \033[32m>\033[0m %s\n' "$1"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$1"; }
fail() { printf '  \033[31mx\033[0m %s\n' "$1" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 \
        || fail "this installer needs '$1'; install it first or download a release archive manually"
}

main() {
    printf '\n  %s\n\n' 'rune installer'
    need curl
    need tar

    os="$(uname -s)"
    arch="$(uname -m)"
    # The release publishes archives for macOS aarch64 and Linux x86_64 only.
    case "$os-$arch" in
        Darwin-arm64|Darwin-aarch64) platform="macos"; cpu="aarch64" ;;
        Linux-x86_64|Linux-amd64)    platform="linux"; cpu="x86_64" ;;
        *) fail "no release archive is published for $os/$arch; see https://github.com/$REPO/releases" ;;
    esac
    archive="rune-cli-$platform-$cpu.tar.gz"
    log "detected $platform/$cpu"

    if command -v sha256sum >/dev/null 2>&1; then
        checksum() { sha256sum "$1" | cut -d' ' -f1; }
    elif command -v shasum >/dev/null 2>&1; then
        checksum() { shasum -a 256 "$1" | cut -d' ' -f1; }
    else
        fail "this installer needs sha256sum or shasum"
    fi

    base="https://github.com/$REPO/releases/latest/download"
    workdir="$(mktemp -d)"
    trap 'rm -rf "$workdir"' EXIT

    log "downloading $archive"
    curl -fsSL --retry 3 --connect-timeout 10 --max-time 300 \
        "$base/$archive" -o "$workdir/$archive" \
        || fail "download failed: $base/$archive"
    curl -fsSL --retry 3 --connect-timeout 10 --max-time 60 \
        "$base/SHA256SUMS" -o "$workdir/SHA256SUMS" \
        || fail "checksum download failed: $base/SHA256SUMS"

    expected="$(awk -v name="$archive" '"'"'$2 == name || $2 == "*" name { print $1; exit }'"'"' "$workdir/SHA256SUMS")"
    actual="$(checksum "$workdir/$archive")"
    [ -n "$expected" ] || fail "SHA256SUMS carries no entry for $archive"
    [ "$expected" = "$actual" ] \
        || fail "checksum mismatch: expected $expected, got $actual; not installing"
    log "SHA-256 verified"

    tar -xzf "$workdir/$archive" -C "$workdir"
    [ -f "$workdir/rune" ] || fail "the archive carries no rune binary"
    mkdir -p "$INSTALL_DIR"
    chmod +x "$workdir/rune"
    mv -f "$workdir/rune" "$INSTALL_DIR/rune"
    log "installed $INSTALL_DIR/rune"

    case ":$PATH:" in
        *:"$INSTALL_DIR":*) ;;
        *)
            warn "$INSTALL_DIR is not on PATH; add this line to your shell config:"
            printf '\n    export PATH="%s:$PATH"\n\n' "$INSTALL_DIR"
            ;;
    esac
    log "next: rune setup"
}

main "$@"
