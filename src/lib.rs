use std::sync::mpsc::{Sender, Receiver};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use ez80::*;

mod parser;

use agon_cpu_emulator::{ DebugResp, DebugCmd };

type InDebugger = std::sync::Arc<std::sync::atomic::AtomicBool>;

fn print_help() {
    println!("While CPU is running:");
    println!("<CTRL-C>                     Pause Agon CPU and enter debugger");
    println!();
    println!("While CPU is paused:");
    println!("break <address>              Set a breakpoint at the hex address");
    println!("c[ontinue]                   Resume (un-pause) Agon CPU");
    println!("delete <address>             Delete a breakpoint");
    println!("dis[assemble] [start] [end]  Disassemble in current ADL mode");
    println!("dis16 [start] [end]          Disassemble in ADL=0 (Z80) mode");
    println!("dis24 [start] [end]          Disassemble in ADL=1 (24-bit) mode");
    println!("exit                         Quit from Agon Light Emulator");
    println!("info breakpoints             List breakpoints");
    println!("[mem]ory <start> [len]       Dump memory");
    println!("n[ext]                       Step over function calls");
    println!("state                        Show CPU state");
    println!(".                            Show CPU state");
    println!("s[tep]                       Execute one instuction");
    println!();
    println!("The previous command can be repeated by pressing return.");
}

fn do_cmd(cmd: parser::Cmd, tx: &Sender<DebugCmd>, rx: &Receiver<DebugResp>, in_debugger: &InDebugger) {
    match cmd {
        parser::Cmd::Core(debug_cmd) => {
            tx.send(debug_cmd).unwrap();
            handle_debug_resp(&rx.recv().unwrap(), in_debugger);
        }
        parser::Cmd::UiHelp => print_help(),
        parser::Cmd::UiExit => std::process::exit(0)
    }
}

fn eval_cmd(text: &str, tx: &Sender<DebugCmd>, rx: &Receiver<DebugResp>, in_debugger: &InDebugger) {
    let words = text.split_whitespace().collect::<Vec<&str>>();

    if let Some(cmd) = parser::parse_cmd(words.into_iter()) {
        do_cmd(cmd, tx, rx, in_debugger);
    } else {
        println!("Unknown or invalid command: {}", text);
    }
}

fn print_registers(reg: &ez80::Registers) {
    println!("PC:{:06x} AF:{:04x} BC:{:06x} DE:{:06x} HL:{:06x} SPS:{:04x} SPL:{:06x} IX:{:06x} IY:{:06x} MB {:02x} ADL {:01x} MADL {:01x}",
        reg.pc,
        reg.get16(Reg16::AF),
        reg.get24(Reg16::BC),
        reg.get24(Reg16::DE),
        reg.get24(Reg16::HL),
        reg.get16(Reg16::SP),
        reg.get24(Reg16::SP),
        reg.get24(Reg16::IX),
        reg.get24(Reg16::IY),
        reg.mbase,
        reg.adl as i32,
        reg.madl as i32,
    );
            /*
            //0bffe9
            println!(" [{:02x} {:02x} {:02x} {:02x}]", sys.peek(pc),
                sys.peek(pc.wrapping_add(1)),
                sys.peek(pc.wrapping_add(2)),
                sys.peek(pc.wrapping_add(3)));
                */
}

fn handle_debug_resp(resp: &DebugResp, in_debugger: &InDebugger) {
    match resp {
        DebugResp::Memory { start, data } => {
            let mut pos = *start;
            for chunk in &mut data.chunks(16) {
                print!("{:06x}: ", pos);
                for byte in chunk {
                    print!("{:02x} ", byte);
                }
                print!("| ");
                for byte in chunk {
                    let ch = if *byte >= 0x20 && byte.is_ascii() {
                        char::from_u32(*byte as u32).unwrap_or(' ')
                    } else {
                        ' '
                    };
                    print!("{}", ch);
                }
                println!();

                pos += 16;
            }
        }
        DebugResp::Message(s) => {
            println!("{}", s);
        }
        DebugResp::IsPaused(p) => {
            in_debugger.store(*p, std::sync::atomic::Ordering::SeqCst);
        }
        DebugResp::TriggerRan(msg) => {
            println!("{}", msg);
            in_debugger.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        DebugResp::Triggers(bs) => {
            println!("Triggers:");
            for b in bs {
                println!("\t&{:x}", b.address);
            }
        }
        DebugResp::Pong => {},
        DebugResp::Disassembly { adl, disasm } => {
            println!("\t.assume adl={}", if *adl {1} else {0});
            for inst in disasm {
                print!("{:06x}: {:20} |", inst.loc, inst.asm);
                for byte in &inst.bytes {
                    print!(" {:02x}", byte);
                }
                println!();
            }
        }
        DebugResp::State { registers, stack, pc_instruction, .. } => {
            print!("{:20} ", pc_instruction);
            print_registers(registers);
            if registers.adl {
                print!("{:20} SPL top ${:06x}:", "", registers.get24(Reg16::SP));
            } else {
                print!("{:20} SPS top ${:04x}:", "", registers.get16(Reg16::SP));
            }
            for byte in stack {
                print!(" {:02x}", byte);
            }
            println!();
        }
        DebugResp::Registers(registers) => {
            print_registers(registers);
        }
    }
}

fn drain_rx(rx: &Receiver<DebugResp>, in_debugger: &InDebugger) {
    loop {
        if let Ok(resp) = rx.try_recv() {
            handle_debug_resp(&resp, in_debugger);
        } else {
            break;
        }
    }
}

const PAUSE_AT_START: bool = true;

pub fn start(tx: Sender<DebugCmd>, rx: Receiver<DebugResp>) {
    let in_debugger = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(PAUSE_AT_START));
    let in_debugger_ = in_debugger.clone();
    let tx_from_ctrlc = tx.clone();

    // should be able to get this from rl.history(), but couldn't figure out the API...
    let mut last_cmd: Option<String> = None;

    println!("Agon Light Emulator Debugger");
    println!();
    print_help();
    if PAUSE_AT_START {
        println!("Interrupting execution.");
    }

    ctrlc::set_handler(move || {
        in_debugger_.store(true, std::sync::atomic::Ordering::SeqCst);
        println!("Interrupting execution.");
        tx_from_ctrlc.send(DebugCmd::Pause).unwrap();
        tx_from_ctrlc.send(DebugCmd::GetState).unwrap();
    }).expect("Error setting Ctrl-C handler");

    // `()` can be used when no completer is required
    let mut rl = DefaultEditor::new().unwrap();
    loop {
        while in_debugger.load(std::sync::atomic::Ordering::SeqCst) {
            drain_rx(&rx, &in_debugger);
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    if line != "" {
                        rl.add_history_entry(line.as_str()).unwrap();
                        eval_cmd(&line, &tx, &rx, &in_debugger);

                        if in_debugger.load(std::sync::atomic::Ordering::SeqCst) {
                            last_cmd = Some(line);
                        } else {
                            last_cmd = None;
                        }
                    } else if let Some (ref l) = last_cmd {
                        eval_cmd(l, &tx, &rx, &in_debugger);
                        //line = rl.history().last();
                    }
                },
                Err(ReadlineError::Interrupted) => {
                    break
                },
                Err(ReadlineError::Eof) => {
                    do_cmd(parser::Cmd::Core(DebugCmd::Continue), &tx, &rx, &in_debugger);
                    break
                },
                Err(err) => {
                    println!("Error: {:?}", err);
                    break
                }
            }
        }

        // when not reading debugger commands, periodically handle messages
        // from the CPU
        drain_rx(&rx, &in_debugger);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
