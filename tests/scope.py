import json, os, os, queue, subprocess, sys, threading, time
env = dict(os.environ)
if "--skip" in sys.argv: env["ACP_HOST_SKIP_PERMISSIONS"] = "1"
prompt = [a for a in sys.argv[1:] if not a.startswith("--")][0]
p = subprocess.Popen(["target/release/acp-host"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1, env=env)
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
def send(o): p.stdin.write(json.dumps(o)+"\n"); p.stdin.flush()
send({"id":"1","method":"session.start","params":{"agent":"workbuddy","cwd":os.environ.get("ACP_HOST_CWD", os.getcwd())}})
text=""; asked=False
end=time.time()+90
while time.time()<end:
    try: e=json.loads(q.get(timeout=1))
    except queue.Empty: continue
    k=e.get("event")
    if k=="ready": send({"id":"2","method":"session.prompt","params":{"session":"s1","text":prompt}})
    elif k=="permission":
        asked=True; print("[被要求授权]", e.get("title"))
        send({"id":"3","method":"permission","params":{"session":"s1","request":e["request"],"option":"allow"}})
    elif k=="text": text+=e.get("text","")
    elif k in ("done","error"): break
print("[是否弹权限]", asked)
print("[回答]", text.strip()[:200])
