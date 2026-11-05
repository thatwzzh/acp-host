# 技术方案

给要改代码或自己包一层客户端的人看。只想用的话，看根目录 README 就够。

## 进程模型

```
宿主程序 ──spawn──> acp-host ──spawn──> 各 agent CLI
          一行一个 JSON        ACP / JSON-RPC
```

- **单进程、多会话**：所有会话共用一个 daemon，按 `session` 字段分流事件。
  早先每次提问各起一个 `readabilityHandler`，第二个会把第一个的回调顶掉——表现为「旧答案渲染到新卡片」。
- **常驻**：每问一次重起 node CLI 要多花 1~2 秒冷启动，宿主 app 一般在启动时预热。
- **退出**：stdin 读到 EOF 时退出（主循环还握着 sender 的话，会留下孤儿进程）。

## 请求协议

请求写 stdin，一行一个 JSON：

| method | params | 说明 |
|---|---|---|
| `agents.list` | — | 列出全部 agent（内置 + `~/.config/acp-host/agents.json`） |
| `agents.detect` | — | 只返回本机真装了的（PATH / 绝对路径探测） |
| `version` | — | 返回 `Cargo.toml` 的版本号 |
| `session.start` | `agent`, `cwd` | 开会话，返回 `session` 句柄 |
| `session.prompt` | `session`, `text` | 发一句话，事件异步回来 |
| `session.cancel` | `session` | 取消当前回合 |
| `session.setModel` | `session`, `model` | 先试 `set_config_option`，失败退回 legacy `set_model` |
| `session.setMode` | `session`, `mode` | 换模式（Agent / Plan / Ask） |
| `session.setOption` | `session`, `configId`, `value` | 改任意选项 |
| `auth` | `session`, `method`（可省） | 显式走登录流程 |
| `permission` | `session`, `request`, `option` / `decision` | 回应权限；`option` 原样回传 optionId（`allow_always` 才生效） |
| `shutdown` | — | 收摊 |

## 事件协议

stdout 每行一个 JSON：

```jsonc
{"id":"1","ok":true,"result":{...}}                    // 请求应答
{"event":"ready","session":"s1","acpSession":"…"}
{"event":"text","session":"s1","text":"片段"}
{"event":"thought","session":"s1","text":"思考片段"}
{"event":"tool","session":"s1","id":"…","title":"…","status":"…"}
{"event":"permission","session":"s1","request":"s1-p4","title":"Bash: ls","options":[…]}
{"event":"authenticated","session":"s1"}
{"event":"modelChanged","session":"s1","model":"…"}
{"event":"done","session":"s1","stopReason":"end_turn"}
{"event":"error","session":"s1","code":-32000,"message":{…},"authRequired":true}
```

`session` 字段是分流依据，客户端**必须**按它路由，不能只靠「当前只有一个会话」的假设。

## 时序

```
session.start
  → initialize（clientCapabilities.fs=false, terminal=false）
  → [按需] authenticate            ← 见「登录策略」
  → session/new
  ← ready（带 models / authMethods / configOptions）
session.prompt
  ← text / thought / tool / permission…
  ← done(stopReason)
```

## 登录策略

**默认不主动 `authenticate`**。原因：WorkBuddy 这类 agent 自己已经登录过，
再替它调一次 `iOA` 会每次提问都弹登录框。

改成按需：收到 `-32000` / `Authentication required` 时才认证，成功后
**自动把刚才那一问重发**，用户无感。

需要前置认证的 agent（Cursor）用 `ACP_HOST_AUTO_AUTH=1` 打开。

## 权限标题拼装

各家给的信息不一样：有的给 `toolCall.title`，有的只给 `rawInput`。
现在按这个优先级拼：`toolCall.title` → `_meta["codebuddy.ai/toolName"]` → `rawInput.command/path/description`。

回传时用客户端给的 `optionId`（`allow_always` / `allow` / `reject`），
压成二值会把「总是允许」降级成「仅一次」。

## 各家 agent 的脾气

| agent | 要注意 |
|---|---|
| **WorkBuddy** | lite 变体才稳定：`--serve --port 0 --acp` + `CODEBUDDY_FORCE_LITE_WB_BUNDLE=1` + 独立 HOME + 去掉代理 |
| **Cursor CLI** | `clientCapabilities.fs` 不给就不流式；`set_config_option` 优先；账号额度受限会回 `Upgrade your plan to continue` |
| **Claude Code** | 需要登录；`availableModels` 可能为空 |

## 环境变量全表

| 变量 | 作用 |
|---|---|
| `ACP_HOST_TRACE=1` | 收发原文打到 stderr |
| `ACP_HOST_TRACE_LIMIT=4000` | 单行截断长度（默认 1200） |
| `ACP_HOST_FS=1` | 声明支持文件/终端能力（有的 agent 不给就不流式） |
| `ACP_HOST_AUTO_AUTH=1` | 开会话前主动 authenticate（默认关） |
| `ACP_HOST_SKIP_PERMISSIONS=1` | 给 CLI 补 `--dangerously-skip-permissions` |
| `ACP_HOST_BIN` | 客户端指定二进制路径 |
| `ACP_HOST_VERSION` | 客户端下载哪个版本的二进制 |

## 真实验证记录

**WorkBuddy：全链路跑通**（真实中文回答 + 换模型）：

```
[ready] models=22 current=fast-model
[done] text='我是一个能帮你完成软件工程任务的人工智能编程助手…'
[model] changed -> balanced-model
[done] text='我还是那个能帮你写代码、查 Bug、跑命令的 AI 编程助手，只是换了模型来回答。'
```

**Cursor**：握手、模型清单（32 个）、登录、换模型都通；真正生成受账号额度限制。

> 2026.05 的 Cursor CLI 在 `session/prompt` 后回 `WritableIterable is closed`，
> 升到 2026.10 后消失，变成明确的额度提示。

## 设计取舍

**为什么是子进程，不是链接库**：崩溃隔离（agent 挂了宿主不受影响）、
跨语言零依赖、独立更新（`brew upgrade` 不用重发 app）。

**为什么客户端只透传文本，不透传思考**：思考是按 token 流式推的，
逐字刷到 UI 上既快又读不懂。现在客户端把 `thought` 转成「正在思考 Ns」的状态，
工具调用另起一行留痕。

**二进制查找顺序**：客户端先看自己的版本化缓存，再看 PATH——
否则机器上躺着一个旧版会静默把新包顶掉（0.1.2 那次事故）。

## 目录

```
src/main.rs            # 进程管理、协议转译、事件分发（全部逻辑）
clients/typescript/    # Node 客户端（无依赖）
clients/python/        # Python 客户端（无依赖）
scripts/publish.sh     # 多渠道发布（配置见 .release.json）
tests/                 # 假 agent + 驱动脚本
.github/workflows/     # ci（构建+冒烟）与 release（四平台产物）
Formula/acp-host.rb    # Homebrew formula（tap 仓库里那份的副本）
```

## 已知限制

- 一轮一问；多轮靠同一个 session 继续发，没做历史压缩
- 不转发 `fs/*`、`terminal/*` 请求（能力声明为 false）
- 协议是**按行 JSON**，不是标准 ACP——客户端不需要懂 ACP，这正是目的
