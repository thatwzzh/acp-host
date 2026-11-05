# acp-host

[中文](README.md) · [English](README.en.md)

**让你的程序能调用电脑里已经装好的 AI 编程助手。**

WorkBuddy、Cursor、Claude、Codex 这些 CLI 都支持一种叫 ACP 的协议，但每接一种语言都要重学一遍：
进程怎么起、登录怎么做、权限怎么回、模型怎么换、各家脾气怎么迁就。

acp-host 把这些事收进一个二进制。你的程序只要会**起进程、按行收发 JSON**，就能用上。

```
你的程序 ──(每行一个 JSON)── acp-host ──(ACP)── workbuddy / cursor / claude / codex
```

## 30 秒跑通

```bash
# 装（任选一种，不需要 Rust）
brew tap thatwzzh/acp-host && brew install acp-host
# 或 npm i -g acp-host-client
# 或 pip install acp-host-client

# 确认能用
acp-host --version

# 看一眼你机器上有哪些 agent
echo '{"id":"1","method":"agents.list"}' | acp-host
```

直接用：

```python
from acp_host import AcpHost

host = AcpHost()
for piece in host.ask("用一句话解释闭包", agent="workbuddy"):
    print(piece, end="")
```

```js
import { AcpHost } from "acp-host-client";

const host = new AcpHost();
for await (const piece of host.ask("用一句话解释闭包", { agent: "workbuddy" })) {
  process.stdout.write(piece);
}
```

不想写客户端？命令行也行：

```bash
echo '{"id":"1","method":"session.start","params":{"agent":"workbuddy","cwd":"."}}' \
  | acp-host
```

## 它替你处理哪些脏活

| 事 | 没有它你得自己写 |
|---|---|
| 起进程、配 PATH、隔离 HOME | 每个 agent 一套 |
| 登录 | 有的必须先 `authenticate`，有的**不能**主动调（否则每问都弹登录） |
| 换模型 / 换模式 / 思考强度 | 新接口 `set_config_option`，旧接口 `set_model`，各家支持不一 |
| 权限往返 | 请求不回应，会话就卡死；`allow_always` 必须原样回传才生效 |
| 流式事件 | 正文 / 思考 / 工具调用 / 工具状态，四种形态 |
| 并发 | 多个提问共用一条管道，按 session 分流，不能串答案 |
| 退出清理 | stdin 关闭时连子进程一起收摊 |

## 加自己的 agent

`~/.config/acp-host/agents.json`：

```json
[
  { "id": "myagent", "name": "我的助手", "command": "myagent --acp" }
]
```

内置 `workbuddy`（自动探测 WorkBuddy.app 里的 CLI）、`codebuddy`、`claude`、`codex`。

## 常用环境变量

| 变量 | 作用 |
|---|---|
| `ACP_HOST_BIN` | 指定二进制路径（客户端用） |
| `ACP_HOST_TRACE=1` | 把协议原文打到 stderr，排查用 |
| `ACP_HOST_AUTO_AUTH=1` | 开会话前主动登录（Cursor 需要；默认关，否则 WorkBuddy 每问都弹登录） |

## 谁在用

[**Sprig（知枝）**](https://github.com/thatwzzh/sprig)：macOS 的划词问答面板。
它把 acp-host 打进 `Sprig.app/Contents/Resources/`，面板只管渲染。
想看「真接进去长什么样」，读它就够了。

## 常见问题

**我直接敲 `acp-host`，怎么什么反应都没有？**
它是等 stdin 的守护进程，直接跑会打印用法。要看到输出，用管道喂 JSON。

**为什么不直接用官方的 ACP SDK？**
SDK 每种语言一套，而且只管协议字段，不管「起进程、登录、权限、各家脾气」这些脏活。
acp-host 是把脏活收掉，不是替代协议。

**权限弹窗能关掉吗？**
能。给 agent 的启动命令加 `--dangerously-skip-permissions`（高危操作仍会问）。
注意：**这不是沙箱**，关掉后 agent 能访问你用户权限下的所有文件。

## 深入

- 协议全表、事件时序、环境变量全表 → [docs/TECHNICAL.md](docs/TECHNICAL.md)
- 发版流程 → [RELEASING.md](RELEASING.md)

MIT © thatwzzh
