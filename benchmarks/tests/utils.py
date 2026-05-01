import subprocess
import os
import sys
import time
from pathlib import Path
import shutil

benchmarks_dir = Path(__file__).resolve().parent.parent


def _clear_directory(dir: str):
    dir_path = Path(dir)

    for item in dir_path.iterdir():
        if item.is_file() or item.is_symlink():
            item.unlink()
        elif item.is_dir():
            shutil.rmtree(item)


def _parse_bytes(size_str):
    units = {"B": 1, "K": 1024, "M": 1024**2, "G": 1024**3, "T": 1024**4}
    number, unit = size_str[:-1], size_str[-1].upper()
    return int(number) * units.get(unit, 1)


def _create_random_files(count: int, size_bytes: int, dir: str):
    block_size_bytes = _parse_bytes("10M")

    for i in range(count):
        file_name = f'file_{i + 1}'
        file_path = os.path.join(dir, file_name)
        with open(file_path, 'wb') as f:
            written = 0
            while written < size_bytes:
                size = min(block_size_bytes, size_bytes - written)
                content = os.urandom(size)
                f.write(content)
                written += size


def create_download_files(count: int, size: str):
    """Create `count` files with random contents of size `size` for download tests."""
    download_dir = benchmarks_dir / "download-files"
    _clear_directory(download_dir)
    _create_random_files(count, _parse_bytes(size), download_dir)


def create_upload_files(count: int, size: str):
    """Create `count` files with random contents of size `size` for upload tests."""
    upload_dir = benchmarks_dir / "upload-files"
    _clear_directory(upload_dir)
    _create_random_files(count, _parse_bytes(size), upload_dir)


def _tool_run(tool: str, cmd: list[str]) -> str:
    result = subprocess.run([
        "docker",
        "compose",
        "--project-directory",
        str(benchmarks_dir),
        "exec",
        "--interactive=false",
        tool + "-client",
    ] + cmd, capture_output=True, text=True)
    return result.stdout.strip()


def tool_download(tool: str, file: str) -> float:
    """Download `file` using `tool` and return the time in seconds it took."""
    start = time.perf_counter()

    match tool:
        case "ftp":
            _tool_run(tool, ["lftp", "-c", f'"open ftp-server; get files/{file}"'])
        case "http3":
            _tool_run(tool, ["curl", "-kO", "--http3", "https://http3-server/files/" + file])
        case _:
            sys.exit("tool_download: unsupported tool " + tool)

    end = time.perf_counter()
    return end - start


def tool_upload(tool: str, file: str) -> float:
    """Upload `file` using `tool` and return the time in seconds it took."""
    start = time.perf_counter()

    match tool:
        case "ftp":
            _tool_run(tool, ["lftp", "-c", f'"open ftp-server; put /ftp-files/uploads/{file}"'])
        # TODO: add http3 support
        case _:
            sys.exit("tool_upload: unsupported tool " + tool)

    end = time.perf_counter()
    return end - start
