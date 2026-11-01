#!/usr/bin/env python3
"""量首字延迟，并打印权限请求。"""
import json, queue, subprocess, threading, time, sys
agent = "workbuddy"
prompt = sys.argv[1] if len(sys.argv) > 1 else "读一下当前目录 README.md 的前三行"
t0 = time.time()
host = subprocess.Popen(["target/release/acp-host"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in host.stdout], daemon=True).start()
def send(o): host.stdin.write(json.dumps(o)+"\n"); host.stdin.flush()
send({"id":"1","method":"session.start","params":{"agent":agent,"cwd":"/Users/wangzihao/code/Sprig"}})
first = None
deadline = time.time() + 120
while time.time() < deadline:
    try: e = json.loads(q.get(timeout=1))
    except queue.Empty: continue
    kind = e.get("event")
    if kind == "ready":
        print(f"ready  {time.time()-t0:.2f}s"); send({"id":"2","method":"session.prompt","params":{"session":"s1","text":prompt}})
    elif kind == "text":
        if first is None:
            first = time.time()-t0; print(f"首字   {first:.2f}s")
    elif kind == "permission":
        print(f"权限   {time.time()-t0:.2f}s", json.dumps(e.get("title"), ensure_ascii=False))
        send({"id":"3","method":"permission","params":{"session":"s1","request":e["request"],"decision":"allow"}})
    elif kind == "done":
        print(f"完成   {time.time()-t0:.2f}s"); break
send({"id":"9","method":"shutdown"})
