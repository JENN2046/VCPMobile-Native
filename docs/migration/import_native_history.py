#!/usr/bin/env python3
"""One-time scoped import. Executes no source code and publishes only a NEW
migration/native-history branch in JENN2046/VCPMobile-Native. Never writes source,
main, existing destination branches, settings, credentials, or releases.
Adapted from the privately delivered prepare_history.py; private archives are not
needed on the runner and are never uploaded. Native workflow is installed later
through the connector, avoiding workflow-write permission in this job.
"""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

SOURCE_URL = "https://github.com/JENN2046/VCPMobile.git"
SOURCE_BRANCH = "codex/native-rust-m0-m1"
SOURCE_BASE = "88ee3aca0e6a57a9ab830ac4bf380770138f1ed0"
SOURCE_HEAD = "a615bff38e497419c62f1bef1601add6ee19bcca"
SOURCE_TREE = "d5b8cc9317d983a11010b922b4aa14d7a7f43c7a"
DESTINATION = "JENN2046/VCPMobile-Native"
STAGING_REF = "refs/heads/migration/native-history"
EXPECTED_MANIFEST = "754a10b0dbd349f27222b3f5d3e113d51700b02e2418a310522b1796902e1fe8"
WORKFLOW_PATH = ".github/workflows/native-core.yml"


def run(*args: str, cwd: Path | None = None, data: bytes | None = None) -> bytes:
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"
    proc = subprocess.run(args, cwd=cwd, env=env, input=data,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=300, check=False)
    if proc.returncode:
        raise RuntimeError(f"{args[0]} failed ({proc.returncode}): " +
                           proc.stderr.decode("utf-8", "replace")[-2500:])
    return proc.stdout


def git(root: Path, *args: str, **kw) -> bytes:
    return run("git", "-C", str(root), *args, **kw)


def entries(root: Path, revision: str, scope: tuple[str, ...] = ()) -> dict[str, tuple[str, str]]:
    result = {}
    for entry in git(root, "ls-tree", "-rz", revision, "--", *scope).split(b"\0"):
        if not entry:
            continue
        meta, name = entry.split(b"\t", 1)
        mode, kind, oid = meta.decode().split()
        path = name.decode("utf-8")
        if kind != "blob" or mode not in ("100644", "100755"):
            raise RuntimeError("Unsupported source mode: " + path)
        result[path] = (mode, oid)
    return result


def provenance(root: Path, revision: str) -> tuple[list[bytes], bytes]:
    header, message = git(root, "cat-file", "commit", revision).split(b"\n\n", 1)
    identities = [line for line in header.splitlines()
                  if line.startswith((b"author ", b"committer ", b"encoding "))]
    return identities, message


def import_scope(src: Path, dst: Path, base: str, head: str,
                 expected: dict[str, tuple[str, str]], count: int) -> list[dict]:
    """Pure local Git operation; used by the synthetic regression as well."""
    git(src, "merge-base", "--is-ancestor", base, head)
    original_ids = git(src, "rev-list", "--reverse", f"{base}..{head}").decode().splitlines()
    if len(original_ids) != count or git(src, "rev-list", "--min-parents=2", f"{base}..{head}").strip():
        raise RuntimeError("Unexpected source development history")
    git(src, "branch", "migration-source", head)
    old_marks, new_marks = dst.parent/"old.marks", dst.parent/"new.marks"
    stream = git(src, "fast-export", "--full-tree", "--reencode=no",
                 f"--export-marks={old_marks}",
                 "--refspec=refs/heads/migration-source:refs/heads/imported",
                 "migration-source", f"^{base}", "--", "native/", "LICENSE")
    dst.mkdir()
    git(dst, "init", "-b", "imported")
    git(dst, "fast-import", "--quiet", f"--export-marks={new_marks}", data=stream)
    old = dict(line.split() for line in old_marks.read_text().splitlines())
    new = dict(line.split() for line in new_marks.read_text().splitlines())
    mapped = {oid: new[mark] for mark, oid in old.items()}
    if set(mapped) != set(original_ids):
        raise RuntimeError("Development commits missing from filtered history")
    mapping = []
    for source_oid in original_ids:
        filtered_oid = mapped[source_oid]
        if provenance(src, source_oid) != provenance(dst, filtered_oid):
            raise RuntimeError("Author, committer, time or message changed")
        tree = entries(dst, filtered_oid)
        if any(not (p.startswith("native/") or p == "LICENSE") for p in tree):
            raise RuntimeError("Unrelated source paths entered history")
        item = {"source": source_oid, "filtered": filtered_oid}
        workflow = git(src, "ls-tree", source_oid, "--", WORKFLOW_PATH).decode().split()
        if workflow:
            item["source_workflow_blob"] = workflow[2]
        mapping.append(item)
    if entries(dst, "imported") != expected:
        raise RuntimeError("Final filtered paths/modes/blob IDs differ")
    if int(git(dst, "rev-list", "--count", "imported")) != count:
        raise RuntimeError("Filtered history count differs")
    git(dst, "reset", "--hard", "imported")  # New isolated repository only.
    git(dst, "fsck", "--strict")
    return mapping


