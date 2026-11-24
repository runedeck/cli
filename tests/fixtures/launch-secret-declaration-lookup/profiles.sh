#!/bin/sh
# Lay out a rune home for the secrets_profile scene in the current directory.
# It starts from home.sh, removes the secretspec stub so the real secretspec
# on PATH answers, and puts profiles.toml in place of the default secret
# declaration. Then it stores one fake value per profile in the file store.
# HOME=. keeps secretspec's own config and audit log in the scene directory.
fixture=$(dirname "$0")
sh "$fixture/home.sh" || exit 1
rm -f bin/secretspec
cp -f "$fixture/profiles.toml" .config/rune/secretspec.toml
for profile in owner agents; do
    HOME=. secretspec set PROOF_ORIGIN "fake-$profile-value" \
        --file .config/rune/secretspec.toml --profile "$profile" \
        --reason "the launch-secret-declaration-lookup proof stores a fake value" \
        > /dev/null 2>&1 || exit 1
done
