# Puzterm

An incomplete but playable crossword puzzle for the terminal with Rust, [Termion](https://github.com/ticki/termion) and [Nom](https://github.com/Geal/nom).

Can currently read non-scrambled across lite (.puz) files without rebuses.

## Screenshot

![Screenshot](../readme-assets/readme-assets/screenshot-1.png?raw=true)

## Usage

`puzterm file.puz`

## Controls

| Keys            | Action       |
| --------------- | ------------ |
| pgup pgdown [ ] | scroll clues |

### Normal Mode

| Keys              | Action       |
| ----------------- | ------------ |
| wasd hjkl ← → ↑ ↓ | move         |
| enter i           | edit mode    |
| e                 | hint         |
| p q ctrl-c        | pause / quit |

### Edit Mode

| Keys      | Action           |
| --------- | ---------------- |
| esc enter | normal mode      |
| ← → ↑ ↓   | move             |
| space     | change direction |
| backspace | previous square  |
