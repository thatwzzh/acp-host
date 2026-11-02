//! acp-host：把本机的 ACP agent 包成一个可被任意语言调用的守护进程。
//!
//! 设计目标只有一条：**别的语言不用懂 ACP**。
//! 它读 stdin 的逐行 JSON 命令，把事件写到 stdout，一行一个 JSON，进程本身无状态依赖。
//!
//! 协议（详见 README）：
//!   → {"id":"1","method":"agents.list"}
//!   → {"id":"2","method":"session.start","params":{"agent":"codebuddy","cwd":"/tmp"}}
//!   → {"id":"3","method":"session.prompt","params":{"session":"s1","text":"你好"}}
//!   ← {"event":"text","session":"s1","text":"…"}
//!   ← {"event":"done","session":"s1","stopReason":"end_turn"}

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use serde_json::{json, Value};

#[derive(Debug)]
enum Incoming {
    StdinClosed,
    Command(Value),
    Agent(String, Value),
    Stderr(String, String),
}

enum Pending {
    Initialize,
    NewSession,
    Prompt,
    Permission { request: String },
    Authenticate,
    SetModel { model: String, session_id: String },
    SetModelLegacy { model: String, session_id: String },
    SetMode,
    SetOption,
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    next_id: u64,
    pending: HashMap<u64, Pending>,
    acp_session: Option<String>,
    cwd: String,
    auth_methods: Value,
    models: Value,
    modes: Value,
    config_options: Value,
    auth_attempted: bool,
    last_prompt: Option<String>,
}

impl Session {
    fn send(&mut self, method: &str, params: Value, pending: Pending) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let payload = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let _ = writeln!(self.stdin, "{}", payload);
        let _ = self.stdin.flush();
        trace("send", &payload.to_string());
        self.pending.insert(id, pending);
        id
    }

    fn notify(&mut self, method: &str, params: Value) {
        let payload = json!({"jsonrpc": "2.0", "method": method, "params": params});
        let _ = writeln!(self.stdin, "{}", payload);
        let _ = self.stdin.flush();
    }

    fn reply(&mut self, id: Value, result: Value) {
        let payload = json!({"jsonrpc": "2.0", "id": id, "result": result});
        let _ = writeln!(self.stdin, "{}", payload);
        let _ = self.stdin.flush();
    }

    fn reply_error(&mut self, id: Value, message: &str, code: i64) {
        let payload = json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": code, "message": message}
        });
        let _ = writeln!(self.stdin, "{}", payload);
        let _ = self.stdin.flush();
    }
}

fn main() {
    let (tx, rx) = channel::<Incoming>();
    let _ = SENDER.set(tx.clone());
    spawn_stdin_reader(tx.clone());

    let mut sessions: HashMap<String, Session> = HashMap::new();
    let mut next_session = 0;

    for message in rx {
        match message {
            // 宿主 app 退出了：收摊，别留孤儿进程
            Incoming::StdinClosed => break,
            Incoming::Command(command) => {
                handle_command(command, &mut sessions, &mut next_session);
            }
            Incoming::Agent(session, value) => {
                handle_agent_message(&session, value, &mut sessions);
            }
            Incoming::Stderr(session, line) => {
                emit(json!({"event": "stderr", "session": session, "text": line}));
            }
        }
    }

    for (_, mut session) in sessions {
        let _ = session.child.kill();
    }
}

fn spawn_stdin_reader(tx: Sender<Incoming>) {
    thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(trimmed) {
                Ok(value) => {
                    if tx.send(Incoming::Command(value)).is_err() {
                        break;
                    }
                }
                Err(error) => emit(json!({"event": "error", "message": error.to_string()})),
            }
        }
        // stdin 读到 EOF（父进程没了或主动关了）：通知主循环退出
        let _ = tx.send(Incoming::StdinClosed);
    });
}

