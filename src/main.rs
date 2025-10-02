use std::net::{TcpListener, TcpStream};
use std::{env, mem};
use std::collections::HashMap;

pub mod protocol;
pub mod network;

use ggez::{
    Context, ContextBuilder, GameResult,
    event::{self, EventHandler},
    graphics::{self, Image, Drawable},
    input::mouse::MouseButton,
};

use rsoderh_chess::*;

use crate::network::{read_message, send_message, NetError};
use crate::protocol::{Message, MessageMove};

const SCREEN_WIDTH: f32 = 800.0;
const SCREEN_HEIGHT: f32 = 800.0;
const FILES: usize = 8;
const RANKS: usize = 8;
const SQUARE_SIZE: f32 = SCREEN_WIDTH / FILES as f32;

// Represents the current UI state, so either playing or promoting
#[derive(Clone, Copy)]
enum UIState {
    Normal,
    Promotion { column: PositionIndex, color: Color },
}

// Board state
struct GUIBoard {
    pieces_img_map: HashMap<Piece, Image>,
    selected_position: Option<Position>,
    game: Game,
    winner: Option<Color>,
    ui_state: UIState,
}

impl GUIBoard {
    fn new(ctx: &mut Context) -> Self {
        let mut pieces_img_map = HashMap::new();

        let piece_assets = [
            (Color::White, PieceKind::Pawn, "white-pawn"),
            (Color::White, PieceKind::Rook, "white-rook"),
            (Color::White, PieceKind::Knight, "white-knight"),
            (Color::White, PieceKind::Bishop, "white-bishop"),
            (Color::White, PieceKind::Queen, "white-queen"),
            (Color::White, PieceKind::King, "white-king"),
            (Color::Black, PieceKind::Pawn, "black-pawn"),
            (Color::Black, PieceKind::Rook, "black-rook"),
            (Color::Black, PieceKind::Knight, "black-knight"),
            (Color::Black, PieceKind::Bishop, "black-bishop"),
            (Color::Black, PieceKind::Queen, "black-queen"),
            (Color::Black, PieceKind::King, "black-king"),
        ];

        for (color, kind, name) in piece_assets {
            let piece = Piece { color, kind };
            let path = format!("/pieces/{}.png", name);
            let img = Image::from_path(ctx, path).unwrap();
            pieces_img_map.insert(piece, img);
        }

        Self {
            pieces_img_map,
            selected_position: None,
            game: Game::new_standard(),
            winner: None,
            ui_state: UIState::Normal,
        }
    }

    // Reset game to initial state
    fn reset(&mut self) {
        self.game = Game::new_standard();
        self.winner = None;
        self.selected_position = None;
        self.ui_state = UIState::Normal;
    }

    // Draw the full board and overlays
    fn draw(&self, canvas: &mut graphics::Canvas, ctx: &Context) {
        self.draw_squares(canvas);
        self.draw_highlights(canvas);
        self.draw_pieces(canvas);
        self.draw_promotion_overlay(canvas);
        self.draw_winner_banner(canvas, ctx);
    }

    // Draw board squares
    fn draw_squares(&self, canvas: &mut graphics::Canvas) {
        for rank in 0..RANKS {
            for file in 0..FILES {
                let is_black = (rank + file) % 2 == 1;
                let color = if is_black {
                    graphics::Color::from_rgb(0x7c, 0x7c, 0x7c)
                } else {
                    graphics::Color::from_rgb(0xcc, 0xcc, 0xcc)
                };

                let rect = graphics::Rect::new(
                    file as f32 * SQUARE_SIZE,
                    rank as f32 * SQUARE_SIZE,
                    SQUARE_SIZE,
                    SQUARE_SIZE,
                );
                canvas.draw(&graphics::Quad, graphics::DrawParam::new().dest_rect(rect).color(color));
            }
        }
    }

