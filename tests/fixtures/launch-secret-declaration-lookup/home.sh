#!/bin/sh
# Lay out a rune home in the current directory for HOME=. scenes: the rune
# config, the default secret declaration beside it, an env file at ./.env,
# and stubs for secretspec and claude in bin/. The repository ignores files
# named config.yaml and .env, so the fixture keeps them as rune-config.yaml
# and home.env. The copies of the stubs get the executable bit here.
fixture=$(dirname "$0")
mkdir -p .config/rune
cp -f "$fixture/rune-config.yaml" .config/rune/config.yaml
cp -f "$fixture/secretspec.toml" .config/rune/secretspec.toml
cp -f "$fixture/home.env" .env
cp -Rf "$fixture/bin" bin
chmod +x bin/secretspec bin/claude
