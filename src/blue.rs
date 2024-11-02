// idk/src/blue.rs

use std::{error::Error, io::stdin, process::id};


macro_rules! error {
    ($msg:expr) => {
        eprintln!("\n\x1b[31;1minvalid input `{}`\x1b[0m\nusage examples: +33, -5, *11, /2\nor empty to exit\n", $msg)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let (mut x, mut input) = (0_usize, String::new());

    println!("my PID is {}\n\n\x1b[34;1mx = {x}\x1b[0m", id());

    loop {
        input.clear();
        stdin().read_line(&mut input)?;

        match input.trim() {
            "e" | "exit" | "" => {
                println!("\x1b[36;1mx = {x}\x1b[0m");
                break;
            },
            cmd => match cmd.chars().next().ok_or("invalid input")? {
                op @ ('+' | '-' | '*' | '/') => match &cmd[1..].trim().parse::<usize>() {
                    Ok(n)    => math(&mut x, op, *n),
                    Err(err) => error!(err)
                },
                _ => error!("invalid operator found as first char")
            }
        }

        println!("\x1b[34;1mx = {x}\x1b[0m");
    }

    Ok(())
}

fn math(a: &mut usize, op: char, b: usize) {
    match op {
        '+' => *a += b,
        '-' => *a -= b,
        '*' => *a *= b,
        '/' => *a /= b,
        _   => panic!("invalid operator (should never happen, call is guarded)")
    }
}

