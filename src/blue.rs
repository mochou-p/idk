// idk/src/blue.rs

use std::{io::{stdin, Result as IoResult}, process::id};

fn main() -> IoResult<()> {
    println!("use this: cargo run --bin red {}", id());

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    Ok(())
}

