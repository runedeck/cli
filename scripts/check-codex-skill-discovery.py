#!/usr/bin/env python3
"""Observe native Codex skill discovery through the app-server protocol.

Catalog collection does not start a model turn. A probe explicitly invokes one
harmless skill and requests one exact read of its companion. The adapter never
accepts model prose as discovery or access evidence. It reads no credentials.
"""

import argparse
import hashlib
import json
import os
import selectors
import shlex
import signal
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path


def json_line(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode() + b"\n"


def sha256(value):
    return hashlib.sha256(value).hexdigest()


class ObservationError(RuntimeError):
    """A required native observation is absent or invalid."""


class NativePeer:
    """Bound runtime and output for one independent app-server process."""

    def __init__(self, executable, cwd, timeout=180, max_bytes=4 * 1024 * 1024, codex_home=None, proxy=False):
        environment = dict(os.environ)
        if codex_home is not None:
            environment["CODEX_HOME"] = str(codex_home)
        self.process = subprocess.Popen(
            [str(executable), "app-server", "proxy" if proxy else "--stdio"], cwd=cwd, env=environment,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True,
        )
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ, "stdout")
        self.selector.register(self.process.stderr, selectors.EVENT_READ, "stderr")
        self.pending = b""
        self.queue = []
        self.raw = []
        self.deadline = time.monotonic() + timeout
        self.max_bytes = max_bytes
        self.received = 0
        self.next_id = 1
        self.stderr_bytes = 0

    def record(self, direction, message):
        record = {"direction": direction, "message": message}
        self.raw.append(record)
        return len(self.raw) - 1

    def send(self, message):
        self.record("client", message)
        self.process.stdin.write(json_line(message))
        self.process.stdin.flush()

    def read(self):
        while not self.queue:
            remaining = self.deadline - time.monotonic()
            if remaining <= 0:
                raise ObservationError("native process exceeded its time limit")
            if not self.selector.get_map():
                raise ObservationError("native process ended before the required observation")
            for key, _ in self.selector.select(min(remaining, 1)):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    self.selector.unregister(key.fileobj)
                    continue
                self.received += len(chunk)
                if self.received > self.max_bytes:
                    raise ObservationError("native output exceeded its byte limit")
                if key.data == "stderr":
                    self.stderr_bytes += len(chunk)
                    continue
                self.pending += chunk
                while b"\n" in self.pending:
                    line, self.pending = self.pending.split(b"\n", 1)
                    try:
                        message = json.loads(line)
                    except (ValueError, UnicodeError) as error:
                        raise ObservationError("native stdout is not JSONL") from error
                    if not isinstance(message, dict):
                        raise ObservationError("native stdout record is not an object")
                    index = self.record("server", message)
                    self.queue.append((index, message))
        index, message = self.queue.pop(0)
        if "id" in message and "method" in message:
            # A discovery probe grants no additional command or permission authority.
            if message["method"] == "item/commandExecution/requestApproval":
                self.send({"id": message["id"], "result": {"decision": "decline"}})
            else:
                self.send({"id": message["id"], "error": {"code": -32601, "message": "Unsupported probe request"}})
        return index, message

    def request(self, method, params):
        request_id = self.next_id
        self.next_id += 1
        self.send({"id": request_id, "method": method, "params": params})
        deferred = []
        while True:
            index, message = self.read()
            if message.get("id") == request_id and "method" not in message:
                if "error" in message:
                    code = message["error"].get("code", "unknown")
                    raise ObservationError(f"native {method} failed with protocol code {code}")
                if not isinstance(message.get("result"), dict):
                    raise ObservationError(f"native {method} returned malformed output")
                self.queue = deferred + self.queue
                return index, message["result"]
            if "method" in message and "id" not in message:
                deferred.append((index, message))

    def close(self):
        self.selector.close()
        if self.process.poll() is None:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.process.wait(timeout=3)
        for stream in [self.process.stdin, self.process.stdout, self.process.stderr]:
            stream.close()


