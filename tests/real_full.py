#!/usr/bin/env python3
"""真实 agent 全流程：登录 → 提问 → 换模型 → 再问。"""
import json, os, queue, subprocess, sys, threading, time

agent = sys.argv[1] if len(sys.argv) > 1 else "cursor"
host = subprocess.Popen(["target/release/acp-host"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, text=True, bufsize=1)
lines = queue.Queue()
threading.Thread(target=lambda: [lines.put(l) for l in host.stdout], daemon=True).start()

def send(obj):
    host.stdin.write(json.dumps(obj) + "\n"); host.stdin.flush()

send({"id": "1", "method": "session.start", "params": {"agent": agent, "cwd": "/tmp"}})

state = {"step": "start", "models": [], "text": ""}
deadline = time.time() + 180
while time.time() < deadline:
    try:
        line = lines.get(timeout=0.5)
    except queue.Empty:
        continue
    e = json.loads(line)
    kind = e.get("event")

    if kind == "ready":
        models = (e.get("models") or {}).get("availableModels") or []
        state["models"] = [m.get("modelId") for m in models]
        print(f"[ready] models={len(state['models'])} current={(e.get('models') or {}).get('currentModelId')}")
        print(f"        authMethods={e.get('authMethods')}")
        if e.get("authMethods") and not state.get("authed"):
            state["step"] = "auth"
            send({"id": "2", "method": "auth", "params": {"session": "s1"}})
        elif state.get("authed"):
            state["step"] = "prompt1"
            send({"id": "3", "method": "session.prompt",
                  "params": {"session": "s1", "text": "用一句话介绍你自己，不要用工具。"}})

    elif kind == "authenticated":
        state["authed"] = True
        print("[auth] OK")

    elif kind == "text":
        state["text"] += e.get("text", "")

    elif kind == "done":
        print(f"[done] stop={e.get('stopReason')} text={state['text'][:120]!r}")
        state["text"] = ""
        if state["step"] == "prompt1" and len(state["models"]) > 1:
            state["step"] = "model"
            target = state["models"][1]
            print(f"[model] switch -> {target}")
            send({"id": "4", "method": "session.setModel",
                  "params": {"session": "s1", "model": target}})
        elif state["step"] == "prompt2":
            break

    elif kind == "modelChanged":
        print(f"[model] changed -> {e.get('model')}")
        state["step"] = "prompt2"
        send({"id": "5", "method": "session.prompt",
              "params": {"session": "s1", "text": "现在换模型了，再回一句。"}})

    elif kind in ("error", "authFailed"):
        print(f"[{kind}] {json.dumps(e, ensure_ascii=False)[:300]}")
        break
    elif kind == "permission":
        send({"id": "9", "method": "permission",
              "params": {"session": "s1", "request": e["request"], "decision": "allow"}})

send({"id": "99", "method": "shutdown"})
