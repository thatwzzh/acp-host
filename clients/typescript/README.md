# acp-host-client（Node / TypeScript）

[`acp-host`](https://github.com/thatwzzh/acp-host) 的薄客户端：起一个进程、按行收发 JSON。
**不需要装 Rust**——`npm install` 时会自动下载对应平台的二进制。

```bash
npm i acp-host-client
```

```js
import { AcpHost } from "acp-host-client";

const host = new AcpHost();
for await (const piece of host.ask("你好", { agent: "workbuddy" })) {
  process.stdout.write(piece);
}
host.close();
```

需要人工批准工具调用时：

```js
for await (const piece of host.ask("看一下 README", {
  agent: "workbuddy",
  onPermission: async (event) => "allow",   // 或 "allow_always" / "reject"
})) {
  process.stdout.write(piece);
}
```

## 它做了什么

- 常驻子进程，会话按 id 分流（并发提问不会串答案）
- 把 `text` / `thought` / `tool` / `permission` 事件转成回调
- 二进制查找：`ACP_HOST_BIN` → 包内 `vendor/` → PATH

## 其它安装方式

```bash
brew tap thatwzzh/acp-host && brew install acp-host   # macOS / Linux
pip install acp-host-client                           # Python
```

协议细节、agent 配置、许可（MIT）见主仓库：https://github.com/thatwzzh/acp-host
