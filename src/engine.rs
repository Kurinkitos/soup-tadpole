
use cozy_chess::*;
use log::{debug, error};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use rayon::prelude::*;

use crate::transposition_table::{TranspositionTable, TableEntry, NodeType};

pub fn init() -> (Sender<EngineMessage>, Receiver<EngineReply>) {
    let (to_engine_send, to_engine_recv) = channel::<EngineMessage>();
    let (from_engine_send, from_engine_recv) = channel::<EngineReply>();
    thread::spawn(move || engine(to_engine_recv, from_engine_send));
    return (to_engine_send, from_engine_recv);
}

pub enum EngineMessage {
    Position(Board, Vec<Move>),
    Go,
    Stop,
    ReadyCheck,
    NewGame,
    Quit,
}

pub enum EngineReply {
    ReadyMessage,
    BestMove(Move, i32)
}


fn engine(recv: Receiver<EngineMessage>, send: Sender<EngineReply>) {
    let mut state = State::default();
    state.transposition_table = Arc::new(TranspositionTable::new());
    state.stop = AtomicBool::new(false);
    while let Ok(msg) = recv.recv() {
        match msg {
            EngineMessage::Position(board, history) => {
                state.board = board;
                debug!("Setup board state {}", state.board);
            },
            EngineMessage::Go => {
                debug!("Starting Search");
                let mut best_move: Option<Move> = None;
                let mut score = 0;
                state.transposition_table.age();
                thread::scope(|s| {
                    let handle = s.spawn(|| {
                        search(&state)
                    });
                    loop {
                        if recv.try_recv().is_ok() {
                            if let Ok(EngineMessage::Stop) = recv.recv() {
                                state.stop.store(true, std::sync::atomic::Ordering::Release);
                                break;
                            } else {
                                error!("Recived wrong message");
                                return;
                            }
                        }
                        if handle.is_finished() {
                            break;
                        }
                    }
                    let (mv, s) = handle.join().unwrap();
                    best_move = Some(mv);
                    score = s;
                });
                send.send(EngineReply::BestMove(best_move.unwrap(), score)).unwrap();
            },
            EngineMessage::Stop => todo!(),
            EngineMessage::Quit => break,
            EngineMessage::ReadyCheck => send.send(EngineReply::ReadyMessage).unwrap(),
            EngineMessage::NewGame => {state.transposition_table = Arc::new(TranspositionTable::new());}
        }
    }
}


#[derive(Default)]
struct State {
    board: Board,
    stop: AtomicBool,
    transposition_table: Arc<TranspositionTable>
}

fn search(state: &State) -> (Move, i32) {
    let mut move_list = Vec::new();
    state.board.generate_moves(|moves| {
        // Unpack dense move set into move list
        move_list.extend(moves);
        false
    });
    let mut best_move: (Move, i32) = (move_list[0], 0);
    let mut boards: Vec<(Board, Move)> = vec![(state.board.clone(), move_list[0]); move_list.len()];
    for (i, mv) in move_list.iter().enumerate() {
        boards[i].0.play_unchecked(*mv);
        boards[i].1 = *mv;
    }
    let mut alpha = -1_000_000;
    let mut beta = 1_000_000;
    for depth in 1..7 {
        if state.stop.load(std::sync::atomic::Ordering::Acquire) {
            return best_move;
        }
        match state.transposition_table.probe(&state.board, depth, alpha, beta) {
            crate::transposition_table::ProbeResult::Miss => (),
            crate::transposition_table::ProbeResult::OrderingHint(mv) => put_move_board_first(mv, &mut boards),
            crate::transposition_table::ProbeResult::SearchResult(mv, s) => {
                best_move = (mv, s);
                continue;
            },
        }

        let mut evaluations: Vec<(Move, Option<i32>)> = Vec::with_capacity(boards.len());

        boards.par_iter().map(|(board, mv)| (mv.clone(), alpha_beta(alpha, beta, 0, depth, board.clone(), state.transposition_table.clone(), &state.stop))).collect_into_vec(&mut evaluations);
        

        evaluations.sort_unstable_by(|(_, e1), (_, e2)| e1.partial_cmp(e2).unwrap());

        debug!("{:?}", evaluations);

        if evaluations.first().unwrap().1.is_none() {
            return best_move;
        }
        best_move = (evaluations.first().unwrap().0, evaluations.first().unwrap().1.unwrap());
        let entry = TableEntry {
            best_response: evaluations.first().unwrap().0,
            depth,
            score: evaluations.first().unwrap().1.unwrap(),
            node: NodeType::PV,
            age: 0,
        };
        state.transposition_table.insert(state.board.clone(), entry);
        // Update the aspiration window
        alpha = evaluations.first().unwrap().1.unwrap() - 100;
        beta = evaluations.first().unwrap().1.unwrap() + 100;
        
    }
    return best_move;
}

