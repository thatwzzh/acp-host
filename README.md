# acp-host

把本机的 ACP agent 包成**一个单二进制守护进程**，任何语言接进来只需要：起进程、写一行 JSON、读一行 JSON。

> **接入实例**：[Sprig（知枝）](https://github.com/thatwzzh/sprig) —— macOS 的划词问答面板。
> 它把 acp-host 打进 `Sprig.app/Contents/Resources/`，面板只管渲染；登录、换模型、权限往返、
> 各家 agent 的时序都在这个二进制里。想知道「真接进去长什么样」，看它就行。


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
| `session.setModel` | `session`, `model` | 换模型（先试 `set_config_option`，失败退回 legacy `set_model`） |
| `session.setMode` | `session`, `mode` | 换模式（Agent / Plan / Ask 这类） |
| `session.setOption` | `session`, `configId`, `value` | 改任意选项（思考强度、权限模式…） |
| `auth` | `session`, `method`（可省） | 走 agent 的登录流程 |
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
{"event":"authenticated","session":"s1"}
{"event":"modelChanged","session":"s1","model":"grok-4.7[…]"}
{"event":"done","session":"s1","stopReason":"end_turn"}
{"event":"error","session":"s1","code":-32000,"message":{…},"authRequired":true}
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

## 安装

三种方式，装完都能直接用（**不需要 Rust**）：

```bash
# Homebrew（macOS / Linux）
brew tap thatwzzh/acp-host && brew install acp-host

# npm：自带客户端，安装时自动拉对应平台的二进制
npm i -g acp-host-client

# PyPI：同上，首次调用时下载到 ~/.cache/acp-host/
pip install acp-host-client
```

不想装包管理器就直接下二进制：

```bash
curl -L https://github.com/thatwzzh/acp-host/releases/latest/download/acp-host-aarch64-apple-darwin.tar.gz \
  | tar xz -C /usr/local/bin
```

## 在你项目里用

```python
from acp_host import AcpHost

host = AcpHost()
for piece in host.ask("你好", agent="workbuddy"):
    print(piece, end="")
```

```js
import { AcpHost } from "acp-host-client";

const host = new AcpHost();
for await (const piece of host.ask("你好", { agent: "workbuddy" })) console.log(piece);
```

客户端只做「起进程 + 按行收发 JSON」，查找二进制的顺序：
`ACP_HOST_BIN` → PATH → npm 包内的 `vendor/` → `~/.cache/acp-host/`。

**自己包一层也很简单**：任何语言都是「起进程 → 写一行 JSON → 读一行 JSON」，
参考 `clients/` 里两个实现，各 80 行左右。

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

## 真实 agent 验证

**WorkBuddy 自带 CLI：全链路跑通**（真实生成中文回答）：

```
[ready] models=22 current=fast-model
[done] text='我是一个能帮你完成软件工程任务的人工智能编程助手，可以读写代码、运行命令和排查问题。'
[model] changed -> balanced-model
[done] text='我还是那个能帮你写代码、查 Bug、跑命令的 AI 编程助手，只是换了模型来回答。'
```

它的启动命令比较特别（lite 变体，否则会撞端口 / 卡在配置拉取）：

```
env -u HTTP_PROXY -u HTTPS_PROXY HOME=<独立目录> CODEBUDDY_FORCE_LITE_WB_BUNDLE=1 \
    '/Applications/WorkBuddy.app/…/cli/bin/codebuddy' --serve --port 0 --acp
```

`agents.list` 里已经内置这一条（自动探测安装路径）。

Cursor CLI 也验过握手、模型清单、登录、换模型，但它的额度/地区限制会回
`Upgrade your plan to continue` —— 那是账号侧的事，协议层没问题：

| 能力 | 结果 |
|---|---|
| `initialize` + 版本校验 | ✅ 两个都通 |
| `session/new` | ✅ 拿到 sessionId |
| 模型清单 | ✅ Cursor 返回 **32 个**模型（`availableModels`） |
| `auth` | ✅ Cursor 返回 `cursor_login`，调用成功 |
| `session.setModel` | ✅ 触发 `modelChanged`（`set_config_option` 可用） |
| 真正生成文本 | ✅ WorkBuddy 通；Cursor 受额度限制；Claude 需登录 |

> 2026.05 的 Cursor CLI 在 `session/prompt` 后会回 `WritableIterable is closed`，升级到
> 2026.10 后消失，变成明确的 `Upgrade your plan to continue`。

## 环境变量

| 变量 | 作用 |
|---|---|
| `ACP_HOST_TRACE=1` | 把收发原文打到 stderr |
| `ACP_HOST_TRACE_LIMIT=4000` | 单行截断长度（默认 1200） |
| `ACP_HOST_FS=1` | 声明支持文件/终端能力（有些 agent 不放开不肯流式） |
| `ACP_HOST_AUTO_AUTH=1` | 开会话前先主动 authenticate（Cursor 这类需要；**默认关**，否则 WorkBuddy 每次提问都会弹登录） |

## 已知限制（v0.1）

- 只做 `session/prompt` 一轮一问；多轮靠同一个 session 继续发
- 不转发 `fs/*`、`terminal/*` 请求（能力声明为 false）
- 登录后**自动重试那一问**还没做（现在只抛 `authRequired`，由客户端决定）
- 上下文多轮复用同一个 session，但没有做历史压缩
- 协议是**按行 JSON**，不是标准 ACP；客户端不需要懂 ACP，这正是目的

## 目录

```
src/main.rs            # 全部逻辑：进程管理、协议转译、事件分发
clients/typescript/    # Node 薄客户端（无依赖）
clients/python/        # Python 薄客户端（无依赖）
tests/fake_agent.py    # 最小 ACP agent，用来验证转译
tests/drive.py         # 驱动脚本：起 host、发命令、自动批准权限
.github/workflows/     # ci（构建+冒烟）与 release（四平台产物）
```