fn handle_command(command: Value, sessions: &mut HashMap<String, Session>, next_session: &mut u32) {
    let id = command.get("id").cloned().unwrap_or(Value::Null);
    let method = command.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = command.get("params").cloned().unwrap_or(json!({}));

    match method {
        // 宿主用它确认装的版本对不对（brew 里躺旧版是最难查的一类问题）
        "version" => {
            ok(id, json!({ "version": env!("CARGO_PKG_VERSION") }));
        }
        "agents.list" => {
            ok(id, json!({ "agents": registry() }));
        }
        // 只返回本机真的装了的：宿主 app 可以直接拿它做「接入」列表
        "agents.detect" => {
            let installed: Vec<Value> = registry()
                .into_iter()
                .filter(|agent| command_installed(agent))
                .collect();
            ok(id, json!({ "agents": installed }));
        }
        "session.start" => {
            let agent = params.get("agent").and_then(|v| v.as_str()).unwrap_or("");
            let cwd = params
                .get("cwd")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().display().to_string());

            let Some(entry) = registry().into_iter().find(|a| a["id"] == agent) else {
                fail(id, "未知 agent，先调 agents.list");
                return;
            };
            let Some(command_line) = entry["command"].as_str() else {
                fail(id, "agent 缺少 command");
                return;
            };

            *next_session += 1;
            let key = format!("s{}", next_session);
            match spawn_agent(&key, command_line, &cwd) {
                Ok(session) => {
                    sessions.insert(key.clone(), session);
                    if let Some(session) = sessions.get_mut(&key) {
                        let full = std::env::var("ACP_HOST_FS").is_ok();
                    session.send(
                            "initialize",
                            json!({
                                "protocolVersion": 1,
                                "clientCapabilities": {
                                    "fs": {"readTextFile": full, "writeTextFile": full},
                                    "terminal": full
                                }
                            }),
                            Pending::Initialize,
                        );
                    }
                    ok(id, json!({ "session": key, "agent": agent }));
                }
                Err(error) => fail(id, &format!("启动失败: {error}")),
            }
        }
        "session.prompt" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let Some(session) = sessions.get_mut(key) else {
                fail(id, "没有这个 session");
                return;
            };
            let Some(acp_session) = session.acp_session.clone() else {
                fail(id, "会话还没就绪（initialize / session/new 未完成）");
                return;
            };
            session.last_prompt = Some(text.to_string());
            session.send(
                "session/prompt",
                json!({
                    "sessionId": acp_session,
                    "prompt": [{"type": "text", "text": text}]
                }),
                Pending::Prompt,
            );
            ok(id, json!({ "accepted": true }));
        }
        "session.cancel" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(session) = sessions.get_mut(key) {
                if let Some(acp_session) = session.acp_session.clone() {
                    session.notify("session/cancel", json!({ "sessionId": acp_session }));
                }
                ok(id, json!({ "cancelled": true }));
            } else {
                fail(id, "没有这个 session");
            }
        }
        "permission" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            let request = params.get("request").and_then(|v| v.as_str()).unwrap_or("");
            let decision = params.get("decision").and_then(|v| v.as_str()).unwrap_or("deny");
            let option = params.get("option").and_then(|v| v.as_str());
            let Some(session) = sessions.get_mut(key) else {
                fail(id, "没有这个 session");
                return;
            };

            let acp_id = session.pending.iter().find_map(|(acp_id, pending)| match pending {
                Pending::Permission { request: stored } if stored == request => Some(*acp_id),
                _ => None,
            });
            let Some(acp_id) = acp_id else {
                fail(id, "没有待处理的权限请求");
                return;
            };
            // 客户端能给出具体 optionId 就用它（allow_always / allow / reject），
            // 否则退回二值判断
            let option_id = option
                .map(|s| s.to_string())
                .unwrap_or_else(|| if decision == "allow" { "allow".into() } else { "reject".into() });
            let outcome = json!({"outcome": "selected", "optionId": option_id});
            let payload = json!({"jsonrpc": "2.0", "id": acp_id,
                                 "result": {"outcome": outcome}});
            let _ = writeln!(session.stdin, "{}", payload);
            let _ = session.stdin.flush();
            session.pending.remove(&acp_id);
            ok(id, json!({ "applied": true }));
        }
        "auth" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            let method = params.get("method").and_then(|v| v.as_str()).unwrap_or("");
            let Some(session) = sessions.get_mut(key) else {
                fail(id, "没有这个 session");
                return;
            };
            let chosen = if method.is_empty() {
                session.auth_methods
                    .as_array()
                    .and_then(|list| list.first())
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                method.to_string()
            };
            if chosen.is_empty() {
                fail(id, "这个 agent 没有可用的登录方式");
                return;
            }
            session.send("authenticate", json!({ "methodId": chosen }), Pending::Authenticate);
            ok(id, json!({ "authenticating": chosen }));
        }
        "session.setModel" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            let model = params.get("model").and_then(|v| v.as_str()).unwrap_or("");
            let Some(session) = sessions.get_mut(key) else {
                fail(id, "没有这个 session");
                return;
            };
            let Some(acp_session) = session.acp_session.clone() else {
                fail(id, "会话还没就绪");
                return;
            };
            let config_id = model_config_id(&session.config_options);
            session.send(
                "session/set_config_option",
                json!({"sessionId": acp_session, "configId": config_id, "value": model}),
                Pending::SetModel { model: model.to_string(), session_id: acp_session },
            );
            ok(id, json!({ "requested": model }));
        }
        "session.setMode" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("");
            let Some(session) = sessions.get_mut(key) else {
                fail(id, "没有这个 session");
                return;
            };
            let Some(acp_session) = session.acp_session.clone() else {
                fail(id, "会话还没就绪");
                return;
            };
            session.send("session/set_mode",
                         json!({"sessionId": acp_session, "modeId": mode}),
                         Pending::SetMode);
            ok(id, json!({ "requested": mode }));
        }
        "session.setOption" => {
            let key = params.get("session").and_then(|v| v.as_str()).unwrap_or("");
            let config_id = params.get("configId").and_then(|v| v.as_str()).unwrap_or("");
            let value = params.get("value").cloned().unwrap_or(Value::Null);
            let Some(session) = sessions.get_mut(key) else {
                fail(id, "没有这个 session");
                return;
            };
            let Some(acp_session) = session.acp_session.clone() else {
                fail(id, "会话还没就绪");
                return;
            };
            session.send("session/set_config_option",
                         json!({"sessionId": acp_session, "configId": config_id,
                                "value": value}),
                         Pending::SetOption);
            ok(id, json!({ "requested": config_id }));
        }
        "shutdown" => {
            for (_, mut session) in sessions.drain() {
                let _ = session.child.kill();
            }
            ok(id, json!({ "bye": true }));
            std::process::exit(0);
        }
        other => fail(id, &format!("未知方法: {other}")),
    }
}