fn alpha_beta(initial_alpha: i32, beta: i32, depth: u32, max_depth: u32, board: Board, table: Arc<TranspositionTable>, stop: &AtomicBool) -> Option<i32> {
    if stop.load(std::sync::atomic::Ordering::Acquire) {

    }
    let mut alpha = initial_alpha;

    if depth >= max_depth {
        let eval = evaluate(&board);
        return Some(eval);
    }

    let mut move_list = Vec::new();
    board.generate_moves(|moves| {
        // Unpack dense move set into move list
        move_list.extend(moves);
        false
    });
    match table.probe(&board, depth, alpha, beta) {
        crate::transposition_table::ProbeResult::Miss => (),
        crate::transposition_table::ProbeResult::OrderingHint(mv) => put_move_first(mv, &mut move_list),
        crate::transposition_table::ProbeResult::SearchResult(_, s) => {
            return Some(s)
        },
    }

    for mv in move_list {
        let mut local_board = board.clone();
        local_board.play_unchecked(mv);
        let score = -alpha_beta(-beta, -alpha, depth +1, max_depth, local_board, table.clone(), stop)?;
        if score >= beta {
            let entry = TableEntry {
                best_response: mv,
                depth: max_depth,
                score,
                node: NodeType::Cut,
                age: 0,
            };
            table.insert(board, entry);
            return Some(beta);
        }
        if score > alpha {
            let entry = TableEntry {
                best_response: mv,
                depth: max_depth,
                score,
                node: NodeType::All,
                age: 0,
            };
            table.insert(board.clone(), entry);
            alpha = score;
        }
    }
    return Some(alpha);
}

fn put_move_board_first(mv: Move, boards: &mut Vec<(Board, Move)>) {
    let target_pos = boards.iter().position(|m| mv == m.1).unwrap();
    boards.swap(target_pos, 0);
}

fn put_move_first(mv: Move, moves: &mut Vec<Move>) {
    let target_pos = moves.iter().position(|m| mv == *m).unwrap();
    moves.swap(target_pos, 0);
}

// Evaluate the board from player that just made a move's perspective
fn evaluate(board: &Board) -> i32 {
    use cozy_chess::Color::*;
    // Handle if the game is over
    match board.status() {
        GameStatus::Won => {
            match board.side_to_move() {
                White => return -20_000,
                Black => return 20_000,
            }
        },
        GameStatus::Drawn => return 0,
        GameStatus::Ongoing => (),
    }

    let material_diff= count_material(board, White) - count_material(board, Black);

    let mobility_diff = calculate_mobility(board, White) - calculate_mobility(board, Black);

    let eval = material_diff + (mobility_diff * 10);

    if board.side_to_move() == Black {
        return eval;
    } else {
        return -eval;
    }
}

// Material value for the given player, in centipawns
fn count_material(board: &Board, player: Color) -> i32 {
    let color = board.colors(player);
    let mut score = 0;

    score += (board.pieces(Piece::Pawn) & color).len() * 100;
    score += (board.pieces(Piece::Knight) & color).len() * 320;
    score += (board.pieces(Piece::Bishop) & color).len() * 330;
    score += (board.pieces(Piece::Rook) & color).len() * 500;
    score += (board.pieces(Piece::Queen) & color).len() * 900;

    return score as i32;
}

// calculate mobility, currently only number of legal moves
fn calculate_mobility(board: &Board, player: Color) -> i32 {
    if player == board.side_to_move() {
        count_moves(board)
    } else {
        let local_board = board.clone();
        match local_board.null_move() {
            Some(b) => count_moves(&b),
            None => 0,
        }
    }

}

fn count_moves(board: &Board) -> i32 {
    let mut c = 0;
    board.generate_moves(|moves| {
        // Unpack dense move set into move list
        c += moves.len();
        false
    });
    c as i32
}

#[cfg(test)]
mod benches {
    use super::*;
    extern crate test;
    use test::Bencher;

    #[test]
    fn test_search() {
        let mut state = State::default();
        state.transposition_table = Arc::new(TranspositionTable::new());

        search(&state);
    }

    #[bench]
    fn bench_search_startpos(b: &mut Bencher) {
        let mut state = State::default();
        b.iter(|| {
            state.transposition_table = Arc::new(TranspositionTable::new());
            search(&state);
    })
    }
}