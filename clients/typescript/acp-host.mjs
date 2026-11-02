// acp-host 薄客户端（无依赖，Node 18+）
// 用法见 https://github.com/thatwzzh/acp-host
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** 优先用安装时下到 vendor/ 的二进制，其次交给 PATH */
function defaultBinary() {
  const bundled = join(dirname(fileURLToPath(import.meta.url)), "vendor", "acp-host");
  return existsSync(bundled) ? bundled : "acp-host";
}

export class AcpHost {
  constructor({ binary = process.env.ACP_HOST_BIN || defaultBinary(), cwd = process.cwd() } = {}) {
    this.binary = binary;
    this.cwd = cwd;
    this.proc = null;
    this.nextSession = 0;
    this.buffer = "";
    this.routes = new Map();
  }

  start() {
    if (this.proc) return this;
    this.proc = spawn(this.binary, [], { stdio: ["pipe", "pipe", "inherit"] });
    this.proc.stdout.setEncoding("utf8");
    this.proc.stdout.on("data", (chunk) => this.#consume(chunk));
    return this;
  }

  #consume(chunk) {
    this.buffer += chunk;
    let index;
    while ((index = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, index);
      this.buffer = this.buffer.slice(index + 1);
      if (!line.trim()) continue;
      let event;
      try { event = JSON.parse(line); } catch { continue; }
      const route = this.routes.get(event.session);
      if (route) route.push(event);
    }
  }

  #send(obj) {
    this.proc.stdin.write(JSON.stringify(obj) + "\n");
  }

  /** 问一句，返回 AsyncIterable<string>；权限请求走 onPermission */
  async *ask(text, { agent = "workbuddy", cwd = this.cwd, onPermission } = {}) {
    this.start();
    const session = `s${++this.nextSession}`;
    const queue = [];
    const waiters = [];
    this.routes.set(session, {
      push: (event) => {
        if (waiters.length) waiters.shift()(event);
        else queue.push(event);
      },
    });
    const next = () => queue.length
      ? Promise.resolve(queue.shift())
      : new Promise((resolve) => waiters.push(resolve));

    this.#send({ id: "1", method: "session.start", params: { agent, cwd } });

    try {
      while (true) {
        const event = await next();
        if (event.event === "ready") {
          this.#send({ id: "2", method: "session.prompt", params: { session, text } });
        } else if (event.event === "text") {
          yield event.text;
        } else if (event.event === "permission") {
          const decision = onPermission ? await onPermission(event) : "reject";
          this.#send({ id: "3", method: "permission",
                       params: { session, request: event.request, option: decision } });
        } else if (event.event === "done") {
          return;
        } else if (event.event === "error") {
          throw new Error(typeof event.message === "string" ? event.message : JSON.stringify(event.message));
        }
      }
    } finally {
      this.routes.delete(session);
    }
  }

  close() {
    this.proc?.kill();
    this.proc = null;
  }
}
