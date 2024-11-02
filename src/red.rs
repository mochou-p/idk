// purple/src/red.rs

use std::{
    array::TryFromSliceError,
    env::args,
    error::Error,
    fs::{exists, read_to_string, File},
    io::{stdin, stdout, BufReader, Error as IoError, Result as IoResult, BufRead, Write},
    mem::zeroed,
    ptr::{from_ref, null_mut}
};

use libc::{
    c_int, c_void, iovec, winsize,
    abort, ioctl, process_vm_readv, ptrace, signal, waitpid, write,
    PTRACE_ATTACH, PTRACE_PEEKDATA, PTRACE_POKEDATA, PTRACE_DETACH, SIGINT, SIG_ERR, STDOUT_FILENO, TIOCGWINSZ
};


type Pid     = i32;
type Address = usize;
type Size    = usize;

const HEX:      u32   = 16;
const PTR_SIZE: usize = (usize::BITS / u8::BITS) as usize;

macro_rules! error {
    ($msg:expr, $end:expr) => {
        eprint!("\x1b[31;1minvalid input `{}`\x1b[0m{}", $msg, $end);
    }
}

macro_rules! impl_memory_region {
    ($t:ty, $badge:expr) => {
        impl MemoryRegion for $t {
            fn new(start: Address, end: Address) -> Self {
                Self { start, end }
            }

            fn start(&self) -> Address { self.start }
            fn   end(&self) -> Address { self.end   }

            fn badge() -> &'static str {
                $badge
            }
        }
    }
}

trait MemoryRegion {
    fn new(start: Address, end: Address) -> Self;

    fn start(&self) -> Address;
    fn   end(&self) -> Address;

    fn size(&self) -> Size {
        self.end() - self.start()
    }

    fn badge() -> &'static str;
}

struct Stack { start: Address, end: Address }
struct Heap  { start: Address, end: Address }

impl_memory_region!(Stack, "\x1b[44;30;1m stack \x1b[0m");
impl_memory_region!(Heap,  "\x1b[43;30;1m heap \x1b[0m");

static mut ATTACHED: bool = false;