def normalize_catalog(response, cwd):
    data = response.get("data")
    if not isinstance(data, list):
        raise ObservationError("native catalog has no data array")
    if not all(isinstance(entry, dict) for entry in data):
        raise ObservationError("native catalog data contains a malformed entry")
    matching = [entry for entry in data if entry.get("cwd") == cwd]
    if len(matching) != 1 or matching[0].get("errors") != []:
        raise ObservationError("native catalog is incomplete or reports errors")
    skills = matching[0].get("skills")
    if not isinstance(skills, list):
        raise ObservationError("native catalog has no skill array")
    catalog = []
    for skill in skills:
        if not isinstance(skill, dict) or not isinstance(skill.get("name"), str) or not isinstance(skill.get("path"), str) or type(skill.get("enabled")) is not bool:
            raise ObservationError("native catalog contains a malformed entry")
        path = Path(skill["path"])
        if not path.is_absolute() or path.name != "SKILL.md":
            raise ObservationError("native catalog path is not an absolute SKILL.md")
        catalog.append({key: skill[key] for key in ["name", "path", "enabled"]})
    return sorted(catalog, key=lambda entry: (entry["name"], entry["path"], entry["enabled"]))


def exact_read_command(command, path):
    """Accept only a literal cat invocation, with an optional shell or RTK wrapper."""
    try:
        words = shlex.split(command)
        if len(words) == 3 and Path(words[0]).name in ["sh", "bash", "zsh"] and words[1] in ["-c", "-lc"]:
            words = shlex.split(words[2])
        if words[:2] == ["rtk", "proxy"]:
            words = words[2:]
    except ValueError:
        return False
    expected = [["cat", "--", str(path)], ["/bin/cat", "--", str(path)]]
    # Shell operators and substitutions cannot be accepted through shlex normalization.
    canonical = [shlex.join(argv) for argv in expected]
    canonical += ["rtk proxy " + value for value in canonical]
    wrappers = [f"{shell} {flag} {shlex.quote(value)}" for shell in ["/bin/sh", "/bin/bash", "/bin/zsh", "sh", "bash", "zsh"] for flag in ["-c", "-lc"] for value in canonical]
    return words in expected and command in canonical + wrappers


def normalize_access(message, session_id, turn_id, skill_path, companion_path, expected_bytes):
    if message.get("method") != "item/completed":
        return None
    params = message.get("params", {})
    item = params.get("item", {})
    if params.get("threadId") != session_id or params.get("turnId") != turn_id:
        return None
    if item.get("type") != "commandExecution" or item.get("status") != "completed" or item.get("exitCode") != 0:
        return None
    if not isinstance(params.get("completedAtMs"), int) or not isinstance(item.get("id"), str):
        return None
    actual_path = Path(skill_path).parent / companion_path
    if not exact_read_command(item.get("command", ""), actual_path):
        return None
    output = item.get("aggregatedOutput")
    if not isinstance(output, str) or output.encode() != expected_bytes:
        return None
    return {
        "type": "tool_access", "event_id": item["id"], "session_id": session_id,
        "skill_path": skill_path, "companion_path": companion_path,
        "content_sha256": sha256(expected_bytes), "status": "completed",
    }


def bind_event(event, index, raw):
    return {**event, "raw_event_index": index, "raw_event_sha256": sha256(json_line(raw[index]))}


def validate_inventory(inventory, cwd, probe_only):
    if not isinstance(inventory, dict) or any(not isinstance(inventory.get(key), str) for key in ["cwd", "inventory_digest", "configuration_digest"]):
        raise ObservationError("inventory lacks its working directory or digests")
    if inventory["cwd"] != cwd:
        raise ObservationError("inventory working directory differs from the native probe")
    if not probe_only and (inventory.get("static_valid") is not True or inventory.get("source_verification") != "verified"):
        raise ObservationError("acceptance requires a valid static inventory and verified current source")


