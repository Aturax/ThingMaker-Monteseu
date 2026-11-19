"""Offline regression checks for local native dependency preparation."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("fetch_sherpa", Path(__file__).with_name("fetch-sherpa.py"))
sherpa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sherpa)


class VerifiedLibraries(unittest.TestCase):
    def test_only_verified_libraries_reach_the_linker(self):
        for system, host, target, lib in [
            ("darwin", "aarch64-apple-darwin", "osx-arm64-static-lib", "libonnxruntime.a"),
            ("darwin", "x86_64-apple-darwin", "osx-x64-static-lib", "libonnxruntime.a"),
            ("win32", "x86_64-pc-windows-msvc", "win-x64-static-MT-Release-lib", "onnxruntime.lib"),
        ]:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                name = f"sherpa-onnx-v2.0.1-{target}"
                archive = root / f"{name}.tar.bz2"
                content = b"verified native code"
                with tarfile.open(archive, "w:bz2") as tar:
                    member = tarfile.TarInfo(f"{name}/lib/{lib}")
                    member.size = len(content)
                    tar.addfile(member, io.BytesIO(content))
                digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                with patch.object(sherpa.sys, "platform", system), patch.dict(sherpa.ARCHIVES[system], {target: digest}), \
                        patch.object(sherpa.subprocess, "check_output", return_value=f"host: {host}"), \
                        patch.dict(os.environ, {"SHERPA_ONNX_ARCHIVE_DIR": tmp}):
                    cache = root / "cache"
                    library = sherpa.local_libs(cache) / lib
                    self.assertEqual(library.read_bytes(), content)
                    library.write_bytes(b"stale library")
                    self.assertEqual((sherpa.local_libs(cache) / lib).read_bytes(), content)
                    (library.parent / "unverified.lib").write_bytes(b"extra")
                    with self.assertRaisesRegex(ValueError, "Unexpected files"):
                        sherpa.local_libs(cache)
                    archive.write_bytes(b"corrupt archive")
                    with self.assertRaisesRegex(ValueError, "Missing or invalid"):
                        sherpa.local_libs(cache)


if __name__ == "__main__":
    unittest.main()
