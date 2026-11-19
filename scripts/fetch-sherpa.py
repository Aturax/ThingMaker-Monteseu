"""Fetch the exact static archives selected by sherpa-onnx-sys 2.0.1."""
import argparse
import filecmp
import hashlib
import os
from pathlib import Path
import sys
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request


ARCHIVES = {
    "darwin": {
        "osx-arm64-static-lib": "d7a9b14f337958e7373c8c650f046df33d210bbaff2f33cb8322dda663ee5e1c",
        "osx-x64-static-lib": "6a3cb3c14104d3a02e7b2a6cd5621f590b14892cce7d0bc0935f7655ef518938",
    },
    "win32": {
        "win-x64-static-MT-Release-lib": "23e7b986b197eda0bbc47ff3d7d5c87afd6ef2b9077214ba26b0f1ec52ef0e66",
    },
}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verified_archive(destination, target, offline=False):
    expected = ARCHIVES[sys.platform][target]
    filename = f"sherpa-onnx-v2.0.1-{target}.tar.bz2"
    archive = destination / filename
    if archive.is_file() and sha256(archive) == expected:
        return archive
    if offline:
        raise ValueError(f"Missing or invalid verified archive: {archive}")
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination) as stage:
        download = Path(stage) / filename
        url = f"https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/{filename}"
        with urllib.request.urlopen(url, timeout=90) as response, download.open("wb") as output:
            shutil.copyfileobj(response, output)
        if sha256(download) != expected:
            raise ValueError(f"Unexpected checksum for {filename}")
        os.replace(download, archive)
    return archive


def local_libs(cache):
    if sys.platform == "win32":
        target = "win-x64-static-MT-Release-lib"
    else:
        host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if line.startswith("host: "))
        target = {"aarch64-apple-darwin": "osx-arm64-static-lib", "x86_64-apple-darwin": "osx-x64-static-lib"}[host]
    supplied = os.environ.get("SHERPA_ONNX_ARCHIVE_DIR")
    archive = verified_archive(Path(supplied) if supplied else cache, target, offline=bool(supplied))
    # The upstream build script prefers previously extracted libraries over ARCHIVE_DIR.
    # Pass our verified lib directory directly, preserving timestamps for incremental builds.
    libs = cache / ARCHIVES[sys.platform][target] / "lib"
    libs.mkdir(parents=True, exist_ok=True)
    prefix = archive.name.removesuffix(".tar.bz2") + "/lib/"
    verified_names = set()
    with tempfile.TemporaryDirectory(dir=libs.parent) as stage, tarfile.open(archive, "r|bz2") as tar:
        for member in tar:
            basename = member.name.removeprefix(prefix)
            if not member.isfile() or basename == member.name or "/" in basename or not basename.endswith((".a", ".lib")):
                continue
            verified_names.add(basename)
            extracted = Path(stage) / basename
            with extracted.open("wb") as output:
                shutil.copyfileobj(tar.extractfile(member), output)
            installed = libs / basename
            if not installed.is_file() or not filecmp.cmp(extracted, installed, shallow=False):
                os.replace(extracted, installed)
    if not verified_names or {p.name for p in libs.iterdir()} != verified_names:
        raise ValueError(f"Unexpected files in verified library directory: {libs}")
    return libs.resolve()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path)
    parser.add_argument("--local-libs", action="store_true")
    args = parser.parse_args()
    if args.local_libs:
        print(local_libs(args.destination))
    else:
        for target in ARCHIVES[sys.platform]:
            verified_archive(args.destination, target)