fn handle_agent_message(key: &str, value: Value, sessions: &mut HashMap<String, Session>) {
    let Some(session) = sessions.get_mut(key) else { return };

    // 带 method 的是对方发起的请求/通知，哪怕它也带 id（权限请求就带）
    if value.get("method").is_some() {
        handle_agent_request(key, value, session);
        return;
    }

    // 我们发出去的请求的回应
    if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
        let Some(pending) = session.pending.remove(&id) else { return };
        let result = value.get("result").cloned().unwrap_or(Value::Null);
        let error = value.get("error").cloned();

        match pending {
            Pending::Initialize => {
                if let Some(error) = error {
                    emit(json!({"event": "error", "session": key, "message": error}));
                    return;
                }
                session.auth_methods = result
                    .get("authMethods")
                    .cloned()
                    .or_else(|| result.pointer("/agentCapabilities/auth/methods").cloned())
                    .unwrap_or(Value::Null);

                // 默认**不**主动登录：WorkBuddy 这类 agent 本身已经登录过，
                // 再替它调一次 iOA 会弹登录。只有真的被要求登录（-32000）才走 authenticate。
                // 需要前置认证的 agent（Cursor）用 ACP_HOST_AUTO_AUTH=1 打开。
                let auto = std::env::var("ACP_HOST_AUTO_AUTH").is_ok();
                let method = session
                    .auth_methods
                    .as_array()
                    .and_then(|list| list.first())
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                if auto {
                    if let Some(method) = method {
                        session.send("authenticate", json!({"methodId": method}),
                                     Pending::Authenticate);
                        return;
                    }
                }
                let cwd = session.cwd.clone();
                session.send("session/new", json!({"cwd": cwd, "mcpServers": []}),
                             Pending::NewSession);
            }
            Pending::NewSession => {
                if let Some(error) = error {
                    let needs_auth = needs_auth(&error);
                    if needs_auth && !session.auth_attempted {
                        session.auth_attempted = true;
                        if let Some(method) = first_auth_method(&session.auth_methods) {
                            emit(json!({"event": "authRequired", "session": key,
                                        "method": method}));
                            session.send("authenticate", json!({"methodId": method}),
                                         Pending::Authenticate);
                            return;
                        }
                    }
                    emit(json!({"event": "error", "session": key, "message": error}));
                    return;
                }
                session.acp_session = result
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                session.models = result.get("models").cloned().unwrap_or(Value::Null);
                session.modes = result.get("modes").cloned().unwrap_or(Value::Null);
                session.config_options = result.get("configOptions").cloned().unwrap_or(Value::Null);
                emit(json!({
                    "event": "ready",
                    "session": key,
                    "acpSession": session.acp_session,
                    "agent": result.get("agentInfo").cloned().unwrap_or(Value::Null),
                    "models": session.models,
                    "modes": session.modes,
                    "configOptions": session.config_options,
                    "authMethods": session.auth_methods
                }));
            }
            Pending::Prompt => {
                if let Some(error) = error {
                    if needs_auth(&error) && !session.auth_attempted {
                        session.auth_attempted = true;
                        if let Some(method) = first_auth_method(&session.auth_methods) {
                            emit(json!({"event": "authRequired", "session": key,
                                        "method": method}));
                            session.send("authenticate", json!({"methodId": method}),
                                         Pending::Authenticate);
                            return;
                        }
                    }
                    let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                    emit(json!({
                        "event": "error", "session": key, "code": code, "message": error,
                        "authRequired": needs_auth(&error),
                        "authMethods": session.auth_methods
                    }));
                } else {
                    let stop = result
                        .get("stopReason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("end_turn");
                    emit(json!({"event": "done", "session": key, "stopReason": stop}));
                }
            }
            Pending::Permission { .. } => {}
            Pending::Authenticate => {
                if let Some(error) = error {
                    emit(json!({"event": "authFailed", "session": key, "error": error}));
                } else {
                    emit(json!({"event": "authenticated", "session": key}));
                }
                // 登录完接着往下走：还没开会话就开，已经在会话里就把刚才那一问重发
                if session.acp_session.is_none() {
                    let cwd = session.cwd.clone();
                    session.send("session/new", json!({"cwd": cwd, "mcpServers": []}),
                                 Pending::NewSession);
                } else if let Some(text) = session.last_prompt.clone() {
                    if let Some(acp_session) = session.acp_session.clone() {
                        session.send("session/prompt",
                                     json!({"sessionId": acp_session,
                                            "prompt": [{"type": "text", "text": text}]}),
                                     Pending::Prompt);
                    }
                }
            }
            Pending::SetModel { model, session_id } => {
                if let Some(error) = error {
                    // 有的 agent 只实现了 legacy 的 session/set_model
                    session.send("session/set_model",
                                 json!({"sessionId": session_id, "modelId": model}),
                                 Pending::SetModelLegacy { model, session_id });
                    let _ = error;
                } else {
                    emit(json!({"event": "modelChanged", "session": key, "model": model}));
                }
            }
            Pending::SetModelLegacy { model, .. } => {
                if let Some(error) = error {
                    emit(json!({"event": "error", "session": key,
                                "message": format!("切换模型失败: {error}")}));
                } else {
                    emit(json!({"event": "modelChanged", "session": key, "model": model}));
                }
            }
            Pending::SetMode => {
                if let Some(error) = error {
                    emit(json!({"event": "error", "session": key, "message": error}));
                } else {
                    emit(json!({"event": "modeChanged", "session": key}));
                }
            }
            Pending::SetOption => {
                if let Some(error) = error {
                    emit(json!({"event": "error", "session": key, "message": error}));
                } else {
                    emit(json!({"event": "optionChanged", "session": key}));
                }
            }
        }
        return;
    }
}

