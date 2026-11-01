#!/usr/bin/env python3
"""逐个试模型，看哪个能真的出字。"""
import json, queue, subprocess, threading, time, sys

CANDIDATES = sys.argv[1:] or ["auto-smart[optimize_for=balanced]",
                              "deepseek-v41-flash-1017[reasoning_effort=high]",
                              "composer-2.5[fast=true]"]
host = subprocess.Popen(["target/release/acp-host"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, text=True, bufsize=1)
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in host.stdout], daemon=True).start()
def send(o): host.stdin.write(json.dumps(o)+"\n"); host.stdin.flush()
def wait(cond, timeout=60):
    end = time.time()+timeout
    while time.time() < end:
        try: e = json.loads(q.get(timeout=0.5))
        except queue.Empty: continue
        if cond(e): return e
    return None

send({"id":"1","method":"session.start","params":{"agent":"cursor","cwd":"/Users/wangzihao/code/Sprig"}})
wait(lambda e: e.get("event") == "ready")
print("[ready]")

for model in CANDIDATES:
    send({"id":"m","method":"session.setModel","params":{"session":"s1","model":model}})
    changed = wait(lambda e: e.get("event") in ("modelChanged","error"))
    if changed and changed.get("event") == "error":
        print(model, "→ 切换失败", str(changed)[:100]); continue
    send({"id":"p","method":"session.prompt","params":{"session":"s1","text":"只回复两个字：收到"}})
    text = ""
    end = time.time()+60
    while time.time() < end:
        try: e = json.loads(q.get(timeout=0.5))
        except queue.Empty: continue
        if e.get("event") == "text": text += e.get("text","")
        if e.get("event") == "done": break
        if e.get("event") == "error": print(model, "→ 错误", str(e)[:120]); text=None; break
    if text is not None:
        print(f"{model} → {text.strip()[:80]!r}")
    if text and "Upgrade" not in text and text.strip():
        break

send({"id":"9","method":"shutdown"})
