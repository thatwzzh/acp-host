#!/usr/bin/env python3
"""最小 ACP agent，用来验证 acp-host 的协议转译。"""
import json
import sys

for line in sys.stdin:
    if not line.strip():
        continue
    msg = json.loads(line)
    method = msg.get("method")

    if method == "initialize":
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"],
                          "result": {"protocolVersion": 1, "agentCapabilities": {}}}), flush=True)
    elif method == "session/new":
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"],
                          "result": {"sessionId": "fake-1"}}), flush=True)
    elif method == "session/prompt":
        sid = msg["params"]["sessionId"]
        for chunk in ["你好，", "我是假 agent。"]:
            print(json.dumps({"jsonrpc": "2.0", "method": "session/update",
                              "params": {"sessionId": sid,
                                         "update": {"sessionUpdate": "agent_message_chunk",
                                                    "content": {"type": "text", "text": chunk}}}}), flush=True)
        print(json.dumps({"jsonrpc": "2.0", "id": 9001, "method": "session/request_permission",
                          "params": {"sessionId": sid, "toolCall": {"title": "读取 /tmp/a.txt"},
                                     "options": [{"optionId": "allow", "name": "允许"},
                                                 {"optionId": "deny", "name": "拒绝"}]}}), flush=True)
        sys.stdin.readline()          # 等客户端回权限
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"],
                          "result": {"stopReason": "end_turn"}}), flush=True)