fn handle_agent_request(key: &str, value: Value, session: &mut Session) {
    let method = value.get("method").and_then(|m| m.as_str()).unwrap_or("");
    match method {
        "_auth/status_update" => {
            emit(json!({"event": "authStatus", "session": key,
                        "status": value.pointer("/params/authStatus").cloned().unwrap_or(Value::Null)}));
        }
        "session/update" => {
            let params = value.get("params").cloned().unwrap_or(Value::Null);
            let update = params.get("update").cloned().unwrap_or(Value::Null);
            let kind = update.get("sessionUpdate").and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "agent_message_chunk" => {
                    let text = text_of(&update);
                    emit(json!({"event": "text", "session": key, "text": text}));
                }
                "agent_thought_chunk" => {
                    let text = text_of(&update);
                    emit(json!({"event": "thought", "session": key, "text": text}));
                }
                "tool_call" | "tool_call_update" => {
                    emit(json!({
                        "event": "tool",
                        "session": key,
                        "id": update.get("toolCallId").cloned().unwrap_or(Value::Null),
                        "title": update.get("title").cloned().unwrap_or(Value::Null),
                        "status": update.get("status").cloned().unwrap_or(Value::Null)
                    }));
                }
                _ => emit(json!({"event": "update", "session": key, "kind": kind})),
            }
        }
        // 3. agent 反过来请求我们（权限），转成事件，等客户端回 permission
        "session/request_permission" => {
            let params = value.get("params").cloned().unwrap_or(Value::Null);
            let request_id = format!("{key}-p{}", session.next_id + 1);
            let title = permission_title(&params);
            if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                session.pending.insert(id, Pending::Permission { request: request_id.clone() });
                emit(json!({
                    "event": "permission",
                    "session": key,
                    "request": request_id,
                    "title": title,
                    "options": params.get("options").cloned().unwrap_or(Value::Null)
                }));
            }
        }
        "fs/read_text_file" | "fs/write_text_file" | "terminal/create" => {
            if let Some(id) = value.get("id").cloned() {
                session.reply_error(id, "acp-host 不提供文件/终端能力", -32601);
            }
        }
        other => {
            // 关键：带 id 的请求必须回一个响应，哪怕是我们不认识的方法。
            // 沉默会让对方一直等，最后把整条流关掉（Cursor 报 WritableIterable is closed 就是这么来的）。
            if let Some(id) = value.get("id").cloned() {
                session.reply_error(id, &format!("acp-host 未实现: {other}"), -32601);
            } else {
                emit(json!({"event": "update", "session": key, "kind": other}));
            }
        }
    }
}

