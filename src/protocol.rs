use rsoderh_chess::{Board, Color, Piece, PieceKind, Position, Slot};

const BOARD_LEN: usize = 8;
const BOARD_SIZE: usize = 64;

#[derive(PartialEq, Debug)]
pub enum ParseError {
    TooLong,
    UnknownMessageType,
    WrongAmountOfFields,

    InvalidMoveFormat,
    InvalidGameState,

    InvalidFENChar,
    InvalidFENLength,
}

#[derive(PartialEq, Debug)]
pub enum SerializeError {
    InvalidPromPiece,
    TooLongQuitMsg,
}

#[derive(PartialEq, Debug)]
pub enum GameState {
    Ongoing,
    WinWhite,
    Draw,
    WinBlack,
}

#[derive(PartialEq, Debug)]
pub struct MessageMove {
    pub board: Board,
    pub mv: (Position, Position),
    pub prom_piece: Option<PieceKind>,
    pub game_state: GameState,
}

#[derive(PartialEq, Debug)]
pub enum Message {
    Quit(String),
    Move(MessageMove),
}

pub fn parse(message: &str) -> Result<Message, ParseError> {
    if message.len() > 128 {
        return Err(ParseError::TooLong);
    }

    let mut message = message.split(":");
    let msg_id= message.next();
    let msg_id = match msg_id {
        Some(id) => id,
        None => return Err(ParseError::UnknownMessageType),
    };

    let message = message.collect::<Vec<_>>();
    let message = message.as_slice();
    
    match msg_id {
        "ChessMOVE" => Ok(Message::Move(parse_message_move(message)?)),
        "ChessQUIT" => Ok(Message::Quit(parse_message_quit(message)?)),
        _ => return Err(ParseError::UnknownMessageType),
    }
}

pub fn serialize(message: &Message) -> Result<String, SerializeError> {
    match message {
        Message::Move(message) => serialize_move(&message),
        Message::Quit(str) => serialize_quit(str),
    }
}

fn serialize_move(message: &MessageMove) -> Result<String, SerializeError> {
    fn fen_encode_pos(pos: &Position) -> (char, char) {
        // Should never fail
        let file = char::from_digit(pos.column.get() as u32 + 10, 18).unwrap();
        let rank = char::from_digit(pos.row.get() as u32 + 1, 10).unwrap();
        return (rank.to_ascii_uppercase(), file.to_ascii_uppercase())
    }

    fn serialize_mv(message: &MessageMove) -> Result<String, SerializeError> {
        // Serialize mv
        let (pos_src, pos_dst) = message.mv;
        let (rank_src, file_src) = fen_encode_pos(&pos_src);
        let (rank_dst, file_dst) = fen_encode_pos(&pos_dst);
        let prom = match message.prom_piece {
            Some(PieceKind::Knight) => 'N',
            Some(PieceKind::Bishop) => 'B',
            Some(PieceKind::Rook)   => 'R',
            Some(PieceKind::Queen)  => 'Q',
            None                    => '0',
            _ => return Err(SerializeError::InvalidPromPiece),
        };
        let serialized_mv: String = [file_src, rank_src, file_dst, rank_dst, prom].into_iter().collect();
        Ok(serialized_mv)
    }

    fn serialize_game_state(message: &MessageMove) -> &str {
        match message.game_state {
            GameState::Ongoing  => "0-0",
            GameState::WinWhite => "1-0",
            GameState::Draw     => "1-1",
            GameState::WinBlack => "0-1",
        }
    }

    fn serialize_board(board: &Board) -> String {
        fn serialize_piece(piece: Piece) -> char {
            let serialized_piece_kind = match piece.kind {
                PieceKind::Pawn     => 'P',
                PieceKind::Knight   => 'N',
                PieceKind::Bishop   => 'B',
                PieceKind::Rook     => 'R',
                PieceKind::Queen    => 'Q',
                PieceKind::King     => 'K',
            };

            if piece.color == Color::White { 
                serialized_piece_kind
            } else { 
                serialized_piece_kind.to_ascii_lowercase()
            }
        }

        (0..BOARD_LEN)
            .map(|rank| {
            let mut fen_rank: String = "".to_string();
            let mut empty_count = 0;
            for file in 0..BOARD_LEN {
                // TODO: fix the ordering of this!!!
                // Should not fail
                let pos = Position::new(file as u8, rank as u8).unwrap();
                match board.at_position(pos) {
                    Slot::Occupied(piece) => {
                        if empty_count > 0 {
                            let chr = std::char::from_digit(empty_count, 10).unwrap();
                            fen_rank.push(chr);
                        }
                        let piece_fen = serialize_piece(piece);
                        fen_rank.push(piece_fen);
                        empty_count = 0;
                    },
                    Slot::Empty => {
                        empty_count += 1;
                        continue;
                    },
                }
            }
            if empty_count > 0 {
                let chr = std::char::from_digit(empty_count, 10).unwrap();
                fen_rank.push(chr);
            }
            fen_rank
        })
        .collect::<Vec<_>>()
        .join("/")
    }

    let serialized_msg_id  = "ChessMOVE";
    let serialized_mv = serialize_mv(message)?;
    let serialized_game_state = serialize_game_state(message);
    let serialized_board= serialize_board(&message.board);
    
    let mut serialized= [serialized_msg_id, &serialized_mv, serialized_game_state, &serialized_board].join(":");
    serialized += ":";
    serialized += &"0".repeat(128 - serialized.len());

    Ok(serialized)

}

