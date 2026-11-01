#!/usr/bin/env python3
"""驱动 acp-host：起进程、发命令、收事件、自动批准权限。"""
import json, os, queue, subprocess, threading, time

host = subprocess.Popen(["target/release/acp-host"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, text=True, bufsize=1)
lines: "queue.Queue[str]" = queue.Queue()

threading.Thread(target=lambda: [lines.put(l) for l in host.stdout], daemon=True).start()

def send(obj):
    host.stdin.write(json.dumps(obj) + "\n"); host.stdin.flush()

send({"id": "1", "method": "agents.list"})
send({"id": "2", "method": "session.start", "params": {"agent": "fake", "cwd": "/tmp"}})
time.sleep(0.6)
send({"id": "3", "method": "session.prompt", "params": {"session": "s1", "text": "说一句话"}})

deadline = time.time() + 6
while time.time() < deadline:
    try:
        line = lines.get(timeout=0.5)
    except queue.Empty:
        continue
    event = json.loads(line)
    print(json.dumps(event, ensure_ascii=False), flush=True)
    if event.get("event") == "permission":
        send({"id": "4", "method": "permission",
              "params": {"session": event["session"], "request": event["request"],
                         "decision": "allow"}})
    if event.get("event") == "done":
        break

send({"id": "9", "method": "shutdown"})