def run_observation(args):
    cwd = str(Path(args.cwd).resolve(strict=True))
    output = Path(args.output_dir)
    output.mkdir(parents=True, exist_ok=False)
    peer = None
    normalized = []
    catalog = []
    harness_version = ""
    session_id = ""
    model_route = ""
    checks = {name: "unverified" for name in ["fresh_session", "complete_catalog", "explicit_invocation", "companion_access"]}
    problem = None
    inventory = {}
    try:
        if args.inventory:
            loaded = json.loads(Path(args.inventory).read_text())
            inventory = loaded.get("skill_readiness", loaded) if isinstance(loaded, dict) else None
            validate_inventory(inventory, cwd, args.probe_only)
        peer = NativePeer(args.codex, cwd, args.timeout, args.max_bytes, args.codex_home, args.proxy)
        _, initialized = peer.request("initialize", {"clientInfo": {"name": "runedeck-skill-discovery", "version": "0.1.0"}})
        harness_version = initialized.get("userAgent", "")
        if not harness_version:
            raise ObservationError("native initialization omitted the harness version")
        peer.send({"method": "initialized", "params": {}})
        catalog_index, response = peer.request("skills/list", {"cwds": [cwd], "forceReload": True})
        catalog = normalize_catalog(response, cwd)
        checks["complete_catalog"] = "passed"
        if args.catalog and catalog != json.loads(Path(args.catalog).read_text()).get("catalog"):
            raise ObservationError("native catalog changed after static inventory")
        if not args.catalog_only:
            skill_path = str(Path(args.skill_path).resolve(strict=True))
            matching = [entry for entry in catalog if entry["path"] == skill_path]
            if len(matching) != 1 or not matching[0]["enabled"]:
                raise ObservationError("canary lacks one enabled native catalog entry")
            skill = matching[0]
            if sum(entry["name"] == skill["name"] for entry in catalog) != 1:
                raise ObservationError("canary name is ambiguous in native discovery")
            relative = Path(args.companion)
            if relative.is_absolute() or ".." in relative.parts or relative.name == "SKILL.md":
                raise ObservationError("canary companion must be a contained relative path")
            companion = (Path(skill_path).parent / relative).resolve(strict=True)
            if not companion.is_relative_to(Path(skill_path).parent):
                raise ObservationError("canary companion escapes its skill")
            expected = companion.read_bytes()
            if len(expected) > 4096:
                raise ObservationError("canary companion exceeds 4096 bytes")
            expected.decode("utf-8")
            params = {"cwd": cwd, "ephemeral": True, "sandbox": "read-only", "approvalPolicy": "never"}
            if args.model:
                params["model"] = args.model
                params["allowProviderModelFallback"] = False
            _, thread = peer.request("thread/start", params)
            session_id = thread["thread"]["id"]
            model_route = f"{thread['modelProvider']}/{thread['model']}"
            if thread.get("cwd") != cwd or thread.get("sandbox", {}).get("type") != "readOnly":
                raise ObservationError("native session did not retain its read-only working directory contract")
            checks["fresh_session"] = "passed"
            normalized.append(bind_event({"type": "native_catalog", "session_id": session_id, "cwd": cwd, "catalog": catalog}, catalog_index, peer.raw))
            command = shlex.join(["rtk", "proxy", "cat", "--", str(Path(skill_path).parent / relative)])
            prompt = f"Invoke the selected skill only for this read-only discovery canary. Run exactly this shell command: {command}\nDo not run its normal workflow. Do not write files or request permissions. After the command finishes, stop."
            _, started = peer.request("turn/start", {"threadId": session_id, "input": [{"type": "text", "text": prompt}, {"type": "skill", "name": skill["name"], "path": skill_path}]})
            turn_id = started["turn"]["id"]
            while True:
                index, message = peer.read()
                access = normalize_access(message, session_id, turn_id, skill_path, relative.as_posix(), expected)
                if access:
                    normalized.append(bind_event(access, index, peer.raw))
                if message.get("method") == "turn/completed" and message.get("params", {}).get("threadId") == session_id:
                    turn = message["params"].get("turn", {})
                    if turn.get("id") != turn_id or turn.get("status") != "completed":
                        raise ObservationError("native canary turn did not complete successfully")
                    checks["explicit_invocation"] = "passed"
                    break
            if not any(event["type"] == "tool_access" for event in normalized):
                raise ObservationError("native turn supplied no exact successful companion read")
            checks["companion_access"] = "passed"
    except (ObservationError, OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        problem = str(error)
    finally:
        if peer:
            peer.close()
    raw = b"".join(json_line(record) for record in peer.raw) if peer else b""
    transcript = b"".join(json_line(event) for event in normalized)
    (output / "raw.jsonl").write_bytes(raw)
    (output / "normalized.jsonl").write_bytes(transcript)
    catalog_record = {"version": "rune-native-skill-catalog/v1", "cwd": cwd,
                      "harness_version": harness_version, "catalog": catalog,
                      "errors": [] if checks["complete_catalog"] == "passed" else [problem or "catalog unavailable"],
                      "raw_transcript_sha256": sha256(raw)}
    (output / "catalog.json").write_text(json.dumps(catalog_record, indent=2) + "\n")
    observation = {"cwd": cwd, "harness_version": harness_version, "model_route": model_route,
                   "session_id": session_id, "checks": checks, "problem": problem,
                   "process_exit_code": peer.process.returncode if peer else None,
                   "stderr_bytes": peer.stderr_bytes if peer else 0,
                   "raw_transcript_sha256": sha256(raw), "acceptance": "unverified"}
    (output / "catalog-observation.json").write_text(json.dumps(observation, indent=2) + "\n")
    if args.inventory and not args.catalog_only and not args.probe_only and isinstance(inventory, dict) and all(key in inventory for key in ["inventory_digest", "configuration_digest"]):
        evidence = {"version": "rune-native-skill-evidence/v1", "harness_version": harness_version,
                    "model_route": model_route, "cwd": cwd, "inventory_digest": inventory["inventory_digest"],
                    "configuration_digest": inventory["configuration_digest"], "session_id": session_id,
                    "observed_at": datetime.now(timezone.utc).isoformat(), "catalog": catalog,
                    "accesses": [{key: event[key] for key in ["skill_path", "companion_path", "content_sha256"]} | {"tool_event_id": event["event_id"]} for event in normalized if event["type"] == "tool_access"],
                    "checks": checks, "transcript_sha256": sha256(transcript), "raw_transcript_sha256": sha256(raw)}
        (output / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps({"output_dir": str(output), "catalog_entries": len(catalog), "checks": checks, "problem": problem, "acceptance": "unverified"}))
    return 1 if problem else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", default="/Applications/ChatGPT.app/Contents/Resources/codex")
    parser.add_argument("--codex-home", help="Optional isolated native home; no credentials are copied")
    parser.add_argument("--proxy", action="store_true", help="Use the supported IPC proxy to the running local app-server")
    parser.add_argument("--cwd", required=True)
    parser.add_argument("--output-dir", required=True, help="New directory for immutable observation artifacts")
    parser.add_argument("--catalog-only", action="store_true")
    parser.add_argument("--probe-only", action="store_true", help="Observe a canary without claiming static acceptance")
    parser.add_argument("--inventory", help="Frozen doctor JSON or SkillReadiness JSON")
    parser.add_argument("--catalog", help="NativeCatalog record used to prepare the frozen inventory")
    parser.add_argument("--skill-path", help="Harmless canary SKILL.md path")
    parser.add_argument("--companion", help="Harmless UTF-8 companion path relative to the skill")
    parser.add_argument("--model", help="Explicit supported native model; otherwise preserve configured model")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--max-bytes", type=int, default=4 * 1024 * 1024)
    args = parser.parse_args()
    if not 1 <= args.timeout <= 300 or not 1024 <= args.max_bytes <= 8 * 1024 * 1024:
        parser.error("timeout must be 1..300 seconds and max-bytes must be 1024..8388608")
    if args.proxy and args.codex_home:
        parser.error("--proxy uses the configured running server and cannot use --codex-home")
    if not args.catalog_only and (not args.skill_path or not args.companion):
        parser.error("a native turn requires --skill-path and --companion")
    if not args.catalog_only and not args.probe_only and (not args.inventory or not args.catalog):
        parser.error("acceptance evidence requires both --inventory and --catalog")
    try:
        return run_observation(args)
    except (ObservationError, OSError, ValueError) as error:
        print(json.dumps({"acceptance": "unverified", "problem": str(error)}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
