// idk/src/red.rs

use std::{
    env::args,
    error::Error,
    io::{Error as IoError, ErrorKind},
    ptr::null_mut
};

use libc::{
    c_void,
    ptrace, waitpid,
    PTRACE_ATTACH, PTRACE_PEEKDATA, PTRACE_POKEDATA, PTRACE_DETACH
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = args().skip(1);

    if args.len() != 3 {
        eprintln!("Usage: cargo run --bin red [PID] [ADDRESS] [NEW VALUE]\n(get it from `cargo run --bin blue`)");
        return Ok(());
    }

    let pid       = args.next().ok_or("arg missing: PID")?.parse::<i32>()?;
    let addr      = usize::from_str_radix(&(args.next().ok_or("arg missing: address")?)[2..], 16)?;
    let new_value = args.next().ok_or("arg missing: new value")?.parse::<i64>()?;

    if unsafe { ptrace(PTRACE_ATTACH, pid, null_mut::<*mut c_void>(), null_mut::<*mut c_void>()) } == -1 {
        return Err(format!("PTRACE_ATTACH error: {}", IoError::last_os_error()).into());
    }

    let n  = unsafe { waitpid(pid, null_mut(), 0) };
    if  n != pid {
        return Err(format!("waitpid returned {n}: {}", IoError::last_os_error()).into());
    }

    let data = unsafe { ptrace(PTRACE_PEEKDATA, pid, addr as *mut c_void, null_mut::<*mut c_void>()) };
    let err  = IoError::last_os_error();
    if data == -1 && err.kind() as i32 <= ErrorKind::Other as i32 {
        return Err(format!("PTRACE_PEEKDATA error: {err}").into());
    }

    print!("{pid}[{addr:#x}]: {data}");

    if unsafe { ptrace(PTRACE_POKEDATA, pid, addr as *mut c_void, new_value as *mut c_void) } == -1 {
        return Err(format!("PTRACE_POKEDATA error: {}", IoError::last_os_error()).into());
    }

    println!(" -> {}", unsafe { ptrace(PTRACE_PEEKDATA, pid, addr as *mut c_void, null_mut::<*mut c_void>()) });

    if unsafe { ptrace(PTRACE_DETACH, pid, null_mut::<*mut c_void>(), null_mut::<*mut c_void>()) } == -1 {
        return Err(format!("PTRACE_DETACH error: {}", IoError::last_os_error()).into());
    }

    Ok(())
}

