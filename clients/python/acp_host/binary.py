"""找到可用的 acp-host 二进制：环境变量 → PATH → 缓存 → 现下。"""
from __future__ import annotations

import hashlib
import os
import platform
import shutil
import sys
import tarfile
import tempfile
import urllib.request

VERSION = os.environ.get("ACP_HOST_VERSION", "0.1.5")
REPO = "thatwzzh/acp-host"


def _target() -> str | None:
    system = {"darwin": "apple-darwin", "linux": "unknown-linux-gnu"}.get(sys.platform)
    machine = platform.machine().lower()
    arch = "aarch64" if machine in ("arm64", "aarch64") else "x86_64" if machine in ("x86_64", "amd64") else None
    if not system or not arch:
        return None
    return f"{arch}-{system}"


def _cache_dir() -> str:
    root = os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache")
    return os.path.join(root, "acp-host", VERSION)


def _download(target: str, destination: str) -> bool:
    base = f"https://github.com/{REPO}/releases/download/v{VERSION}"
    archive_url = f"{base}/acp-host-{target}.tar.gz"
    checksum_url = archive_url + ".sha256"
    try:
        with urllib.request.urlopen(archive_url, timeout=60) as response:
            payload = response.read()
        with urllib.request.urlopen(checksum_url, timeout=30) as response:
            expected = response.read().decode().split()[0]
        actual = hashlib.sha256(payload).hexdigest()
        if expected and actual != expected:
            return False
        os.makedirs(os.path.dirname(destination), exist_ok=True)
        with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as handle:
            handle.write(payload)
            temp = handle.name
        with tarfile.open(temp) as archive:
            archive.extract("acp-host", path=os.path.dirname(destination))
        os.unlink(temp)
        os.chmod(destination, 0o755)
        return True
    except Exception:
        return False


def resolve() -> str:
    """返回二进制路径；拿不到就抛错，提示怎么手动指定。"""
    configured = os.environ.get("ACP_HOST_BIN")
    if configured:
        return configured

    # 先看自己下过的那份：它的版本和包是对齐的。
    # 反过来先信 PATH，用户机器上躺着一个旧版就会静默用到旧版（曾经踩过：
    # brew 里的 0.1.0 把 0.1.2 的包顶掉，连 version 命令都没有）。
    target = _target()
    if target:
        cached = os.path.join(_cache_dir(), "acp-host")
        if os.path.exists(cached):
            return cached
        if _download(target, cached):
            return cached

    on_path = shutil.which("acp-host")
    if on_path:
        return on_path

    raise FileNotFoundError(
        "找不到 acp-host。三种办法：装 Homebrew 包 `brew install thatwzzh/acp-host/acp-host`；"
        "从 https://github.com/thatwzzh/acp-host/releases 下对应平台的包放进 PATH；"
        "或设置环境变量 ACP_HOST_BIN 指向它的路径。")
