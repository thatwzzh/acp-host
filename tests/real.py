#!/usr/bin/env python3
"""对着真实 agent 跑一轮：起会话、发一句话、打印所有事件。"""
import json, queue, subprocess, sys, threading, time

agent = sys.argv[1] if len(sys.argv) > 1 else "workbuddy"
prompt = sys.argv[2] if len(sys.argv) > 2 else "用一句话介绍你自己，不要用工具。"

host = subprocess.Popen(["target/release/acp-host"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, text=True, bufsize=1)
lines: "queue.Queue[str]" = queue.Queue()
threading.Thread(target=lambda: [lines.put(l) for l in host.stdout], daemon=True).start()

def send(obj):
    host.stdin.write(json.dumps(obj) + "\n"); host.stdin.flush()

send({"id": "1", "method": "session.start", "params": {"agent": agent, "cwd": "/tmp"}})

deadline = time.time() + 120
ready = False
while time.time() < deadline:
    try:
        line = lines.get(timeout=0.5)
    except queue.Empty:
        continue
    event = json.loads(line)
    text = event.get("text", "")
    printable = {k: (v if not isinstance(v, str) or len(v) < 500 else v[:500] + "…") for k, v in event.items()}
    print(json.dumps(printable, ensure_ascii=False), flush=True)
    if event.get("event") == "ready" and not ready:
        ready = True
        send({"id": "2", "method": "session.prompt", "params": {"session": "s1", "text": prompt}})
    if event.get("event") == "permission":
        send({"id": "3", "method": "permission",
              "params": {"session": "s1", "request": event["request"], "decision": "allow"}})
    if event.get("event") in ("done", "error"):
        break

send({"id": "9", "method": "shutdown"})