    // Draw selection and valid move highlights
    fn draw_highlights(&self, canvas: &mut graphics::Canvas) {
        let Some(src_position) = self.selected_position else { return };

        // Selected square
        let rect = graphics::Rect::new(
            src_position.column() as f32 * SQUARE_SIZE,
            (7 - src_position.row()) as f32 * SQUARE_SIZE,
            SQUARE_SIZE,
            SQUARE_SIZE,
        );
        canvas.draw(
            &graphics::Quad,
            graphics::DrawParam::new()
                .dest_rect(rect)
                .color(graphics::Color::from_rgba(0xF5, 0xF5, 0xDC, 128)),
        );

        // Valid moves
        if let Some(valid_moves) = self.game.valid_moves(src_position) {
            for pos in valid_moves.iter() {
                let rect = graphics::Rect::new(
                    pos.column() as f32 * SQUARE_SIZE,
                    (7 - pos.row()) as f32 * SQUARE_SIZE,
                    SQUARE_SIZE,
                    SQUARE_SIZE,
                );
                canvas.draw(
                    &graphics::Quad,
                    graphics::DrawParam::new()
                        .dest_rect(rect)
                        .color(graphics::Color::from_rgba(0xA6, 0x7B, 0x5B, 128)),
                );
            }
        }
    }

    // Draw chess pieces
    fn draw_pieces(&self, canvas: &mut graphics::Canvas) {
        for rank in 0..8 {
            for file in 0..8 {
                let slot = self.game.board().at_position(Position::new(file, rank).unwrap());
                if let Slot::Occupied(piece) = slot {
                    if let Some(img) = self.pieces_img_map.get(&piece) {
                        let dest_x = file as f32 * SQUARE_SIZE;
                        let dest_y = (7 - rank) as f32 * SQUARE_SIZE;

                        let scale = [
                            SQUARE_SIZE / img.width() as f32,
                            SQUARE_SIZE / img.height() as f32,
                        ];

                        canvas.draw(img, graphics::DrawParam::new().dest([dest_x, dest_y]).scale(scale));
                    }
                }
            }
        }
    }

    // Draw promotion overlay
    fn draw_promotion_overlay(&self, canvas: &mut graphics::Canvas) {
        if let UIState::Promotion { color, .. } = self.ui_state {
            // Dim background
            let dim_rect = graphics::Rect::new(0.0, 0.0, SCREEN_WIDTH, SCREEN_HEIGHT);
            canvas.draw(
                &graphics::Quad,
                graphics::DrawParam::new()
                    .dest_rect(dim_rect)
                    .color(graphics::Color::from_rgba(0, 0, 0, 160)),
            );

            // Promotion choices
            let choices = [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight];
            let start_x = SCREEN_WIDTH / 2.0 - 2.0 * SQUARE_SIZE;
            let y = SCREEN_HEIGHT / 2.0 - SQUARE_SIZE / 2.0;

            for (i, kind) in choices.iter().enumerate() {
                let x = start_x + i as f32 * SQUARE_SIZE;

                // Background tile
                let tile = graphics::Rect::new(x, y, SQUARE_SIZE, SQUARE_SIZE);
                canvas.draw(
                    &graphics::Quad,
                    graphics::DrawParam::new()
                        .dest_rect(tile)
                        .color(graphics::Color::from_rgba(240, 240, 240, 220)),
                );

                let piece = Piece { color, kind: *kind };
                if let Some(img) = self.pieces_img_map.get(&piece) {
                    let scale = [
                        SQUARE_SIZE / img.width() as f32,
                        SQUARE_SIZE / img.height() as f32,
                    ];
                    canvas.draw(img, graphics::DrawParam::new().dest([x, y]).scale(scale));
                }
            }
        }
    }

    // Draw winner banner if game is finished
    fn draw_winner_banner(&self, canvas: &mut graphics::Canvas, ctx: &Context) {
        let Some(winner) = self.winner else { return };

        let msg = match winner {
            Color::White => "White wins!",
            Color::Black => "Black wins!",
        };

        let text = graphics::Text::new(graphics::TextFragment {
            text: msg.to_string(),
            scale: Some(graphics::PxScale::from(120.0)),
            ..Default::default()
        });

        let dims = text.dimensions(ctx);
        let dest_point = [
            SCREEN_WIDTH / 2.0 - dims.w as f32 / 2.0,
            SCREEN_HEIGHT / 2.0 - dims.h as f32 / 2.0,
        ];

        // Outline
        let outline = 3.0;
        for (dx, dy) in [
            (-outline, 0.0), (outline, 0.0), (0.0, -outline), (0.0, outline),
            (-outline, -outline), (outline, -outline), (-outline, outline), (outline, outline),
        ] {
            canvas.draw(
                &text,
                graphics::DrawParam::new()
                    .dest([dest_point[0] + dx, dest_point[1] + dy])
                    .color(graphics::Color::BLACK),
            );
        }

        // Main text
        canvas.draw(
            &text,
            graphics::DrawParam::new()
                .dest(dest_point)
                .color(graphics::Color::WHITE),
        );
    }

