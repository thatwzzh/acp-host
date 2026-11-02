"""acp-host 薄客户端：起进程、按行 JSON 收事件。无第三方依赖。"""
from __future__ import annotations

import json
import os
import queue
import subprocess
import threading
from typing import Callable, Iterator


class AcpHost:
    def __init__(self, binary: str | None = None, cwd: str | None = None):
        from .binary import resolve
        self.binary = binary or resolve()
        self.cwd = cwd or os.getcwd()
        self._proc: subprocess.Popen | None = None
        self._next = 0
        self._routes: dict[str, queue.Queue] = {}
        self._lock = threading.Lock()

    def start(self) -> "AcpHost":
        if self._proc:
            return self
        self._proc = subprocess.Popen(
            [self.binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, bufsize=1)
        threading.Thread(target=self._read, daemon=True).start()
        return self

    def _read(self) -> None:
        assert self._proc and self._proc.stdout
        for line in self._proc.stdout:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            route = self._routes.get(event.get("session", ""))
            if route:
                route.put(event)

    def _send(self, payload: dict) -> None:
        assert self._proc and self._proc.stdin
        self._proc.stdin.write(json.dumps(payload) + "\n")
        self._proc.stdin.flush()

    def ask(self, text: str, agent: str = "workbuddy", cwd: str | None = None,
            on_permission: Callable[[dict], str] | None = None) -> Iterator[str]:
        """问一句，逐块产出文本；权限请求交给 on_permission 返回 optionId。"""
        self.start()
        with self._lock:
            self._next += 1
            session = f"s{self._next}"
        route: queue.Queue = queue.Queue()
        self._routes[session] = route
        self._send({"id": "1", "method": "session.start",
                    "params": {"agent": agent, "cwd": cwd or self.cwd}})
        try:
            while True:
                event = route.get()
                kind = event.get("event")
                if kind == "ready":
                    self._send({"id": "2", "method": "session.prompt",
                                "params": {"session": session, "text": text}})
                elif kind == "text":
                    yield event.get("text", "")
                elif kind == "permission":
                    decision = on_permission(event) if on_permission else "reject"
                    self._send({"id": "3", "method": "permission",
                                "params": {"session": session,
                                           "request": event.get("request"),
                                           "option": decision}})
                elif kind == "done":
                    return
                elif kind == "error":
                    raise RuntimeError(str(event.get("message")))
        finally:
            self._routes.pop(session, None)

    def close(self) -> None:
        if self._proc:
            self._proc.kill()
            self._proc = None
