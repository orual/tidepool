use std::collections::HashMap;

use core_bridge::ToCore;
use core_bridge_derive::FromCore;
use core_effect::{EffectContext, EffectError, EffectHandler};
use core_eval::value::Value;

// === Tag 0: Repl ===

#[derive(FromCore)]
pub enum ReplReq {
    #[core(name = "ReadLine")]
    ReadLine,
    #[core(name = "Display")]
    Display(String),
}

enum InputSource {
    Interactive(rustyline::DefaultEditor),
    File { lines: Vec<String>, pos: usize },
}

pub struct ReplHandler {
    source: InputSource,
}

impl ReplHandler {
    pub fn new() -> Self {
        ReplHandler {
            source: InputSource::Interactive(rustyline::DefaultEditor::new().unwrap()),
        }
    }

    pub fn from_file(path: &str) -> Self {
        let contents = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
        let lines: Vec<String> = contents
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        ReplHandler {
            source: InputSource::File { lines, pos: 0 },
        }
    }
}

impl EffectHandler for ReplHandler {
    type Request = ReplReq;

    fn handle(&mut self, req: ReplReq, cx: &EffectContext) -> Result<Value, EffectError> {
        match req {
            ReplReq::ReadLine => {
                match &mut self.source {
                    InputSource::Interactive(editor) => {
                        // Loop on parse errors so the user can retry
                        loop {
                            match editor.readline("tide> ") {
                                Ok(line) => {
                                    let t = line.trim().to_string();
                                    if t.is_empty() {
                                        continue;
                                    }
                                    editor.add_history_entry(&t).ok();
                                    match crate::parser::parse(&t) {
                                        Ok(expr) => {
                                            let val = expr.to_value(cx.table()).map_err(|e| {
                                                EffectError::Handler(format!("ToCore failed: {:?}", e))
                                            })?;
                                            break cx.respond(Some(val));
                                        }
                                        Err(e) => {
                                            eprintln!("Parse error: {}", e);
                                            // loop: re-prompt
                                        }
                                    }
                                }
                                Err(_) => break cx.respond(None::<Value>),
                            }
                        }
                    }
                    InputSource::File { lines, pos } => {
                        if *pos < lines.len() {
                            let text = lines[*pos].clone();
                            *pos += 1;
                            match crate::parser::parse(&text) {
                                Ok(expr) => {
                                    let val = expr.to_value(cx.table()).map_err(|e| {
                                        EffectError::Handler(format!("ToCore failed: {:?}", e))
                                    })?;
                                    cx.respond(Some(val))
                                }
                                Err(e) => {
                                    eprintln!("Parse error: {}", e);
                                    cx.respond(None::<Value>)
                                }
                            }
                        } else {
                            cx.respond(None::<Value>)
                        }
                    }
                }
            }
            ReplReq::Display(s) => {
                println!("{}", s);
                cx.respond(())
            }
        }
    }
}

// === Tag 1: Console ===

#[derive(FromCore)]
pub enum ConsoleReq {
    #[core(name = "Print")]
    Print(String),
}

pub struct ConsoleHandler;

impl EffectHandler for ConsoleHandler {
    type Request = ConsoleReq;

    fn handle(&mut self, req: ConsoleReq, cx: &EffectContext) -> Result<Value, EffectError> {
        match req {
            ConsoleReq::Print(s) => {
                println!("{}", s);
                cx.respond(())
            }
        }
    }
}

// === Tag 2: Env ===
// Env stores TVal values, but TVal is recursive and hard to deserialize.
// We keep Values opaque — just clone them through.

#[derive(FromCore)]
pub enum EnvReq {
    #[core(name = "EnvLookup")]
    EnvLookup(String),
    #[core(name = "EnvExtend")]
    EnvExtend(String, Value),
}

pub struct EnvHandler {
    env: HashMap<String, Value>,
}

impl EnvHandler {
    pub fn new() -> Self {
        EnvHandler {
            env: HashMap::new(),
        }
    }
}

impl EffectHandler for EnvHandler {
    type Request = EnvReq;

    fn handle(&mut self, req: EnvReq, cx: &EffectContext) -> Result<Value, EffectError> {
        match req {
            EnvReq::EnvLookup(key) => {
                let result = self.env.get(&key).cloned();
                cx.respond(result)
            }
            EnvReq::EnvExtend(key, val) => {
                self.env.insert(key, val);
                cx.respond(())
            }
        }
    }
}

// === Tag 3: Net ===

#[derive(FromCore)]
pub enum NetReq {
    #[core(name = "HttpGet")]
    HttpGet(String),
}

pub struct NetHandler;

impl EffectHandler for NetHandler {
    type Request = NetReq;

    fn handle(&mut self, req: NetReq, cx: &EffectContext) -> Result<Value, EffectError> {
        match req {
            NetReq::HttpGet(url) => {
                let body = ureq::get(&url)
                    .call()
                    .map_err(|e| EffectError::Handler(format!("HTTP error: {}", e)))?
                    .into_string()
                    .map_err(|e| EffectError::Handler(format!("Read error: {}", e)))?;
                cx.respond(body)
            }
        }
    }
}

// === Tag 4: Fs ===

#[derive(FromCore)]
pub enum FsReq {
    #[core(name = "FsRead")]
    FsRead(String),
    #[core(name = "FsWrite")]
    FsWrite(String, String),
}

pub struct FsHandler;

impl EffectHandler for FsHandler {
    type Request = FsReq;

    fn handle(&mut self, req: FsReq, cx: &EffectContext) -> Result<Value, EffectError> {
        match req {
            FsReq::FsRead(path) => {
                let contents = std::fs::read_to_string(&path)
                    .map_err(|e| EffectError::Handler(format!("fs read error: {}", e)))?;
                cx.respond(contents)
            }
            FsReq::FsWrite(path, contents) => {
                std::fs::write(&path, &contents)
                    .map_err(|e| EffectError::Handler(format!("fs write error: {}", e)))?;
                cx.respond(())
            }
        }
    }
}
