# ${TITLE}

> Activate this repository's commit and push checks after cloning.

I want you to finish setting up ${NAME}. Execute all the steps below autonomously.

## OBJECTIVE

Hooks active (git `core.hooksPath` plus the jj `push` alias when the repo is colocated) and the toolchain for this repo installed.

## DONE WHEN

`make validate` exits 0, and `git config core.hooksPath` prints `.githooks`.

## TODO

- [ ] Run `make install`
- [ ] Install missing check tools (`prek` or `gitleaks`, `shellcheck`)
- [ ] Run `make validate` and confirm it exits 0

## Steps

```sh
make install
command -v prek >/dev/null || brew install gitleaks shellcheck
make validate
```

EXECUTE NOW: Complete the above TODO list to achieve: hooks wired and `make validate` green.
