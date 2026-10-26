#!/bin/sh
# Lay out a rune home in the current directory for HOME=. scenes. Rune reads
# .config/rune/config.yaml under HOME, and the repository ignores files named
# config.yaml, so the fixture keeps the config as rune-config.yaml. The copy
# of bin/claude gets the executable bit here, so the fixture needs none.
fixture=$(dirname "$0")
mkdir -p .config/rune
cp -f "$fixture/rune-config.yaml" .config/rune/config.yaml
cp -f "$fixture/proof.env" proof.env
cp -Rf "$fixture/bin" bin
chmod +x bin/claude