extern "C" fn sigint_handler(_signum: c_int) {
    let buf   = "\x1b[H\x1b[J";
    let count = buf.len();

    unsafe {
        write(STDOUT_FILENO, buf.as_ptr().cast::<c_void>(), count);
        abort();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = args().skip(1);
    if args.len() != 1 {
        return Err("usage: sudo -E cargo run --bin red [PID]".into());
    }

    let pid = args.next().unwrap().parse::<Pid>()?;
    check_pid(pid)?;
    let name = get_pid_name(pid)?;

    let (stack, heap_option) = parse_pid_maps(pid)?;

    if unsafe { signal(SIGINT, sigint_handler as usize) } == SIG_ERR {
        return Err(format!("signal error: {}", IoError::last_os_error()).into());
    }

    let (mut input, mut is_stack, mut list) = (String::new(), true, vec![]);

    print!("\x1b[3J\x1b[1J\x1b[H`{name}`\n{} \x1b[46;30;1m usize \x1b[0m ", Stack::badge());
    stdout().flush()?;

    loop {
        input.clear();
        stdin().read_line(&mut input)?;

        match input.trim() {
            "e" | "exit" | "" => {
                print!("\x1b[H\x1b[J");
                break;
            },
            "c" | "clear" => {
                list.clear();
                print!("\x1b[J");
                move_cursor(is_stack)?;
            },
            "s" | "stack" => if is_stack {
                move_cursor(is_stack)?;
            } else {
                list.clear();
                is_stack = !is_stack;
                print!("\x1b[2;H{} \x1b[46;30;1m usize \x1b[0m \x1b[J", Stack::badge());
                stdout().flush()?;
            },
            "h" | "heap" => if heap_option.is_some() && is_stack {
                list.clear();
                is_stack = !is_stack;
                print!("\x1b[2;H{} \x1b[46;30;1m usize \x1b[0m \x1b[J", Heap::badge());
                stdout().flush()?;
            } else {
                move_cursor(is_stack)?;
            },
            cmd => if let Ok(value) = cmd.parse::<usize>() {
                if list.is_empty() {
                    if is_stack {
                        list = find_usize_in_memory_region(pid, value, &stack)?;
                    } else if let Some(ref heap) = heap_option {
                        list = find_usize_in_memory_region(pid, value, heap)?;
                    }

                    if list.len() == 1 {
                        write_to_address(pid, list[0], value)?;
                        break;
                    }

                    print_address_list(&list)?;
                    move_cursor(is_stack)?;
                } else {
                    filter_addresses(pid, &mut list, value)?;

                    if list.len() == 1 {
                        write_to_address(pid, list[0], value)?;
                        break;
                    }

                    print_address_list(&list)?;
                    print!("\x1b[J");
                    move_cursor(is_stack)?;
                }
            } else {
                error!("unknown command", "\x1b[J");
                move_cursor(is_stack)?;
            }
        }
    }

    Ok(())
}

fn ptrace_attach(pid: Pid) -> Result<(), Box<dyn Error>> {
    if unsafe { ATTACHED } {
        ptrace_detach(pid)?;

        return Err("double PTRACE_ATTACH detected".into());
    }

    if unsafe { ptrace(PTRACE_ATTACH, pid, null_mut::<*mut c_void>(), null_mut::<*mut c_void>()) } == -1 {
        return Err(format!("PTRACE_ATTACH error: {}", IoError::last_os_error()).into());
    }

    unsafe { ATTACHED = true; }

    match unsafe { waitpid(pid, null_mut(), 0) } {
        x if x == pid => (),
        n => {
            ptrace_detach(pid)?;

            return Err(format!("waitpid returned {n}: {}", IoError::last_os_error()).into());
        }
    }

    Ok(())
}

fn ptrace_peek(pid: Pid, address: Address) -> Result<usize, Box<dyn Error>> {
    if !unsafe { ATTACHED } {
        return Err("cannot PTRACE_PEEKDATA before PTRACE_ATTACH and waitpid".into());
    }

    match unsafe { ptrace(PTRACE_PEEKDATA, pid, address as *mut c_void, null_mut::<*mut c_void>()) } {
        -1 => {
            ptrace_detach(pid)?;

            Err(format!("PTRACE_PEEKDATA error: {}", IoError::last_os_error()).into())
        },
        #[expect(clippy::cast_possible_truncation, reason = "false positive, libc::c_long is i32 on 32bit")]
        #[expect(clippy::cast_sign_loss,           reason = "that is intended")]
        value => Ok(value as usize)
    }
}

fn ptrace_poke(pid: Pid, address: Address, value: usize) -> Result<(), Box<dyn Error>> {
    if !unsafe { ATTACHED } {
        return Err("cannot PTRACE_POKEDATA before PTRACE_ATTACH and waitpid".into());
    }

    if unsafe { ptrace(PTRACE_POKEDATA, pid, address as *mut c_void, value as *mut c_void) } == -1 {
        ptrace_detach(pid)?;

        return Err(format!("PTRACE_POKEDATA error: {}", IoError::last_os_error()).into());
    }

    Ok(())
}

fn ptrace_detach(pid: Pid) -> Result<(), Box<dyn Error>> {
    if !unsafe { ATTACHED } {
        return Err("double PTRACE_DETACH detected".into());
    }

    if unsafe { ptrace(PTRACE_DETACH, pid, null_mut::<*mut c_void>(), null_mut::<*mut c_void>()) } == -1 {
        return Err(format!("PTRACE_DETACH error: {}", IoError::last_os_error()).into());
    }

    unsafe { ATTACHED = false; }

    Ok(())
}

fn move_cursor(is_stack: bool) -> IoResult<()> {
    if is_stack {
        print!("\x1b[2;17H\x1b[K");
        stdout().flush()?;
    } else {
        print!("\x1b[2;16H\x1b[K");
        stdout().flush()?;
    }

    Ok(())
}

fn print_address_list(addresses: &[Address]) -> Result<(), Box<dyn Error>> {
    if addresses.is_empty() {
        print!("no matches (list cleared)\x1b[K");
        return Ok(());
    }

    let mut size = unsafe { zeroed::<winsize>() };

    if unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size) } == -1 {
        return Err(format!("ioctl error: {}", IoError::last_os_error()).into());
    }

    let len   = addresses.len();
    let clamp = (size.ws_row as usize - 2).min(len);

    print!("\x1b[K");

    for address in addresses.iter().take(clamp - 1) {
        println!("{address:#x}");
    }

    if clamp < len {
        print!("... ({} more)", len - clamp + 1);
    } else {
        print!("{:#x}", addresses[clamp - 1]);
    }

    stdout().flush()?;

    Ok(())
}

