use crate::debugger_command::DebuggerCommand;
use crate::dwarf_data::{DwarfData, Error as DwarfError};
use crate::inferior::{Inferior, Status};
use nix::sys::ptrace;
use nix::sys::wait::waitpid;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::collections::HashMap;

#[derive(Clone)]
struct Breakpoint {
    id: i64,
    addr: usize,
    orig_byte: u8,
}

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    debug_data: Option<DwarfData>,
    inferior: Option<Inferior>,
    breakpoints_list: Vec<(i64, usize)>,
    breakpoints_map: HashMap<usize, Breakpoint>,
    breakpoint_count: i64,
    current_result: Result<Status, nix::Error>,
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        // TODO (milestone 3): initialize the DwarfData
        let debug_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(DwarfError::ErrorOpeningFile) => {
                println!("Could not open file {}", target);
                std::process::exit(1);
            }
            Err(DwarfError::DwarfFormatError(err)) => {
                println!("Could not debugging symbols from {}: {:?}", target, err);
                std::process::exit(1);
            }
        };

        debug_data.print();

        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            debug_data: Some(debug_data),
            inferior: None,
            breakpoints_list: Vec::new(),
            breakpoints_map: HashMap::new(),
            breakpoint_count: 0,
            current_result: Ok(Status::Exited(0)),
        }
    }

    fn parse_address(addr: &str) -> Option<usize> {
        let addr_without_0x = if addr.to_lowercase().starts_with("0x") {
            &addr[2..]
        } else {
            &addr
        };
        usize::from_str_radix(addr_without_0x, 16).ok()
    }

    fn deal_status(&self, result: &Result<Status, nix::Error>) {
        match result {
            Ok(status) => match status {
                crate::inferior::Status::Stopped(_, signal, mut rip) => {
                    println!("Child stopped (signal {})", signal);
                    if let Some(data) = self.debug_data.as_ref() {
                        let func_name = data.get_function_from_addr(rip).expect("invalid addr");
                        let func_line = data.get_line_from_addr(rip).expect("invalid addr");
                        println!("Stopped at {} ({})", func_name, func_line);
                    } else {
                        eprintln!("invalid debug data!");
                    }
                }
                crate::inferior::Status::Exited(_) => {
                    println!("Child exited (status 0)");
                }
                crate::inferior::Status::Signaled(signal) => {
                    println!("Child Signaled (signal {})", signal);
                }
            },
            Err(err) => {
                eprintln!("{}", err);
            }
        }
    }

    fn set_breakpoint(&mut self, point_id: i64, addr: usize) -> Option<Breakpoint> {
        let orig_byte = self
            .inferior
            .as_mut()
            .unwrap()
            .write_byte(addr, 0xcc)
            .ok()?;
        Some(Breakpoint {
            id: point_id,
            addr: addr,
            orig_byte: orig_byte,
        })
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => {
                    if let Some(mut inferior) = self.inferior.take() {
                        let _ = inferior.kill();
                    }

                    if let Some(inferior) = Inferior::new(&self.target, &args) {
                        // Create the inferior
                        self.inferior = Some(inferior);
                        // TODO (milestone 1): make the inferior run
                        // You may use self.inferior.as_mut().unwrap() to get a mutable reference
                        // to the Inferior object
                        for idx in 0..self.breakpoints_list.len() {
                            let (point_id, addr) = self.breakpoints_list[idx];
                            let breakpoint = self
                                .set_breakpoint(point_id, addr)
                                .expect("set breakpoint failed!");
                            self.breakpoints_map.insert(addr, breakpoint);
                        }

                        self.current_result = self.inferior.as_mut().unwrap().continue_run(None);
                        self.deal_status(&self.current_result);
                    } else {
                        println!("Error starting subprocess");
                    }
                }

                DebuggerCommand::Continue => {
                    if self.inferior.is_none() {
                        eprintln!("Error no subprocess is running!");
                    }
                    if self.current_result.is_ok() {
                        let status = self
                            .current_result
                            .as_ref()
                            .ok()
                            .expect("get current result failed!");
                        if let Status::Stopped(pid, _signal, rip) = status {
                            let stopped_rip = rip - 1;
                            let breakpoint = &self.breakpoints_map[&stopped_rip];
                            // restore old value
                            let _ = self
                                .inferior
                                .as_mut()
                                .unwrap()
                                .write_byte(stopped_rip, breakpoint.orig_byte);

                            let _ = ptrace::step(*pid, None);
                            let _ = waitpid(*pid, None);

                            let _ = self
                                .inferior
                                .as_mut()
                                .unwrap()
                                .write_byte(stopped_rip, 0xcc);
                        }
                    }
                    self.current_result = self.inferior.as_mut().unwrap().continue_run(None);
                    self.deal_status(&self.current_result);
                }

                DebuggerCommand::Backtrace => {
                    let _ = self
                        .inferior
                        .as_mut()
                        .unwrap()
                        .print_backtrace(&self.debug_data);
                }

                DebuggerCommand::BreakPoint(point_addr) => {
                    let mut addr: usize = 0;
                    if point_addr.to_lowercase().starts_with("0x") {
                        addr = Self::parse_address(&point_addr).expect("invalied address");
                        println!("Set breakpoint {} at {}", self.breakpoint_count, point_addr);
                    } else if point_addr.chars().all(|char| char.is_ascii_digit()) {
                        let line_number = point_addr
                            .parse::<usize>()
                            .expect("failed to parse addr to line number");
                        addr = self
                            .debug_data
                            .as_ref()
                            .unwrap()
                            .get_addr_for_line(None, line_number)
                            .expect("failed to get addr for line");

                        println!("Set breakpoint {} at {:x}", self.breakpoint_count, addr);
                    } else {
                        addr = self
                            .debug_data
                            .as_ref()
                            .unwrap()
                            .get_addr_for_function(None, &point_addr)
                            .expect("faile to get addr for cuntion");

                        println!("Set breakpoint {} at {:x}", self.breakpoint_count, addr);
                    }

                    self.breakpoints_list.push((self.breakpoint_count, addr));
                    if self.inferior.is_some() {
                        let breakpoint = self
                            .set_breakpoint(self.breakpoint_count, addr)
                            .expect("set_breakpoint failed!");
                        self.breakpoints_map.insert(addr, breakpoint);
                    }
                    self.breakpoint_count += 1;
                }

                DebuggerCommand::Quit => {
                    if let Some(mut inferior) = self.inferior.take() {
                        let _ = inferior.kill();
                    }
                    return;
                }
            }
        }
    }

    /// This function prompts the user to enter a command, and continues re-prompting until the user
    /// enters a valid command. It uses DebuggerCommand::from_tokens to do the command parsing.
    ///
    /// You don't need to read, understand, or modify this function.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("(deet) ") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type \"quit\" to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressed ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().len() == 0 {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!(
                            "Warning: failed to save history file at {}: {}",
                            self.history_path, err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }
}
