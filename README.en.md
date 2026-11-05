# acp-host

[中文](README.md) · [English](README.en.md)

**Let your program use the AI coding assistants already installed on your machine.**

CLIs like WorkBuddy, Cursor, Claude and Codex all speak a protocol called ACP — but every language you integrate means learning the whole dance again: spawning the process, handling login, answering permission requests, switching models, and tolerating each vendor's quirks.

acp-host puts all of that into a single binary. Your program only needs to **spawn a process and read/write one JSON object per line**.

```
your program ──(one JSON per line)── acp-host ──(ACP)── workbuddy / cursor / claude / codex
```

## 30 seconds to a working call

```bash
# Install (pick one; no Rust toolchain needed)
brew tap thatwzzh/acp-host && brew install acp-host
# or: npm i -g acp-host-client
# or: pip install acp-host-client

# Check it works
acp-host --version

# See which agents are available on this machine
echo '{"id":"1","method":"agents.list"}' | acp-host
```

Call it directly:

```python
from acp_host import AcpHost

host = AcpHost()
for piece in host.ask("Explain closures in one sentence", agent="workbuddy"):
    print(piece, end="")
```

```js
import { AcpHost } from "acp-host-client";

const host = new AcpHost();
for await (const piece of host.ask("Explain closures in one sentence", { agent: "workbuddy" })) {
  process.stdout.write(piece);
}
```

No client needed at all — a pipe works too:

```bash
echo '{"id":"1","method":"session.start","params":{"agent":"workbuddy","cwd":"."}}' \
  | acp-host
```

## What it handles for you

| Task | Without it you write yourself |
|---|---|
| Spawning, PATH setup, isolated HOME | Once per agent |
| Login | Some agents need `authenticate` first; others must **not** be logged in again (or every prompt pops a dialog) |
| Model / mode / reasoning level | Newer `set_config_option`, legacy `set_model`, per-vendor support |
| Permission round-trips | Unanswered requests hang the session; `allow_always` must be passed through verbatim to work |
| Streaming events | Text / thoughts / tool calls / tool status, four shapes |
| Concurrency | Many prompts share one pipe, routed by session id so answers never mix |
| Cleanup on exit | Reaping child processes when stdin hits EOF |

## Add your own agent

`~/.config/acp-host/agents.json`:

```json
[
  { "id": "myagent", "name": "My Assistant", "command": "myagent --acp" }
]
```

Built-in: `workbuddy` (auto-detects the CLI inside WorkBuddy.app), `codebuddy`, `claude`, `codex`.

## Common environment variables

| Variable | Purpose |
|---|---|
| `ACP_HOST_BIN` | Point at a specific binary (used by clients) |
| `ACP_HOST_TRACE=1` | Dump raw protocol traffic to stderr |
| `ACP_HOST_AUTO_AUTH=1` | Authenticate before opening a session (needed for Cursor; off by default so WorkBuddy doesn't re-prompt on every prompt) |

## Used by

[**Sprig**](https://github.com/thatwzzh/sprig) — a macOS select-and-ask panel.
It bundles acp-host inside `Sprig.app/Contents/Resources/`; the panel only renders.
If you want to see what a real integration looks like, read that.

## FAQ

**I ran `acp-host` and nothing happened — is it broken?**
It's a daemon waiting on stdin. Run it directly and it prints usage. To get output, pipe JSON into it.

**Why not just use the official ACP SDK?**
The SDKs are one per language and only cover protocol fields — not the messy parts like spawning, login, permissions, and vendor quirks. acp-host removes the mess, not the protocol.

**Can I turn off the permission prompts?**
Yes — add `--dangerously-skip-permissions` to the agent command (high-risk actions still ask).
Note: **this is not a sandbox** — with prompts off, the agent can reach anything your user account can.

## Go deeper

- Full protocol tables, event timing, all environment variables → [docs/TECHNICAL.md](docs/TECHNICAL.md)
- Release process → [RELEASING.md](RELEASING.md)

MIT © thatwzzh