    // Replace game state and perform move
    fn perform_move(&mut self, mv: HalfMoveRequest) {
        let placeholder = Game::new(self.game.board().clone(), self.game.turn);
        let game = mem::replace(&mut self.game, placeholder);
        let result = game.perform_move(mv);

        self.game = match result {
            MoveResult::Ongoing(new_game, check) => {
                println!("Check outcome: {:?}", check);
                new_game
            }
            MoveResult::Finished(finished) => {
                println!("Game over: {:?}", finished.result());

                let rsoderh_chess::GameResult::Checkmate { winner, .. } = finished.result();
                self.winner = Some(*winner);

                Game::new(finished.board().clone(), self.game.turn)
            }
            MoveResult::Illegal(game, why) => {
                println!("Illegal move: {:?}", why);
                game
            }
        };
    }
}

// Main game container
struct MyGame {
    board: GUIBoard,
    stream: Option<TcpStream>,
    playing_as: Color,
}

impl MyGame {
    pub fn new(ctx: &mut Context, stream: Option<TcpStream>, playing_as: Color) -> Self {
        Self { board: GUIBoard::new(ctx), stream, playing_as }
    }
}

impl EventHandler for MyGame {
    fn update(&mut self, _ctx: &mut Context) -> GameResult {
        if self.board.game.turn == self.playing_as {
            return Ok(());
        }
        match self.stream.as_mut() {
            Some(stream) => {
                let message = read_message(stream);
                match message {
                    Ok(message) => {
                        match message {
                            Message::Move(message) => {
                                let placeholder = Game::new(self.board.game.board().clone(), self.board.game.turn);
                                let game = mem::replace(&mut self.board.game, placeholder);
                                let result = match message.prom_piece {
                                    Some(piece_kind) => game.perform_move(HalfMoveRequest::Promotion { column: message.mv.1.column, kind: piece_kind }),
                                    None => game.perform_move(HalfMoveRequest::Standard { source: message.mv.0, dest: message.mv.1 }),
                                };

                                self.board.game = match result {
                                    MoveResult::Ongoing(new_game, check) => {
                                        println!("Check outcome: {:?}", check);
                                        new_game
                                    }
                                    MoveResult::Finished(finished) => {
                                        println!("Game over: {:?}", finished.result());

                                        let rsoderh_chess::GameResult::Checkmate { winner, .. } = finished.result();
                                        self.board.winner = Some(*winner);

                                        Game::new(finished.board().clone(), self.board.game.turn)
                                    }
                                    MoveResult::Illegal(_game, why) => {
                                        println!("Illegal move: {:?}", why);
                                        let _ = send_message(stream, &Message::Quit("Desync".to_string()));
                                        panic!("Board desync!!!");
                                    }
                                };
                            },
                            Message::Quit(s) => {
                                panic!("Opponent quit: {s}");
                            }
                        }
                    },
                    Err(e) => {
                        match e {
                            NetError::IoError(_e) => {},
                            NetError::ParseError(e) => panic!("Failed to read opponent moves: {e:?}"),
                            NetError::SerializeError(e) => panic!("Failed to read opponent moves: {e:?}"),
                        }
                    }
                }
            },
            None => (),
        };
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, graphics::Color::WHITE);
        self.board.draw(&mut canvas, ctx);
        canvas.finish(ctx)
    }

    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut Context,
        button: MouseButton,
        x: f32,
        y: f32,
    ) -> GameResult {
        if button != MouseButton::Left {
            return Ok(());
        }
        // Don't play if it's not your turn
        if self.board.game.turn != self.playing_as {
            return Ok(());
        }
        // Reset if game ended
        if self.board.winner.is_some() {
            self.board.reset();
            return Ok(());
        }

        // Handle promotion overlay
        if let UIState::Promotion { column, .. } = self.board.ui_state {
            let choices = [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight];
            let start_x = SCREEN_WIDTH / 2.0 - 2.0 * SQUARE_SIZE;
            let y_choice = SCREEN_HEIGHT / 2.0 - SQUARE_SIZE / 2.0;

            for (i, kind) in choices.iter().enumerate() {
                let x_choice = start_x + i as f32 * SQUARE_SIZE;
                let inside = x >= x_choice && x <= x_choice + SQUARE_SIZE
                    && y >= y_choice && y <= y_choice + SQUARE_SIZE;

                if inside {
                    self.board.perform_move(HalfMoveRequest::Promotion { column, kind: *kind });
                    self.board.ui_state = UIState::Normal;
                    self.board.selected_position = None;

                    unimplemented!("Promotion not implemented");
                }
            }
            return Ok(());
        }

        // Normal board interaction
        let col = (x / SQUARE_SIZE).floor() as u8;
        let rank = 7 - (y / SQUARE_SIZE).floor() as u8;
        let clicked_position = Position::new(col, rank).unwrap();
        let clicked_square = self.board.game.board().at_position(clicked_position);

        match self.board.selected_position {
            Some(src_position) => {
                if let Some(valid_moves) = self.board.game.valid_moves(src_position) {
                    if valid_moves.into_iter().any(|mv| mv == clicked_position) {
                        // Pawn promotion
                        if let Slot::Occupied(piece) = self.board.game.board().at_position(src_position) {
                            let is_promotion_rank =
                                (piece.color == Color::White && clicked_position.row() == 7) ||
                                (piece.color == Color::Black && clicked_position.row() == 0);

                            if piece.kind == PieceKind::Pawn && is_promotion_rank {
                                self.board.ui_state = UIState::Promotion {
                                    column: clicked_position.column,
                                    color: piece.color,
                                };
                                self.board.selected_position = None;
                                return Ok(());
                            }
                        }
                        // Regular move
                        self.board.perform_move(HalfMoveRequest::Standard {
                            source: src_position,
                            dest: clicked_position,
                        });
                        let message = Message::Move(MessageMove {
                            board: self.board.game.board().clone(),
                            mv: (src_position, clicked_position),
                            prom_piece: None,
                            game_state: protocol::GameState::Ongoing,
                        });

                        match self.stream.as_mut() {
                            Some(stream) => { let _ = send_message(&stream, &message); }
                            None => { self.playing_as = if self.playing_as == Color::White {Color::Black} else {Color::White}; }
                        };

                    }
                }
                self.board.selected_position = None;
            }
            None => {
                if let Slot::Occupied(piece) = clicked_square {
                    if piece.color == self.board.game.turn {
                        self.board.selected_position = Some(clicked_position);
                    }
                }
            }
        }

        Ok(())
    }
}

