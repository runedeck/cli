#!/usr/bin/env python3
"""Run reviewed checks against frozen inputs and retain external receipts."""

import argparse
import hashlib
import json
import os
import re
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path

VERSION = "rune-artifact-checks/v1"
ENVIRONMENT = ("PATH", "HOME", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME")


class CheckError(ValueError):
    """The frozen check contract could not be satisfied."""


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    result = hashlib.sha256()
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise CheckError(f"Expected a regular file: {path.name}")
        for block in iter(lambda: source.read(65536), b""):
            result.update(block)
    return result.hexdigest()


def pinned(path, expected):
    if not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise CheckError("A pin must be a lowercase SHA-256 digest.")
    data = path.read_bytes()
    if digest(data) != expected:
        raise CheckError(f"The reviewed pin does not match {path.name}.")
    return data


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise CheckError(f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(data):
    return json.loads(data, object_pairs_hook=unique_object)


def relative(value):
    if not isinstance(value, str) or not value or "\\" in value:
        raise CheckError("Use a nonempty relative POSIX path.")
    path = Path(value)
    if path.is_absolute() or any(part in (".", "..", "") for part in value.split("/")):
        raise CheckError(f"Unsafe relative path: {value}")
    return path


def inside(path, root):
    return path == root or root in path.parents


def positive_integer(value):
    return type(value) is int and value > 0


def load_manifest(data):
    manifest = read_json(data)
    if not isinstance(manifest, dict) or set(manifest) != {
        "version",
        "inputs",
        "checks",
    }:
        raise CheckError("The manifest requires only version, inputs, and checks.")
    if manifest["version"] != VERSION:
        raise CheckError("Unsupported manifest version.")
    inputs, checks = manifest["inputs"], manifest["checks"]
    if (
        not isinstance(inputs, list)
        or not inputs
        or not isinstance(checks, list)
        or not checks
    ):
        raise CheckError("Declare at least one input and one check.")
    paths = [relative(value) for value in inputs]
    if len(set(paths)) != len(paths):
        raise CheckError("Duplicate input roots are not permitted.")
    for left in paths:
        if any(left != right and left in right.parents for right in paths):
            raise CheckError("Input roots must not overlap.")
    ids = set()
    for check in checks:
        base = {"id", "argv", "timeout_seconds", "max_output_bytes", "adapter"}
        if not isinstance(check, dict):
            raise CheckError("Each check must be an object.")
        adapter = check.get("adapter")
        extra = {"expected_tests"} if adapter in ("cargo", "unittest") else set()
        if adapter == "layer-report":
            extra = {"report_root"}
        if (
            adapter not in ("command", "cargo", "unittest", "layer-report")
            or set(check) != base | extra
        ):
            raise CheckError("Invalid check adapter or fields.")
        identifier = check["id"]
        if not isinstance(identifier, str) or not re.fullmatch(
            r"[a-z][a-z0-9-]{0,63}", identifier
        ):
            raise CheckError(
                "Use a lowercase check ID with letters, digits, and hyphens."
            )
        if identifier in ids:
            raise CheckError(f"Duplicate check ID: {identifier}")
        ids.add(identifier)
        argv = check["argv"]
        if (
            not isinstance(argv, list)
            or not argv
            or any(not isinstance(arg, str) or "\0" in arg for arg in argv)
            or not argv[0]
        ):
            raise CheckError("Each check requires a nonempty argv array.")
        timeout = check["timeout_seconds"]
        if type(timeout) not in (int, float) or not 0 < timeout <= 3600:
            raise CheckError("A check timeout must be between zero and 3600 seconds.")
        limit = check["max_output_bytes"]
        if not positive_integer(limit) or limit > 16 * 1024 * 1024:
            raise CheckError(
                "A check output limit must be between one byte and 16 MiB."
            )
        if extra == {"expected_tests"} and not positive_integer(
            check["expected_tests"]
        ):
            raise CheckError(
                "Test checks require a positive exact expected_tests count."
            )
        if adapter == "layer-report":
            relative(check["report_root"])
    return manifest


def environment():
    result = {key: os.environ[key] for key in ENVIRONMENT if key in os.environ}
    result.update(LANG="C", LC_ALL="C", NO_COLOR="1", PYTHONDONTWRITEBYTECODE="1")
    return result


def inventory(root, inputs):
    entries = {}

    def visit(path):
        name = path.relative_to(root).as_posix()
        parent = root
        for part in path.relative_to(root).parts[:-1]:
            parent = parent / part
            if parent.is_symlink():
                raise CheckError(f"An input has a symbolic directory ancestor: {name}")
        if not inside(path.parent.resolve(strict=True), root):
            raise CheckError(f"An input parent leaves the source root: {name}")
        info = path.lstat()
        mode = stat.S_IMODE(info.st_mode)
        if stat.S_ISLNK(info.st_mode):
            target = path.resolve(strict=True)
            if not inside(target, root) or not target.is_file():
                raise CheckError(f"Only contained file links are supported: {name}")
            entries[name] = {
                "kind": "symlink",
                "mode": mode,
                "target": os.readlink(path),
                "target_mode": stat.S_IMODE(target.stat().st_mode),
                "sha256": file_digest(target),
            }
        elif stat.S_ISREG(info.st_mode):
            entries[name] = {"kind": "file", "mode": mode, "sha256": file_digest(path)}
        elif stat.S_ISDIR(info.st_mode):
            entries[name] = {"kind": "directory", "mode": mode}
            for child in sorted(path.iterdir()):
                visit(child)
        else:
            raise CheckError(f"Unsupported input entry: {name}")

    for value in inputs:
        visit(root / relative(value))
    return entries


def executables(root, checks, env):
    result = {}
    for check in checks:
        command = check["argv"][0]
        found = (
            str(root / command)
            if "/" in command and not Path(command).is_absolute()
            else command
        )
        found = shutil.which(found, path=env.get("PATH", ""))
        if found is None:
            raise CheckError(f"Missing executable for {check['id']}: {command}")
        path = Path(found).resolve(strict=True)
        if not path.is_file():
            raise CheckError(f"The executable is not a regular file: {command}")
        result[check["id"]] = {
            "path": str(path),
            "sha256": file_digest(path),
            "mode": stat.S_IMODE(path.stat().st_mode),
        }
    return result


def state(root, manifest, options):
    pinned(Path(__file__).resolve(), options.runner_sha256)
    pinned(Path(options.manifest).resolve(), options.manifest_sha256)
    env = environment()
    return {
        "version": VERSION,
        "root": str(root),
        "runner_sha256": options.runner_sha256,
        "manifest_sha256": options.manifest_sha256,
        "inputs": inventory(root, manifest["inputs"]),
        "executables": executables(root, manifest["checks"], env),
        "environment": env,
        "python": {
            "path": str(Path(sys.executable).resolve()),
            "sha256": file_digest(Path(sys.executable).resolve()),
        },
    }


def external_path(value, root):
    path = Path(value).absolute()
    if path.is_symlink() or inside(path.resolve(), root):
        raise CheckError("Store snapshots and receipts outside the source root.")
    return path


def write_json(path, value):
    with path.open("x", encoding="utf-8") as destination:
        json.dump(value, destination, sort_keys=True, indent=2)
        destination.write("\n")


def write_receipt_file(directory, descriptor, name, data):
    expected, current = os.fstat(descriptor), directory.lstat()
    if (expected.st_dev, expected.st_ino) != (
        current.st_dev,
        current.st_ino,
    ) or not stat.S_ISDIR(current.st_mode):
        raise CheckError("The receipt directory changed during execution.")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    file_descriptor = os.open(name, flags, 0o600, dir_fd=descriptor)
    with os.fdopen(file_descriptor, "wb") as destination:
        destination.write(data)


def stop(process):
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    else:
        process.kill()
    process.wait()


def capture(argv, root, env, timeout, limit):
    started = time.monotonic()
    process = subprocess.Popen(
        argv,
        cwd=root,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    output = bytearray()
    fault = None
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        while selector.get_map():
            remaining = timeout - (time.monotonic() - started)
            if remaining <= 0:
                fault = "timeout"
                break
            for key, _ in selector.select(min(remaining, 0.1)):
                block = os.read(key.fileobj.fileno(), 65536)
                if not block:
                    selector.unregister(key.fileobj)
                    continue
                available = limit - len(output)
                output.extend(block[:available])
                if len(block) > available:
                    fault = "output-limit"
                    break
            if fault:
                break
    if fault:
        stop(process)
    else:
        try:
            process.wait(timeout=max(0.001, timeout - (time.monotonic() - started)))
        except subprocess.TimeoutExpired:
            fault = "timeout"
            stop(process)
    if os.name == "posix":
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            pass
        else:
            stop(process)
            fault = fault or "background-process"
    process.stdout.close()
    return bytes(output), process.returncode, fault


def assess(check, raw, exit_code, root):
    if exit_code != 0:
        raise CheckError(f"The command exited {exit_code}.")
    text = raw.decode("utf-8")
    adapter = check["adapter"]
    if adapter == "command":
        return {"kind": "command", "test_count": None}
    if adapter == "layer-report":
        report = read_json(text)
        expected_root = (root / relative(check["report_root"])).resolve(strict=True)
        if (
            not isinstance(report, dict)
            or report.get("version") != "rune-skill-source-layers/v1"
            or report.get("valid") is not True
            or report.get("findings") != []
            or not positive_integer(report.get("checked"))
            or report.get("root") != str(expected_root)
        ):
            raise CheckError(
                "The source-layer report is invalid, empty, or for another root."
            )
        return {"kind": adapter, "checked": report["checked"]}
    if adapter == "cargo":
        summaries = re.findall(
            r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
            text,
            re.MULTILINE,
        )
        if not summaries or any(
            status != "ok" or int(failed) or int(ignored) or int(measured)
            for status, _, failed, ignored, measured, _ in summaries
        ):
            raise CheckError(
                "Cargo results are missing, failed, ignored, or measured without tests."
            )
        count = sum(int(passed) for _, passed, _, _, _, _ in summaries)
    else:
        summaries = re.findall(r"^Ran (\d+) tests? in [^\n]+$", text, re.MULTILINE)
        if (
            len(summaries) != 1
            or not re.search(r"^OK$", text, re.MULTILINE)
            or re.search(r"^OK \(|^FAILED", text, re.MULTILINE)
        ):
            raise CheckError("Unittest results are missing, failed, or skipped.")
        count = int(summaries[0])
    if count != check["expected_tests"]:
        raise CheckError(f"Expected {check['expected_tests']} tests, observed {count}.")
    return {"kind": adapter, "test_count": count}


def execute(options):
    pinned(Path(__file__).resolve(), options.runner_sha256)
    root = Path(options.root).resolve(strict=True)
    if not root.is_dir():
        raise CheckError("The source root must be a directory.")
    manifest = load_manifest(pinned(Path(options.manifest), options.manifest_sha256))
    output = external_path(options.output, root)
    if options.action == "freeze":
        before = state(root, manifest, options)
        if state(root, manifest, options) != before:
            raise CheckError(
                "Inputs changed while freezing. Freeze a stable source tree."
            )
        write_json(output, before)
        print(json.dumps({"snapshot": str(output), "sha256": file_digest(output)}))
        return 0
    snapshot_path = external_path(options.snapshot, root)
    frozen = read_json(pinned(snapshot_path, options.snapshot_sha256))
    output.mkdir(exist_ok=False)
    descriptor = os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    receipt = {
        "version": VERSION,
        "snapshot_sha256": options.snapshot_sha256,
        "runner_sha256": options.runner_sha256,
        "manifest_sha256": options.manifest_sha256,
        "snapshot": frozen,
        "required_check_ids": [check["id"] for check in manifest["checks"]],
        "checks": [],
        "accepted": False,
    }
    try:
        if state(root, manifest, options) != frozen:
            raise CheckError(
                "The frozen inputs, environment, or executables are stale."
            )
        for check in manifest["checks"]:
            if state(root, manifest, options) != frozen:
                raise CheckError("Inputs changed before the next check.")
            argv = [frozen["executables"][check["id"]]["path"], *check["argv"][1:]]
            result = {"id": check["id"], "argv": argv, "accepted": False}
            receipt["checks"].append(result)
            raw, code, fault = capture(
                argv,
                root,
                frozen["environment"],
                check["timeout_seconds"],
                check["max_output_bytes"],
            )
            log = output / f"{check['id']}.log"
            write_receipt_file(output, descriptor, log.name, raw)
            result.update(
                exit_code=code, output_sha256=digest(raw), output_file=log.name
            )
            if fault:
                raise CheckError(f"{check['id']}: {fault}")
            result["observation"] = assess(check, raw, code, root)
            if state(root, manifest, options) != frozen:
                raise CheckError(f"{check['id']}: inputs changed during the check.")
            result["accepted"] = True
        if state(root, manifest, options) != frozen:
            raise CheckError("Inputs changed after the checks.")
        receipt["accepted"] = True
    except (OSError, ValueError, RuntimeError) as error:
        receipt["error"] = str(error)
    finally:
        try:
            write_receipt_file(
                output, descriptor, "receipt.json", encoded(receipt) + b"\n"
            )
        finally:
            os.close(descriptor)
    print(
        json.dumps(
            {"receipt": str(output / "receipt.json"), "accepted": receipt["accepted"]}
        )
    )
    return 0 if receipt["accepted"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    subparsers = parser.add_subparsers(dest="action", required=True)
    for action in ("freeze", "run"):
        command = subparsers.add_parser(action, allow_abbrev=False)
        for name in ("root", "manifest", "runner-sha256", "manifest-sha256", "output"):
            command.add_argument(f"--{name}", required=True)
        if action == "run":
            command.add_argument("--snapshot", required=True)
            command.add_argument("--snapshot-sha256", required=True)
    try:
        return execute(parser.parse_args())
    except (OSError, ValueError, RuntimeError) as error:
        print(json.dumps({"accepted": False, "error": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