/// 这个错误是不是「要登录」
fn needs_auth(error: &Value) -> bool {
    let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    let message = error.get("message").and_then(|v| v.as_str()).unwrap_or("");
    code == -32000 || message.to_lowercase().contains("auth")
}

fn first_auth_method(methods: &Value) -> Option<String> {
    methods
        .as_array()
        .and_then(|list| list.first())
        .and_then(|m| m.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 权限请求的标题。各家 agent 给的信息不一样：有的给 title，
/// 有的只给 toolCall.rawInput（WorkBuddy 就是后者），尽量拼出人话。
fn permission_title(params: &Value) -> String {
    if let Some(title) = params.pointer("/toolCall/title").and_then(|v| v.as_str()) {
        return title.to_string();
    }
    let tool = params
        .pointer("/toolCall/_meta/codebuddy.ai~1toolName")
        .and_then(|v| v.as_str())
        .or_else(|| params.pointer("/toolCall/kind").and_then(|v| v.as_str()))
        .unwrap_or("工具调用");
    let detail = params
        .pointer("/toolCall/rawInput/command")
        .and_then(|v| v.as_str())
        .or_else(|| params.pointer("/toolCall/rawInput/path").and_then(|v| v.as_str()))
        .or_else(|| params.pointer("/toolCall/rawInput/description").and_then(|v| v.as_str()))
        .unwrap_or("");
    if detail.is_empty() { tool.to_string() } else { format!("{tool}: {detail}") }
}

fn model_config_id(options: &Value) -> String {
    if let Some(list) = options.as_array() {
        for option in list {
            let id = option.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let category = option.get("category").and_then(|v| v.as_str()).unwrap_or("");
            if id == "model" || category == "model" {
                return id.to_string();
            }
        }
    }
    "model".to_string()
}

fn text_of(update: &Value) -> String {
    update
        .pointer("/content/text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn spawn_agent(key: &str, command_line: &str, cwd: &str) -> anyhow::Result<Session> {
    let mut child = Command::new("/bin/sh")
        .arg("-lc")
        .arg(command_line)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");

    // 每个 session 起两个读取线程；事件统一走全局通道
    let key_out = key.to_string();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            trace("recv", &line);
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if let Some(sender) = SENDER.get() {
                    let _ = sender.send(Incoming::Agent(key_out.clone(), value));
                }
            }
        }
    });

    let key_err = key.to_string();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            if let Some(sender) = SENDER.get() {
                let _ = sender.send(Incoming::Stderr(key_err.clone(), line));
            }
        }
    });

    Ok(Session {
        child,
        stdin,
        next_id: 0,
        pending: HashMap::new(),
        acp_session: None,
        cwd: cwd.to_string(),
        auth_methods: Value::Null,
        models: Value::Null,
        modes: Value::Null,
        config_options: Value::Null,
        auth_attempted: false,
        last_prompt: None,
    })
}

static SENDER: std::sync::OnceLock<Sender<Incoming>> = std::sync::OnceLock::new();

