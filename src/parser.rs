use std::num::ParseIntError;
use agon_cpu_emulator::{ DebugCmd, Trigger };

pub enum Cmd {
    Core(DebugCmd),
    UiHelp,
    UiExit
}

// trigger $40000 "hey" pause state

pub fn parse_cmd(mut tokens: std::vec::IntoIter<&str>) -> Option<Cmd> {
    if let Some(tok) = tokens.next() {
        match tok {
            "help" => Some(Cmd::UiHelp),
            "info" => {
                match tokens.next() {
                    Some("breakpoints") => Some(Cmd::Core(DebugCmd::ListTriggers)),
                    _ => None
                }
            }
            "delete" => {
                if let Ok(addr) = parse_number(tokens.next().unwrap_or("")) {
                    Some(Cmd::Core(DebugCmd::DeleteTrigger(addr)))
                } else {
                    println!("delete expects an address argument");
                    None
                }
            }
            "break" => {
                if let Ok(addr) = parse_number(tokens.next().unwrap_or("")) {
                    println!("Setting breakpoint at &{:x}", addr);
                    Some(Cmd::Core(DebugCmd::AddTrigger(Trigger {
                        address: addr,
                        msg: "Cpu paused at breakpoint".to_string(),
                        once: false,
                        actions: vec![]
                    })))
                } else {
                    println!("break <address>");
                    None
                }
            }
            "exit" => Some(Cmd::UiExit),
            "n" | "next" => {
                Some(Cmd::Core(DebugCmd::StepOver))
            }
            "s" | "step" => {
                Some(Cmd::Core(DebugCmd::Step))
            }
            "registers" => {
                Some(Cmd::Core(DebugCmd::GetRegisters))
            }
            "mem" | "memory" => {
                let start_ = parse_number(tokens.next().unwrap_or(""));
                if let Ok(start) = start_ {
                    let len = parse_number(tokens.next().unwrap_or("")).unwrap_or(16);

                    Some(Cmd::Core(DebugCmd::GetMemory { start, len }))
                } else {
                    println!("mem <start> [len]");
                    None
                }
            }
            "." | "state" => {
                Some(Cmd::Core(DebugCmd::GetState))
            }
            mode @ ("dis16" | "dis24" | "dis" | "disassemble") => {
                let adl = match mode {
                    "dis16" => Some(false),
                    "dis24" => Some(true),
                    _ => None
                };
                let start = parse_number(tokens.next().unwrap_or(""));
                if let Ok(start) = start {
                    let end = parse_number(tokens.next().unwrap_or("")).unwrap_or(start + 0x20);
                    println!("disassemble {} {}", start, end);
                    Some(Cmd::Core(DebugCmd::Disassemble { adl, start, end }))
                } else {
                    Some(Cmd::Core(DebugCmd::DisassemblePc { adl }))
                }
            }
            "c" | "continue" => {
                Some(Cmd::Core(DebugCmd::Continue))
            }
            _ => None
        }
    } else {
        None
    }
}

fn parse_number(s: &str) -> Result<u32, ParseIntError> {
    if s.starts_with('&') || s.starts_with('$') {
        u32::from_str_radix(s.get(1..s.len()).unwrap_or(""), 16)
    }
    else if s.ends_with('h') || s.ends_with('H') {
        u32::from_str_radix(s.get(0..s.len()-1).unwrap_or(""), 16)
    } else {
        u32::from_str_radix(s, 10)
    }
}
