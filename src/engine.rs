
use cozy_chess::*;
use log::debug;
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use dashmap::DashMap;
use rayon::prelude::*;

pub fn init() -> (Sender<EngineMessage>, Receiver<EngineReply>) {
    let (to_engine_send, to_engine_recv) = channel::<EngineMessage>();
    let (from_engine_send, from_engine_recv) = channel::<EngineReply>();
    thread::spawn(move || engine(to_engine_recv, from_engine_send));
    return (to_engine_send, from_engine_recv);
}

pub enum EngineMessage {
    Position(Board),
    Go,
    Stop,
    ReadyCheck,
    Quit,
}

pub enum EngineReply {
    ReadyMessage,
    BestMove(Move)
}


fn engine(recv: Receiver<EngineMessage>, send: Sender<EngineReply>) {
    let mut state = State::default();
    state.transposition_table = Arc::new(DashMap::new());
    while let Ok(msg) = recv.recv() {
        match msg {
            EngineMessage::Position(board) => {
                state.board = board;
                debug!("Setup board state {}", state.board);
            },
            EngineMessage::Go => {
                debug!("Starting Search");
                let best_move = search(&state);
                send.send(EngineReply::BestMove(best_move)).unwrap();
                state.transposition_table = Arc::new(DashMap::new());
            },
            EngineMessage::Stop => todo!(),
            EngineMessage::Quit => break,
            EngineMessage::ReadyCheck => send.send(EngineReply::ReadyMessage).unwrap(),
        }
    }
}


#[derive(Default)]
struct State {
    board: Board,
    transposition_table: Arc<DashMap<Board, i64>>
}

fn search(state: &State) -> Move {
    let mut move_list = Vec::new();
    state.board.generate_moves(|moves| {
        // Unpack dense move set into move list
        move_list.extend(moves);
        false
    });
    let mut boards: Vec<Board> = vec![state.board.clone(); move_list.len()];
    for (i, mv) in move_list.iter().enumerate() {
        boards[i].play_unchecked(*mv);
    }

    let mv_iter = move_list.par_iter();
    let eval_iter = boards.par_iter().map(|b| alpha_beta(-1000, 1000, 5, b.clone(), state.transposition_table.clone()));
    let mut evaluations : Vec<(&Move, i64)> = mv_iter.zip(eval_iter).collect();

    evaluations.sort_unstable_by(|(_, e1), (_, e2)| e1.partial_cmp(e2).unwrap());

    debug!("{:?}", evaluations);

    *evaluations.first().unwrap().0
}

fn alpha_beta(a: i64, beta: i64, depth: u32, board: Board, table: Arc<DashMap<Board, i64>>) -> i64 {
    let mut alpha = a;

    match table.get(&board) {
        Some(a) => {
            return *a.value()
        },
        None => (),
    }

    if depth == 0 {
        let eval = evaluate(&board);
        table.insert(board, eval);
        return eval;
    }

    let mut move_list = Vec::new();
    board.generate_moves(|moves| {
        // Unpack dense move set into move list
        move_list.extend(moves);
        false
    });

    for mv in move_list {
        let mut local_board = board.clone();
        local_board.play_unchecked(mv);
        let score = -alpha_beta(-beta, -alpha, depth -1, local_board, table.clone());
        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }
    return alpha;
}




// Evaluate the board from player that just made a move's perspective
fn evaluate(board: &Board) -> i64 {
    let player = match board.side_to_move() {
        Color::White => Color::Black,
        Color::Black => Color::White,
    };
    // Handle if the game is over
    match board.status() {
        GameStatus::Won => return 20000,
        GameStatus::Drawn => return 0,
        GameStatus::Ongoing => (),
    }

    let material_diff= count_material(board, &board.side_to_move()) - count_material(board, &player);

    return material_diff ;
}

fn count_material(board: &Board, player: &Color) -> i64 {
    let color = board.colors(*player);
    let mut score = 0;

    score += (board.pieces(Piece::Pawn) & color).len();
    score += (board.pieces(Piece::Knight) & color).len() * 3;
    score += (board.pieces(Piece::Bishop) & color).len() * 3;
    score += (board.pieces(Piece::Rook) & color).len() * 5;
    score += (board.pieces(Piece::Queen) & color).len() * 8;

    return score as i64;
}

#[cfg(test)]
mod benches {
    use super::*;
    extern crate test;
    use test::Bencher;

    #[bench]
    fn bench_search(b: &mut Bencher) {
        let mut state = State::default();
        state.transposition_table = Arc::new(DashMap::new());
        b.iter(|| search(&state))
    }
}