/// ACP_HOST_TRACE=1 时把收发原文打到 stderr，排查字段用
fn trace(direction: &str, payload: &str) {
    if std::env::var("ACP_HOST_TRACE").is_ok() {
        let limit = std::env::var("ACP_HOST_TRACE_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(1200);
        let text = if payload.len() > limit { format!("{}…", &payload[..limit]) } else { payload.to_string() };
        eprintln!("[{direction}] {text}");
    }
}

fn emit(value: Value) {
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "{}", value);
    let _ = stdout.flush();
}

fn ok(id: Value, result: Value) {
    emit(json!({"id": id, "ok": true, "result": result}));
}

fn fail(id: Value, message: &str) {
    emit(json!({"id": id, "ok": false, "error": message}));
}

/// 命令能不能在本机找到：绝对路径看文件，其它看 PATH 里有没有第一个词
fn command_installed(agent: &Value) -> bool {
    let Some(command) = agent.get("command").and_then(|v| v.as_str()) else { return false };
    // workbuddy 那条是 env ... '绝对路径' ... 的形式，直接看引号里的可执行文件
    if let Some(start) = command.find('\'') {
        if let Some(end) = command[start + 1..].find('\'') {
            let path = &command[start + 1..start + 1 + end];
            return std::path::Path::new(path).exists();
        }
    }
    let program = command.split_whitespace().next().unwrap_or("");
    if program.starts_with('/') {
        return std::path::Path::new(program).exists();
    }
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|dir| std::path::Path::new(dir).join(program).exists())
}

fn registry() -> Vec<Value> {
    let mut agents = default_agents();
    if let Some(custom) = custom_agents() {
        agents.extend(custom);
    }
    agents
}

fn default_agents() -> Vec<Value> {
    let mut list = vec![
        json!({"id": "cursor", "name": "Cursor CLI", "command": "agent acp"}),
        json!({"id": "codebuddy", "name": "CodeBuddy Code", "command": "codebuddy --acp"}),
        json!({"id": "claude", "name": "Claude Code",
               "command": "npx -y @agentclientprotocol/claude-agent-acp"}),
        json!({"id": "codex", "name": "Codex",
               "command": "npx -y @agentclientprotocol/codex-acp"}),
        json!({"id": "gemini", "name": "Gemini CLI", "command": "gemini --experimental-acp"}),
        json!({"id": "opencode", "name": "OpenCode", "command": "opencode acp"}),
        json!({"id": "qwen", "name": "Qwen Code", "command": "qwen --experimental-acp"}),
        json!({"id": "kiro", "name": "Kiro", "command": "kiro-cli acp"}),
        json!({"id": "qoder", "name": "Qoder", "command": "qodercli --acp"}),
    ];
    // WorkBuddy 自带的 lite 变体：要 --serve + 随机端口 + 独立 HOME + 去掉宿主代理，
    // 否则会撞端口（EADDRINUSE）或者卡在它自己的配置拉取上。
    let home = std::env::var("HOME").unwrap_or_default();
    let bundled = format!(
        "{home}/Applications/WorkBuddy.app/Contents/Resources/app.asar.unpacked/cli/bin/codebuddy"
    );
    let system = "/Applications/WorkBuddy.app/Contents/Resources/app.asar.unpacked/cli/bin/codebuddy";
    let cli = if std::path::Path::new(system).exists() {
        Some(system.to_string())
    } else if std::path::Path::new(&bundled).exists() {
        Some(bundled)
    } else {
        None
    };
    if let Some(cli) = cli {
        let agent_home = format!("{home}/Library/Application Support/acp-host/agent-home");
        let _ = std::fs::create_dir_all(&agent_home);
        let command = format!(
            "env -u HTTP_PROXY -u HTTPS_PROXY -u http_proxy -u https_proxy -u ALL_PROXY \
             HOME='{agent_home}' CODEBUDDY_FORCE_LITE_WB_BUNDLE=1 \
             '{cli}' --serve --port 0 --acp"
        );
        // 免打扰：宿主设置了 ACP_HOST_SKIP_PERMISSIONS=1 就给 CLI 补这个开关
        let command = if std::env::var("ACP_HOST_SKIP_PERMISSIONS").is_ok() {
            format!("{command} --dangerously-skip-permissions")
        } else {
            command
        };
        list.insert(0, json!({"id": "workbuddy", "name": "WorkBuddy", "command": command}));
    }
    list
}

fn custom_agents() -> Option<Vec<Value>> {
    let path = std::env::var("HOME").ok()? + "/.config/acp-host/agents.json";
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Vec<Value>>(&text).ok()
}
