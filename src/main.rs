#![feature(test)]

use cozy_chess::{Board, Rank, File};
use engine::{EngineMessage, EngineReply};
use std::{io::{self, BufRead}, process::exit, sync::mpsc::{Receiver, Sender}};
use vampirc_uci::{UciMessage, parse_one, UciFen, UciMove, UciSquare, UciPiece, UciInfoAttribute};
use log::{LevelFilter, info, error, debug};


mod engine;
mod transposition_table;

fn main() {
    let _res = simple_logging::log_to_file("test.log", LevelFilter::Debug);
    let (sender, reciver) = init();
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
                sender.send(EngineMessage::ReadyCheck).unwrap();
                reciver.recv().unwrap();
                let ready_ok = UciMessage::ReadyOk; 
                send(ready_ok);
            },
            UciMessage::Stop => {
                sender.send(EngineMessage::Stop).unwrap();
                debug!("Sending nonsense move for now");
                let best_move = UciMessage::BestMove { best_move: UciMove::from_to(UciSquare::from('e', 7) , UciSquare::from('e', 6)), ponder: None };
                info!("Stopping");
                send(best_move);
            }
            UciMessage::Quit => {
                info!("Shutting down");
                exit(0);
            },
            UciMessage::UciNewGame => {
                sender.send(EngineMessage::NewGame).unwrap();
            },
            UciMessage::Position { startpos, fen, moves } => {
                if startpos {
                    info!("Setting board to starting position");
                    let mut board = Board::default();
                    let mut history: Vec<cozy_chess::Move> = Vec::new();
                    for mv in moves {
                        let mv_str = format!("{}", mv);
                        let mv: cozy_chess::Move = mv_str.parse().unwrap();
                        board.play_unchecked(mv);
                        history.push(mv);
                    } 
                    sender.send(EngineMessage::Position(board, history)).unwrap();
                } else {
                    if let Some(UciFen(fen_str)) = fen {
                        let mut board = fen_str.parse::<Board>().unwrap();
                        let mut history: Vec<cozy_chess::Move> = Vec::new();
                        for mv in moves {
                            let mv_str = format!("{}", mv);
                            let mv: cozy_chess::Move = mv_str.parse().unwrap();
                            board.play_unchecked(mv);
                            history.push(mv);
                        } 
                        sender.send(EngineMessage::Position(board, history)).unwrap();
                    } else {
                        error!("Invalide position message recived!");
                        exit(1);
                    }
                }
            },
            UciMessage::Go { time_control, search_control} => {
                sender.send(EngineMessage::Go).unwrap();
                if let EngineReply::BestMove(mv, score) = reciver.recv().unwrap() {
                    let score_msg = UciMessage::Info(vec!(UciInfoAttribute::Score { cp: Some(score), mate: None, lower_bound: None, upper_bound: None }));
                    send(score_msg);
                    let m = move_to_ucimove(mv);
                    let best_move_msg = UciMessage::BestMove { best_move: m, ponder: None };
                    send(best_move_msg);
                } else {
                    error!("Incorrect reply from engine to Go command");
                    exit(1);
                }
            }
            msg => {
                log::error!("Unhandled message uci : {}", msg);
                let err_msg = UciMessage::info_string(format!("Message {} not handled", msg));
                send(err_msg);
            }
        }
   }
}

fn init() -> (Sender<EngineMessage>, Receiver<EngineReply>){
    info!("init() started");
    let comms = engine::init();
    info!("init() finished");
    return comms;
}

fn send(msg: UciMessage) {
    println!("{}", msg);
    info!("Sent message: {}", msg);
}

fn move_to_ucimove(mv: cozy_chess::Move) -> UciMove{
    let from_square = UciSquare::from(file_to_ucifile(mv.from.file()), rank_to_ucirank(mv.from.rank()));
    let to_square = UciSquare::from(file_to_ucifile(mv.to.file()), rank_to_ucirank(mv.to.rank()));
    let promotion = mv.promotion.map(| p | piece_to_ucipiece(p));
    UciMove { from:from_square, to: to_square, promotion: promotion }
}
fn rank_to_ucirank(rank: Rank) -> u8 {
    match rank {
        Rank::First => 1,
        Rank::Second => 2,
        Rank::Third => 3,
        Rank::Fourth => 4,
        Rank::Fifth => 5,
        Rank::Sixth => 6,
        Rank::Seventh => 7,
        Rank::Eighth => 8,
    }
}

fn file_to_ucifile(file: File) -> char {
    match file {
        File::A => 'a',
        File::B => 'b',
        File::C => 'c',
        File::D => 'd',
        File::E => 'e',
        File::F => 'f',
        File::G => 'g',
        File::H => 'h'
    }
}
fn piece_to_ucipiece(piece: cozy_chess::Piece) -> UciPiece {
    match piece {
        cozy_chess::Piece::Pawn => UciPiece::Pawn,
        cozy_chess::Piece::Knight => UciPiece::Knight,
        cozy_chess::Piece::Bishop => UciPiece::Bishop,
        cozy_chess::Piece::Rook => UciPiece::Rook,
        cozy_chess::Piece::Queen => UciPiece::Queen,
        cozy_chess::Piece::King => UciPiece::King
    }
}