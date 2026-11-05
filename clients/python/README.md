# acp-host-client（Python）

[`acp-host`](https://github.com/thatwzzh/acp-host) 的薄客户端：起一个进程、按行收发 JSON。
**不需要装 Rust**——首次调用时会自动下载对应平台的二进制。

```bash
pip install acp-host-client
```

```python
from acp_host import AcpHost

host = AcpHost()
for piece in host.ask("你好", agent="workbuddy"):
    print(piece, end="")
host.close()
```

需要人工批准工具调用时：

```python
for piece in host.ask("看一下 README", on_permission=lambda event: "allow"):
    print(piece, end="")
```

## 二进制从哪来

按这个顺序找，**先看自己的缓存再看 PATH**——避免机器上躺着一个旧版把新包顶掉：

1. 环境变量 `ACP_HOST_BIN`
2. `~/.cache/acp-host/<版本>/acp-host`（本包下的，版本与包对齐）
3. PATH 上的 `acp-host`
4. 都没有就下载（带 sha256 校验）

想用别的版本：`ACP_HOST_VERSION=0.1.4` 或直接设 `ACP_HOST_BIN`。

## 其它安装方式

```bash
brew tap thatwzzh/acp-host && brew install acp-host   # macOS / Linux
npm i acp-host-client                                 # Node / TypeScript
```

协议细节、agent 配置、许可（MIT）见主仓库：https://github.com/thatwzzh/acp-host
