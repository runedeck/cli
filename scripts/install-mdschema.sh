#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=tool-versions
source "$script_directory/tool-versions"
readonly MDSCHEMA_VERSION
readonly target_directory=${1:-"$HOME/.local/bin"}

archive_digest() {
    case "$1:$2" in
        darwin:amd64) printf '%s\n' "$MDSCHEMA_DARWIN_AMD64_SHA256" ;;
        darwin:arm64) printf '%s\n' "$MDSCHEMA_DARWIN_ARM64_SHA256" ;;
        linux:amd64) printf '%s\n' "$MDSCHEMA_LINUX_AMD64_SHA256" ;;
        linux:arm64) printf '%s\n' "$MDSCHEMA_LINUX_ARM64_SHA256" ;;
        *) echo "missing reviewed mdschema digest for $1/$2" >&2; return 1 ;;
    esac
}

case "$(uname -s)" in
    Darwin) system_name=darwin ;;
    Linux) system_name=linux ;;
    *) echo "unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    arm64|aarch64) machine_name=arm64 ;;
    x86_64|amd64) machine_name=amd64 ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

readonly system_name machine_name
readonly archive_name="mdschema_${MDSCHEMA_VERSION}_${system_name}_${machine_name}.tar.gz"
readonly archive_url="https://github.com/jackchuka/mdschema/releases/download/v${MDSCHEMA_VERSION}/${archive_name}"
expected_digest=$(archive_digest "$system_name" "$machine_name")
readonly expected_digest
temporary_directory=$(mktemp -d)
trap 'command rm -rf "$temporary_directory"' EXIT

curl -fsSL "$archive_url" -o "$temporary_directory/$archive_name"
printf '%s  %s\n' "$expected_digest" "$archive_name" > "$temporary_directory/selected-checksum"
(
    cd "$temporary_directory"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum --check selected-checksum
    else
        shasum -a 256 --check selected-checksum
    fi
)

tar -xzf "$temporary_directory/$archive_name" -C "$temporary_directory" mdschema
mkdir -p "$target_directory"
install -m 0755 "$temporary_directory/mdschema" "$target_directory/mdschema"
