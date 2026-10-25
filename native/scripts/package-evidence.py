#!/usr/bin/env python3
"""Package tracked source only. Do not archive the working directory or secrets."""
from __future__ import annotations
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import zipfile


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(root), *args])


def package(root: Path, output: Path) -> Path:
    commit = git(root, "rev-parse", "HEAD").decode().strip()
    tree = git(root, "rev-parse", "HEAD^{tree}").decode().strip()
    source = git(root, "archive", "--format=zip", "HEAD", "native", ".github/workflows/native-core.yml", "LICENSE")
    output.mkdir(parents=True, exist_ok=True)
    archive = output / "native-source.zip"
    with zipfile.ZipFile(io.BytesIO(source)) as original:
        tracked = {entry.filename: original.read(entry) for entry in original.infolist() if not entry.is_dir()}
    files = {name: hashlib.sha256(data).hexdigest() for name, data in sorted(tracked.items())}
    manifest = {
        "commit": commit, "tree": tree,
        "scope": ["native/", ".github/workflows/native-core.yml", "LICENSE"],
        "workflow_run_id": os.getenv("GITHUB_RUN_ID"),
        "source_files_sha256": files,
        "validation": "Packaging is not build, service, or device acceptance.",
    }
    lock = root / "native/Cargo.lock"
    lock_bytes = lock.read_bytes() if lock.is_file() else None
    if lock_bytes is not None:
        recorded = tracked.get("native/Cargo.lock")
        if recorded is not None and recorded != lock_bytes:
            raise RuntimeError("Cargo.lock differs from tracked revision")
        manifest["cargo_lock"] = {"sha256": hashlib.sha256(lock_bytes).hexdigest(), "tracked": recorded is not None}
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as target:
        for name, data in sorted(tracked.items()):
            target.writestr(name, data)
        if lock_bytes is not None and "native/Cargo.lock" not in tracked:
            target.writestr("resolved-dependencies/Cargo.lock", lock_bytes)
        target.writestr("SOURCE_MANIFEST.json", json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (output / "native-source.sha256").write_text(f"{digest}  native-source.zip\n", encoding="utf-8")
    (output / "source-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Source {commit}: {len(files)} tracked files; SHA256 {digest}")
    return archive


if __name__ == "__main__":
    repository = Path(__file__).resolve().parents[2]
    package(repository, repository / "native/target/evidence")
