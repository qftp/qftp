import subprocess
import os
import sys
import time
from pathlib import Path
import shutil

benchmarks_dir = Path(__file__).resolve().parent.parent
tools = ["ftp", "http3"]


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
    block_size_bytes = _parse_bytes("1G")

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


def _tool_run(tool: str, cmd: list[str], server=False) -> str:
    result = subprocess.run([
        "docker",
        "compose",
        "--project-directory",
        str(benchmarks_dir),
        "exec",
        "--interactive=false",
        tool + ("-server" if server else "-client"),
    ] + cmd, capture_output=True, text=True)
    return result.stderr.strip()


def tool_download(tool: str, files: list[str]) -> [float, str]:
    """Download `files` using `tool` and return the time in seconds it took."""
    start = time.perf_counter()
    output = ""

    match tool:
        case "ftp":
            output = _tool_run(tool, ["lftp", "-c", f'set xfer:clobber on; open ftp-server; mget {" ".join(map(lambda f: "files/" + f, files))}'])
        case "http3":
            output = _tool_run(tool, ["curl", "-kZ", "--http3", "--remote-name-all"] + list(map(lambda f: "https://http3-server/files/" + f, files)))
        case _:
            sys.exit("tool_download: unsupported tool " + tool)

    end = time.perf_counter()
    return end - start, output


def tool_upload(tool: str, file: str) -> [float, str]:
    """Upload `file` using `tool` and return the time in seconds it took."""
    start = time.perf_counter()
    output = ""

    match tool:
        case "ftp":
            output = _tool_run(tool, ["lftp", "-c", f'open ftp-server; put /ftp-files/uploads/{file}'])
        # TODO: add http3 support
        case _:
            sys.exit("tool_upload: unsupported tool " + tool)

    end = time.perf_counter()
    return end - start, output


def tc_add_download(tool: str, rule: str):
    """Add tc rule that affects downloads with the given tool."""
    # tc applies to egress (outgoing traffic)
    _tool_run(tool, [
        "tc",
        "qdisc",
        "add",
        "dev",
        "eth0",
    ] + rule.split(" "), server=True)


def tc_add_download_all(rule: str):
    """Add tc rule that affects downloads with all tools."""
    for tool in tools:
        tc_add_download(tool, rule)


def set_packet_loss_download(loss_percent: int):
    for tool in tools:
        _tool_run(tool, "nft add table inet net_sim".split(" "), server=True)
        _tool_run(tool, "nft add chain inet net_sim postrouting { type filter hook postrouting priority 0 \\; }".split(" "), server=True)
        _tool_run(tool, f"nft add rule inet net_sim postrouting chaos probability {loss_percent} drop".split(" "), server=True)


def tc_add_upload(tool: str, rule: str):
    """Add tc rule that affects uploads with the given tool."""
    _tool_run(tool, [
        "tc",
        "qdisc",
        "add",
        "dev",
        "eth0",
    ] + rule.split(" "))


def tc_add_upload_all(rule: str):
    """Add tc rule that affects uploads with all tools."""
    for tool in tools:
        tc_add_upload(tool, rule)


def net_cleanup():
    """Cleanup applied tc rules for all tools."""
    for tool in tools:
        _tool_run(tool, "nft delete table inet net_sim".split(" "), server=True)

        _tool_run(tool, [
            "tc",
            "qdisc",
            "del",
            "dev",
            "eth0",
            "root"
        ])

        _tool_run(tool, [
            "tc",
            "qdisc",
            "del",
            "dev",
            "eth0",
            "root"
        ], server=True)
