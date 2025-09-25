# Rust Chess GUI

A simple chess game written in Rust, using [`ggez`](https://ggez.rs/) for graphics and [`rsoderh_chess`](https://github.com/rsoderh_chess) for the chess logic.

## How to run
Clone this repo and build with Cargo:
```bash
cargo run
```

## How to use
The project exposes a simple GUI that lets you play chess locally.  

### Controls
- **Left-click on a piece** - select it
- **Left-click on a highlighted square** - move the selected piece  
- **When a pawn promotes** - pick a new piece from the overlay  
- **After checkmate** - click anywhere to reset the game  