def main() -> None:
    if os.environ.get("GITHUB_REPOSITORY") != DESTINATION:
        raise RuntimeError("Wrong destination repository")
    if os.environ.get("GITHUB_REF") != "refs/heads/main":
        raise RuntimeError("Bootstrap must run from main")
    workspace = Path(os.environ["GITHUB_WORKSPACE"])
    expected_seed = os.environ["GITHUB_SHA"]
    destination_url = f"https://github.com/{DESTINATION}.git"
    origin = git(workspace, "remote", "get-url", "origin").decode().strip()
    if origin not in (destination_url, destination_url[:-4]):
        raise RuntimeError("Unexpected publication remote")
    if git(workspace, "rev-parse", "HEAD").decode().strip() != expected_seed:
        raise RuntimeError("Checkout differs from triggering commit")
    remote_source = run("git", "-c", "credential.helper=", "ls-remote",
                        SOURCE_URL, f"refs/heads/{SOURCE_BRANCH}").decode().split()
    if not remote_source or remote_source[0] != SOURCE_HEAD:
        raise RuntimeError("Source moved; stop and reconcile")
    before = git(workspace, "ls-remote", "origin", "refs/heads/*").decode().splitlines()
    if before != [f"{expected_seed}\trefs/heads/main"]:
        raise RuntimeError("Destination is no longer the expected bootstrap-only repo")
    evidence = workspace/"migration-evidence"
    evidence.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="vcp-import-") as temp:
        work = Path(temp)
        src, dst = work/"source", work/"filtered"
        run("git", "-c", "credential.helper=", "clone", "--depth=17",
            "--filter=blob:none", "--no-checkout", "--single-branch", "--branch",
            SOURCE_BRANCH, SOURCE_URL, str(src))
        if git(src, "rev-parse", "HEAD").decode().strip() != SOURCE_HEAD:
            raise RuntimeError("Source moved during fetch")
        if git(src, "rev-parse", "HEAD^{tree}").decode().strip() != SOURCE_TREE:
            raise RuntimeError("Source tree identity mismatch")
        source_tree = entries(src, SOURCE_HEAD, ("native/", "LICENSE", WORKFLOW_PATH))
        scope_tree = {p: v for p, v in source_tree.items()
                      if p.startswith("native/") or p in ("LICENSE", WORKFLOW_PATH)}
        manifest = {p: hashlib.sha256(git(src, "show", f"{SOURCE_HEAD}:{p}")).hexdigest()
                    for p in scope_tree}
        canonical = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
        if len(manifest) != 59 or hashlib.sha256(canonical).hexdigest() != EXPECTED_MANIFEST:
            raise RuntimeError("Source differs from the reviewed 59-file package")
        expected = {p: v for p, v in scope_tree.items() if p != WORKFLOW_PATH}
        mapping = import_scope(src, dst, SOURCE_BASE, SOURCE_HEAD, expected, 16)
        filtered_head = git(dst, "rev-parse", "HEAD").decode().strip()
        docs = dst/"docs/migration"
        docs.mkdir(parents=True)
        record = {"source_repository": "JENN2046/VCPMobile", "source_base": SOURCE_BASE,
                  "source_head": SOURCE_HEAD, "source_tree": SOURCE_TREE,
                  "filtered_head": filtered_head, "development_commits": len(mapping),
                  "source_file_count": len(manifest), "source_manifest_sha256": EXPECTED_MANIFEST,
                  "workflow_blob": scope_tree[WORKFLOW_PATH][1],
                  "workflow_history_note": "Native and LICENSE history filtered; current workflow is imported byte-for-byte separately. Historical workflow blobs remain referenced in COMMIT_MAP and source repository.",
                  "bootstrap_head": expected_seed, "import_run": os.environ.get("GITHUB_RUN_ID"),
                  "status": "SCOPED_HISTORY_STAGED_PENDING_CUTOVER_AND_CI"}
        (docs/"COMMIT_MAP.json").write_text(json.dumps(mapping, indent=2)+"\n")
        (docs/"SOURCE_MANIFEST_A615BFF.json").write_text(json.dumps(
            {"commit": SOURCE_HEAD, "tree": SOURCE_TREE, "source_files_sha256": manifest}, indent=2)+"\n")
        (docs/"MIGRATION_REPORT.json").write_text(json.dumps(record, indent=2)+"\n")
        (docs/"MIGRATION_STATUS.md").write_text(
            "# Repository migration\n\nNative and LICENSE history is staged with 16 author/time/message-preserved commits. "
            "All 59 scoped source files match the reviewed package; the workflow is installed separately. "
            "The original source branch is unchanged. Main/development cutover and new-repository CI remain pending.\n\n"
            "Old run 40282422568: Android succeeded; iOS had three UI passes and six Keychain failures "
            "(unavailable / -34018). No failure is erased by migration. M2 through M5 are incomplete; "
            "real service, physical devices and signing remain separate acceptance requirements.\n")
        git(dst, "add", "docs/migration")
        git(dst, "-c", "user.name=github-actions[bot]",
            "-c", "user.email=41898282+github-actions[bot]@users.noreply.github.com",
            "commit", "-m", "docs: record scoped native migration and source identities")
        staged_head = git(dst, "rev-parse", "HEAD").decode().strip()
        git(dst, "fsck", "--strict")
        # Recheck both references immediately before publishing one NEW staging ref.
        current_source = run("git", "-c", "credential.helper=", "ls-remote", SOURCE_URL,
                             f"refs/heads/{SOURCE_BRANCH}").decode().split()
        if not current_source or current_source[0] != SOURCE_HEAD:
            raise RuntimeError("Source changed before publication")
        if git(workspace, "ls-remote", "origin", "refs/heads/*").decode().splitlines() != before:
            raise RuntimeError("Destination changed before publication")
        git(workspace, "fetch", str(dst), f"refs/heads/imported:{STAGING_REF}")
        git(workspace, "push", "--porcelain", "origin", f"{STAGING_REF}:{STAGING_REF}")
        remote_after = git(workspace, "ls-remote", "origin", "refs/heads/main", STAGING_REF).decode().splitlines()
        expected_after = sorted([f"{expected_seed}\trefs/heads/main", f"{staged_head}\t{STAGING_REF}"])
        if sorted(remote_after) != expected_after:
            raise RuntimeError("Published refs did not match; inspect without retrying")
        result = {**record, "staged_head": staged_head, "main_unchanged": True,
                  "source_unchanged": True, "status": "SCOPED_HISTORY_STAGED_VERIFIED"}
        for path in docs.iterdir():
            (evidence/path.name).write_bytes(path.read_bytes())
        (evidence/"RESULT.json").write_text(json.dumps(result, indent=2)+"\n")
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
