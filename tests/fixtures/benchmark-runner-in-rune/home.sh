#!/bin/sh
# Lay out a rune home and a bench workspace in the current directory for
# HOME=. scenes. Rune reads .config/rune/config.yaml under HOME, and the
# repository ignores files named config.yaml, so the fixture keeps the config
# as rune-config.yaml. The config names ./workspace as the bench checkout.
# The stub binaries get their executable bit on the copy, so the fixture
# itself needs no mode bits.
fixture=$(dirname "$0")
mkdir -p .config/rune
cp -f "$fixture/rune-config.yaml" .config/rune/config.yaml
cp -Rf "$fixture/workspace" workspace
cp -Rf "$fixture/bin" bin
chmod +x bin/*