fn serialize_quit(str: &str) -> Result<String, SerializeError> {
    let mut serialized = "ChessQUIT:".to_string() + str + ":";
    if serialized.len() > 128{
        return Err(SerializeError::TooLongQuitMsg);
    }
    serialized += &"0".repeat(128 - serialized.len());
    Ok(serialized)
}

fn parse_message_move(message: &[&str]) -> Result<MessageMove, ParseError> {
    match *message {
        [mv, game_state, board, _padding] => {
            if mv.len() != 5 {
                return Err(ParseError::InvalidMoveFormat);
            }

            let prom_piece = match &mv[4..5] {
                "0" => None,
                "N" | "n" => Some(PieceKind::Knight),
                "B" | "b" => Some(PieceKind::Bishop),
                "R" | "r" => Some(PieceKind::Rook),
                "Q" | "q" => Some(PieceKind::Queen),
                _ => return Err(ParseError::InvalidMoveFormat),
            };
            let mv = {
                let mv_src = &mv[0..2];
                let mv_dst = &mv[2..4];
                (Position::parse(mv_src).ok_or(ParseError::InvalidMoveFormat)?, 
                 Position::parse(mv_dst).ok_or(ParseError::InvalidMoveFormat)?)
            };

            let game_state = match game_state {
                "0-0" => GameState::Ongoing,
                "1-0" => GameState::WinWhite,
                "0-1" => GameState::WinBlack,
                "1-1" => GameState::Draw,
                _ => return Err(ParseError::InvalidGameState),
            };
            
            let board = parse_fen(board)?;

            Ok(MessageMove {
                board,
                game_state,
                mv,
                prom_piece,
            })
        },
        _ => return Err(ParseError::WrongAmountOfFields),
    }
}

fn parse_message_quit(message: &[&str]) -> Result<String, ParseError> {
    match *message {
        [op_msg, _padding] => Ok(op_msg.to_string()),
        _ => Err(ParseError::WrongAmountOfFields),
    }
}