fn filter_addresses(pid: Pid, addresses: &mut Vec<Address>, value: usize) -> Result<(), Box<dyn Error>> {
    assert!(!addresses.is_empty(), "list is empty (should never happen, call is guarded)");

    ptrace_attach(pid)?;

    let mut i = 0;
    while i < addresses.len() {
        if ptrace_peek(pid, addresses[i])? == value {
            i += 1;
        } else {
            addresses.swap_remove(i);
        }
    }

    ptrace_detach(pid)?;

    Ok(())
}

fn write_to_address(pid: Pid, address: Address, old_value: usize) -> Result<(), Box<dyn Error>> {
    print!("\x1b[H[{address:#x}] {old_value} -> \x1b[J");
    stdout().flush()?;

    let mut input = String::new();
    stdin().read_line(&mut input)?;

    match input.trim().parse() {
        Ok(value) => {
            ptrace_attach(pid)?;
            ptrace_poke(pid, address, value)?;
            print!("\x1b[H\x1b[J");
            stdout().flush()?;
            ptrace_detach(pid)?;
        },
        Err(err) => {
            error!(err, '\n');
        }
    }

    Ok(())
}

fn check_pid(pid: Pid) -> Result<(), Box<dyn Error>> {
    if !exists(format!("/proc/{pid}/"))? {
        return Err(format!("PID {pid} does not exist").into());
    }

    Ok(())
}

fn get_pid_name(pid: Pid) -> IoResult<String> {
    let mut name = read_to_string(format!("/proc/{pid}/comm"))?;
    name.pop();

    Ok(name)
}

fn parse_map<MR: MemoryRegion>(entry: &str) -> Result<MR, Box<dyn Error>> {
    let region = &entry[..entry.find(' ').ok_or("PID {pid} has a corrupted maps file")?];
    let dash   = (region.len() - 1) / 2 + 1;
    let start  = Address::from_str_radix(&region[..dash-1], HEX)?;
    let end    = Address::from_str_radix(&region[  dash..], HEX)?;

    Ok(MR::new(start, end))
}

fn parse_pid_maps(pid: Pid) -> Result<(Stack, Option<Heap>), Box<dyn Error>> {
    let file   = File::open(format!("/proc/{pid}/maps"))?;
    let reader = BufReader::new(file);
    let lines  = reader.lines();

    let (mut stack_option, mut heap_option) = (None, None);

    for line_option in lines {
        let line = line_option?;

        if line.chars().last().ok_or("PID {pid} has a corrupted maps file (empty line)")? == ']' {
            let start = line.rfind('[').ok_or("PID {pid} has a corrupted maps file (no matching bracket)")?;

            match &line[start..] {
                "[heap]" => {
                    heap_option = Some(parse_map(&line)?);

                    if stack_option.is_some() {
                        break;
                    }
                },
                "[stack]" => {
                    stack_option = Some(parse_map(&line)?);

                    if heap_option.is_some() {
                        break;
                    }
                },
                _ => ()
            }
        }
    }

    let stack = stack_option.ok_or("PID {pid} has a corrupted maps file (no stack) (should never happen)")?;

    Ok((stack, heap_option))
}

fn find_usize_in_memory_region<MR: MemoryRegion>(pid: Pid, value: usize, memory_region: &MR) -> Result<Vec<Address>, Box<dyn Error>> {
    let mut buffer = vec![0_u8; memory_region.size()];

    let start = memory_region.start();

    let dst = iovec { iov_base: buffer.as_mut_ptr().cast::<c_void>(), iov_len: buffer.len() };
    let src = iovec { iov_base: start as *mut c_void,                 iov_len: buffer.len() };

    if unsafe { process_vm_readv(pid, from_ref::<iovec>(&dst), 1, from_ref::<iovec>(&src), 1, 0) } == -1 {
        return Err(format!("process_vm_readv error: {}", IoError::last_os_error()).into());
    }

    let mut matches = vec![];

    for (i, chunk) in buffer.chunks(PTR_SIZE).enumerate() {
        if u8_slice_to_usize(chunk)? == value {
            matches.push(start + (i * u8::BITS as usize));
        }
    }

    Ok(matches)
}

fn u8_slice_to_usize(slice: &[u8]) -> Result<usize, TryFromSliceError> {
    #[cfg(target_endian = "big")]
    return Ok(usize::from_be_bytes(slice.try_into()?));

    #[cfg(target_endian = "little")]
    return Ok(usize::from_le_bytes(slice.try_into()?));
}