fn parse_cmd(mut ctx: &mut Context, args: Vec<String>) -> MyGame {
    if let Some(address) = args.get(1) {
        if let Some(server_str) = args.get(2) && server_str == "server" {
            let listener = TcpListener::bind(address);
            let listener = match listener {
                Ok(listener) => listener,
                Err(e) => panic!("Couldn't not bind to address '{}': {e:?}", address),
            };
            println!("Waiting for opponent...");
            let stream = match listener.accept() {
                Ok((stream, _addr)) => { let _ = stream.set_nonblocking(true); MyGame::new(&mut ctx, Some(stream), Color::White) },
                Err(e) => panic!("Opponent failed to connect: {e:?}"),
            };
            print!("Opponent connected!");
            stream
        } else if let Some(client_str) = args.get(2) && client_str == "client" { 
            let stream = match TcpStream::connect(address) {
                Ok(stream) => stream,
                Err(e) => panic!("Failed to connect to opponent: {e:?}"),
            };
            let _ = stream.set_nonblocking(true);
            MyGame::new(&mut ctx, Some(stream), Color::Black)
        } else {
            panic!("You have to specify 'server' or 'client' after the address");
        }
    } else {
        MyGame::new(&mut ctx, None, Color::White)
    }
}

fn main() {
    let (mut ctx, event_loop) = ContextBuilder::new("my_game", "Author")
        .window_mode(ggez::conf::WindowMode::default().dimensions(SCREEN_WIDTH, SCREEN_HEIGHT))
        .add_resource_path("./resources")
        .build()
        .expect("Failed to create ggez context");

    ctx.gfx.set_window_title("Chess");

    let args: Vec<String> = env::args().collect();
    let my_game = parse_cmd(&mut ctx, args);

    event::run(ctx, event_loop, my_game).expect("Program failed");
}
