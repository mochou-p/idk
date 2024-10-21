// idk/src/blue.rs

use std::{io::{stdin, Result}, process::id};

fn main() -> Result<()> {
    let x: i64 = 420;

    println!("use this: sudo -E cargo run --bin red {} {:p} 1337", id(), &x);
    println!("x = {x}");

    let mut s = String::new();
    stdin().read_line(&mut s)?;

    println!("x = {x}");

    Ok(())
}