fn parse_fen(fen: &str) -> Result<Board, ParseError> {
    let mut board = Board::new_empty();
    
    let mut index: usize = BOARD_SIZE;
    for rank in fen.split("/") {
        for chr in rank.chars().rev() {
            if let Some(skips) = chr.to_digit(10) {
                index -= skips as usize;
                continue;
            }
            index -= 1;

            // Index wraps around to usize::MAX when "below zero"
            if index >= BOARD_SIZE {
                // FEN string too long
                return Err(ParseError::InvalidFENLength);
            }

            let piece_kind = match chr.to_ascii_uppercase() {
                'P' => PieceKind::Pawn,
                'N' => PieceKind::Knight,
                'B' => PieceKind::Bishop,
                'R' => PieceKind::Rook,
                'Q' => PieceKind::Queen,
                'K' => PieceKind::King,
                _ => return Err(ParseError::InvalidFENChar),
            };

            let piece_color = match chr.is_uppercase() {
                true => Color::White,
                false => Color::Black,
            };

            let rank = index / BOARD_LEN;
            let file = BOARD_LEN - (index % BOARD_LEN) - 1;

            assert!(rank < BOARD_LEN);
            assert!(file < BOARD_LEN);

            let position = Position::new(file as u8, rank as u8);
            let position = match position {
                Some(p) => p,
                None => unreachable!(), // Something is wrong with the underlying chess library
            };

            let slot = board.at_position_mut(position);
            *slot = Slot::Occupied(
                Piece {
                    color: piece_color,
                    kind: piece_kind,
                }
            );
        }
    }
    if index != 0 {
        // FEN string too short
        return Err(ParseError::InvalidFENLength)
    }

    Ok(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_all_zeros(s: &str) -> bool {
        s.chars().all(|c| c == '0')
    }

    // Taken from the spec: https://github.com/INDA25PlusPlus/chesstp-spec
    fn move_padding_len(board_len: usize) -> usize {
        128 - 9 - 1 - 5 - 1 - 3 - 1 - board_len - 1
    }

    #[test]
    fn serialize_move_e2e4() {
        let board = Board::new_empty();

        let src = Position::new(4, 1).expect("pos e2");
        let dst = Position::new(4, 3).expect("pos e4");

        let msg = Message::Move(MessageMove {
            board,
            mv: (src, dst),
            prom_piece: None,
            game_state: GameState::Ongoing,
        });

        let s = serialize(&msg).expect("serialize move");
        assert_eq!(s.len(), 128, "MOVE must be 128 bytes");

        let parts: Vec<&str> = s.split(':').collect();
        assert_eq!(parts.len(), 5, "Expected five parts");

        assert_eq!(parts[0], "ChessMOVE");
        assert_eq!(parts[1], "E2E40", "files must be CAPITAL letters");
        assert_eq!(parts[2], "0-0");
        assert_eq!(parts[3], "8/8/8/8/8/8/8/8");
        assert!(is_all_zeros(parts[4]), "padding must be only '0's");

        let pad_len = parts[4].len();
        assert_eq!(
            pad_len,
            move_padding_len(parts[3].len()),
            "padding length must follow the spec formula"
        );

        // Try parse the serialized board
        let parsed = parse(&s).expect("parse serialized move");
        match parsed {
            Message::Move(mm) => {
                assert_eq!(mm.game_state, GameState::Ongoing);
                assert_eq!(mm.prom_piece, None);
                assert_eq!(mm.mv.0, src);
                assert_eq!(mm.mv.1, dst);
            }
            _ => panic!("expected Message::Move"),
        }
    }

    #[test]
    fn serialize_move_with_promotion_and_winwhite() {
        // A7 -> A8 with promotion to Queen
        let board = Board::new_empty();
        let src = Position::new(0, 6).unwrap(); // A7
        let dst = Position::new(0, 7).unwrap(); // A8

        let msg = Message::Move(MessageMove {
            board,
            mv: (src, dst),
            prom_piece: Some(PieceKind::Queen),
            game_state: GameState::WinWhite,
        });

        let s = serialize(&msg).expect("serialize move");
        assert_eq!(s.len(), 128);

        let parts: Vec<&str> = s.split(':').collect();
        assert_eq!(parts[0], "ChessMOVE");
        assert_eq!(parts[1], "A7A8Q", "promotion letter allowed, case-insensitive; file letters CAPITAL");
        assert_eq!(parts[2], "1-0");
        assert!(is_all_zeros(parts[4]));
    }

    #[test]
    fn serialize_move_rejects_invalid_promotion_piece() {
        // Using King as "promotion" target must be rejected by serializer
        let board = Board::new_empty();
        let src = Position::new(0, 6).unwrap(); // A7
        let dst = Position::new(0, 7).unwrap(); // A8

        let msg = Message::Move(MessageMove {
            board,
            mv: (src, dst),
            prom_piece: Some(PieceKind::King), // invalid promotion piece
            game_state: GameState::Ongoing,
        });

        let err = serialize(&msg).expect_err("invalid promotion piece must error");
        assert!(matches!(err, SerializeError::InvalidPromPiece));
    }

    #[test]
    fn serialize_quit_empty_message() {
        let s = serialize_quit("").expect("serialize quit");
        assert_eq!(s.len(), 128);

        let parts: Vec<&str> = s.split(':').collect();
        assert_eq!(parts.len(), 3, "id:message:padding expected");
        assert_eq!(parts[0], "ChessQUIT");
        assert_eq!(parts[1], "", "empty optional message is allowed by spec");
        assert!(is_all_zeros(parts[2]));

        // Try parse the serialized board
        let parsed = parse(&s).expect("parse quit");
        match parsed {
            Message::Quit(m) => assert_eq!(m, ""),
            _ => panic!("expected Message::Quit"),
        }
    }

    #[test]
    fn serialize_quit_with_message() {
        let msg = "I had a panic attack";
        let s = serialize_quit(msg).expect("serialize quit with text");
        assert_eq!(s.len(), 128);

        let parts: Vec<&str> = s.split(':').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "ChessQUIT");
        assert_eq!(parts[1], msg);
        assert!(is_all_zeros(parts[2]));

        // Try parse the serialized board
        let parsed = parse(&s).expect("parse quit");
        match parsed {
            Message::Quit(m) => assert_eq!(m, msg),
            _ => panic!("expected Message::Quit"),
        }
    }

    #[test]
    fn serialize_quit_rejects_too_long() {
        let too_long = "X".repeat(118);
        let err = serialize_quit(&too_long).expect_err("must reject >117 bytes");
        assert!(matches!(err, SerializeError::TooLongQuitMsg));
    }

    #[test]
    fn parse_quit_missing_padding_is_error() {
        // Missing trailing ':' + padding; total length < 128
        let bad = "ChessQUIT:Bye";
        let res = parse(bad);
        assert_eq!(res, Err(ParseError::WrongAmountOfFields));
    }

    #[test]
    fn parse_quit_with_colon_in_optional_message_is_error() {
        // Spec says: optional message must not contain ':'
        let bad = "ChessQUIT:hello:world:0";
        let res = parse(bad);
        assert_eq!(res, Err(ParseError::WrongAmountOfFields));
    }

    #[test]
    fn parse_valid_move_no_promotion() {
        let fen = "8/8/8/8/8/8/8/8";
        let msg = format!("ChessMOVE:a2a40:0-0:{}:x", fen);

        let result = parse(&msg);
        assert!(matches!(result, Ok(Message::Move(_))));
    }

    #[test]
    fn parse_valid_move_with_promotion() {
        let fen = "8/8/8/8/8/8/8/8";
        let msg = format!("ChessMOVE:a7a8Q:1-0:{}:x", fen);

        let result = parse(&msg);
        match result {
            Ok(Message::Move(m)) => {
                assert_eq!(m.prom_piece, Some(PieceKind::Queen));
                assert!(matches!(m.game_state, GameState::WinWhite));
            }
            _ => panic!("expected valid Move with promotion"),
        }
    }

    #[test]
    fn parse_too_long_message() {
        let msg = "A".repeat(200);
        let result = parse(&msg);
        assert_eq!(result, Err(ParseError::TooLong));
    }

    #[test]
    fn parse_unknown_message_type() {
        let msg = "NotChess:foo:bar:baz:qux";
        let result = parse(msg);
        assert_eq!(result, Err(ParseError::UnknownMessageType));
    }

    #[test]
    fn parse_invalid_move_string() {
        // 'move' string only 3 chars long
        let fen = "8/8/8/8/8/8/8/8";
        let msg = format!("ChessMOVE:a2b:0-0:{}:x", fen);

        let result = parse(&msg);
        assert_eq!(result, Err(ParseError::InvalidMoveFormat));
    }

    #[test]
    fn parse_invalid_game_state() {
        let fen = "8/8/8/8/8/8/8/8";
        let msg = format!("ChessMOVE:a2a40:weird:{}:x", fen);

        let result = parse(&msg);
        assert_eq!(result, Err(ParseError::InvalidGameState));
    }

    #[test]
    fn parse_invalid_fen_char() {
        // `Z` is not a valid FEN piece
        let fen = "8/8/8/8/8/8/8/7Z";
        let msg = format!("ChessMOVE:a2a40:0-0:{}:x", fen);

        let result = parse(&msg);
        assert_eq!(result, Err(ParseError::InvalidFENChar));
    }

    #[test]
    fn parse_invalid_fen_length() {
        // Too short: only 7 ranks
        let fen = "8/8/8/8/8/8/8";
        let msg = format!("ChessMOVE:a2a40:0-0:{}:x", fen);

        let result = parse(&msg);
        assert_eq!(result, Err(ParseError::InvalidFENLength));
    }
}