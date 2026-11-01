# acp-host

把本机的 ACP agent 包成**一个单二进制守护进程**，任何语言接进来只需要：起进程、写一行 JSON、读一行 JSON。

```
你的 app ──(stdin/stdout, 每行一个 JSON)── acp-host ──(ACP/JSON-RPC)── codebuddy / claude / codex
```

存在的理由：ACP 官方已经有 Rust / TS / Python / Kotlin / Java 五套 SDK，但**每接一个语言都要重学一遍协议**。
真正麻烦的不是协议本身，而是这堆琐事：

- 起进程、给 PATH、隔离 HOME、崩溃重连
- `initialize` → `session/new` → `session/prompt` 的时序
- 流式 `session/update` 的多种形态（正文 / 思考 / 工具调用）
- `session/request_permission` 要把决定回传，回错了会话就卡死
- 各家 agent 的脾气（每连接要 authenticate、只认 legacy `set_model`）

这些在这个二进制里做一次，其他语言只做「薄客户端」。

## 编译

```bash
cargo build --release        # → target/release/acp-host，约 450KB
```

## 用

```bash
echo '{"id":"1","method":"agents.list"}' | ./acp-host
```

### 协议

请求（stdin，一行一个）：

| method | params | 说明 |
|---|---|---|
| `agents.list` | — | 列出可用 agent（内置 + `~/.config/acp-host/agents.json`） |
| `session.start` | `agent`, `cwd` | 起一个会话；返回 `session` 句柄 |
| `session.prompt` | `session`, `text` | 发一句话；事件异步回来 |
| `session.cancel` | `session` | 取消当前回合 |
| `permission` | `session`, `request`, `decision` | 回应权限请求（`allow` / `deny`） |
| `shutdown` | — | 收摊 |

响应和事件（stdout）：

```jsonc
{"id":"1","ok":true,"result":{...}}
{"event":"ready","session":"s1","acpSession":"…"}
{"event":"text","session":"s1","text":"片段"}
{"event":"thought","session":"s1","text":"思考片段"}
{"event":"tool","session":"s1","id":"…","title":"…","status":"…"}
{"event":"permission","session":"s1","request":"s1-p4","title":"读取 /tmp/a.txt","options":[…]}
{"event":"done","session":"s1","stopReason":"end_turn"}
{"event":"error","session":"s1","message":{…}}
```

### 加一个 agent

`~/.config/acp-host/agents.json`：

```json
[
  { "id": "fake", "name": "Fake Agent",
    "command": "python3 /path/to/fake_agent.py" }
]
```

内置了 `workbuddy`（自动探测 WorkBuddy.app 里的 CLI）、`codebuddy`、`claude`、`codex`。

## 薄客户端

Python（20 行）：

```python
import json, subprocess
host = subprocess.Popen(["./acp-host"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)

def send(obj):
    host.stdin.write(json.dumps(obj) + "\n"); host.stdin.flush()

send({"id": "1", "method": "session.start", "params": {"agent": "claude", "cwd": "/tmp"}})
send({"id": "2", "method": "session.prompt", "params": {"session": "s1", "text": "你好"}})

for line in host.stdout:
    event = json.loads(line)
    if event.get("event") == "text":
        print(event["text"], end="", flush=True)
    elif event.get("event") == "done":
        break
```

TypeScript / Swift / Go 同理：起进程、按行读写 JSON，没有 SDK 依赖。

## 已知限制（v0.1）

- 只做 `session/prompt` 一轮一问；多轮靠同一个 session 继续发
- 不转发 `fs/*`、`terminal/*` 请求（能力声明为 false）
- 模型切换、登录授权还没做（下一步：`session/set_model`、`auth` 流程）
- 协议是**按行 JSON**，不是标准 ACP；客户端不需要懂 ACP，这正是目的

## 目录

```
src/main.rs         # 全部逻辑：进程管理、协议转译、事件分发
tests/fake_agent.py # 最小 ACP agent，用来验证转译
tests/drive.py      # 驱动脚本：起 host、发命令、自动批准权限
```
