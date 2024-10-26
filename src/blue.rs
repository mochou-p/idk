// idk/src/blue.rs

use std::{io::{stdin, Result as IoResult}, process::id};

fn main() -> IoResult<()> {
    let x: usize = 420;

    println!("use this: `sudo -E cargo run --bin red {} {x} 1337`", id());
    println!("x = {x}, it is at {:p}", &x);

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    println!("x = {x}");

    Ok(())
}

