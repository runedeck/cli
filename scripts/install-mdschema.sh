#!/usr/bin/env bash
set -euo pipefail

readonly version="0.13.4"
# Pin the release asset digest published at https://github.com/jackchuka/mdschema/releases/tag/v0.13.4.
readonly expected_sha256="b4d86b3c273172aca0380f6c3e5a23f731cfc5576633b467f6c5f923e48180e3"
readonly target_directory=${1:-"$HOME/.local/bin"}
readonly archive_name="mdschema_${version}_linux_amd64.tar.gz"
readonly release_url="https://github.com/jackchuka/mdschema/releases/download/v${version}/${archive_name}"
temporary_directory=$(mktemp -d)
trap 'command rm -rf "$temporary_directory"' EXIT

curl -fsSL "$release_url" -o "$temporary_directory/$archive_name"
actual_sha256=$(sha256sum "$temporary_directory/$archive_name")
actual_sha256=${actual_sha256%% *}
if [ "$actual_sha256" != "$expected_sha256" ]; then
    echo "mdschema archive hash mismatch" >&2
    exit 1
fi

tar -xzf "$temporary_directory/$archive_name" -C "$temporary_directory" mdschema
mkdir -p "$target_directory"
install -m 0755 "$temporary_directory/mdschema" "$target_directory/mdschema"
