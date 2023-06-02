use cozy_chess;

fn main() {
    let board = cozy_chess::Board::default();
    let mut move_list = Vec::new();
    board.generate_moves(|moves| {
        // Unpack dense move set into move list
        move_list.extend(moves);
        false
    });
    for mv in move_list {
        let mut new = board.clone();
        new.play_unchecked(mv);
        println!("{}", new);
    }
}
