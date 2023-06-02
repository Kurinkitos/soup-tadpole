use cozy_chess;
use std::{io::{self, BufRead}, process::exit};
use vampirc_uci::{UciMessage, parse_one, UciOptionConfig};
use log::{LevelFilter, info};

fn main() {
    let _res = simple_logging::log_to_file("test.log", LevelFilter::Info);
    for line in io::stdin().lock().lines() {
        let msg: UciMessage = parse_one(&line.unwrap());
        info!("Received message: {}", msg);
        match msg {
            UciMessage::Uci => { 
                let id_response = UciMessage::id_name("soup-tadpole");
                send(id_response);
                let uci_ok = UciMessage::UciOk;
                send(uci_ok);
            },
            UciMessage::IsReady => {
                let ready_ok = UciMessage::ReadyOk; 
                send(ready_ok);
            },
            UciMessage::Stop => {
                info!("Stopping");
            }
            UciMessage::Quit => {
                info!("Shutting down");
                exit(0);
            }
            msg => {
                log::error!("Unhandled message uci : {}", msg);
                let err_msg = UciMessage::info_string(format!("Message {} not handled", msg));
                send(err_msg);
            }
        }
   }
}

fn init() {
    info!("init() started");
    info!("init() finished");
}

fn send(msg: UciMessage) {
    println!("{}", msg);
    info!("Sent message: {}", msg);
}

struct EngineState {
    board: cozy_chess::Board,
}