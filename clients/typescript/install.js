// 安装时把对应平台的 acp-host 二进制拉到 vendor/，用户不用另装 Rust 或手动下包
import { createHash } from "node:crypto";
import { mkdirSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = process.env.ACP_HOST_VERSION || "0.1.7";
const REPO = "thatwzzh/acp-host";
const root = dirname(fileURLToPath(import.meta.url));
const vendor = join(root, "vendor");
const binary = join(vendor, "acp-host");

if (process.env.ACP_HOST_SKIP_DOWNLOAD === "1") process.exit(0);
if (existsSync(binary)) process.exit(0);

const platform = process.platform === "darwin" ? "apple-darwin"
  : process.platform === "linux" ? "unknown-linux-gnu" : null;
const arch = process.arch === "arm64" ? "aarch64" : process.arch === "x64" ? "x86_64" : null;
if (!platform || !arch) {
  console.warn(`[acp-host] 不支持的平台 ${process.platform}/${process.arch}，请手动设置 ACP_HOST_BIN`);
  process.exit(0);
}
const target = `${arch}-${platform}`;
const base = `https://github.com/${REPO}/releases/download/v${VERSION}`;

mkdirSync(vendor, { recursive: true });
const tgz = join(vendor, "acp-host.tar.gz");

async function get(url) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`${res.status} ${url}`);
  return Buffer.from(await res.arrayBuffer());
}

try {
  const [archive, checksumText] = await Promise.all([
    get(`${base}/acp-host-${target}.tar.gz`),
    get(`${base}/acp-host-${target}.tar.gz.sha256`).then(String),
  ]);
  const expected = checksumText.trim().split(/\s+/)[0];
  const actual = createHash("sha256").update(archive).digest("hex");
  if (expected && expected !== actual) throw new Error(`校验和不符：${actual} != ${expected}`);
  writeFileSync(tgz, archive);
  execFileSync("tar", ["xzf", tgz, "-C", vendor, "acp-host"]);
  rmSync(tgz, { force: true });
  console.log(`[acp-host] 已就绪：${target}`);
} catch (error) {
  console.warn(`[acp-host] 下载失败（${error.message}）。可手动设置 ACP_HOST_BIN 指向二进制。`);
}
