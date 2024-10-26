// idk/src/red.rs

use std::{
    array::TryFromSliceError,
    env::args,
    error::Error,
    fmt::{Formatter, Result as FmtResult, Display},
    fs::{exists, read_to_string, File},
    io::{BufReader, Error as IoError, ErrorKind, Result as IoResult, BufRead},
    ptr::{from_ref, null_mut}
};

use libc::{
    c_void, iovec,
    process_vm_readv, ptrace, waitpid,
    PTRACE_ATTACH, PTRACE_POKEDATA, PTRACE_DETACH
};

type Pid     = i32;
type Address = usize;
type Size    = usize;

const KIB: Size = 1024;
const MIB: Size = 1024 * KIB;
const GIB: Size = 1024 * MIB;

const POINTER_SIZE: usize = (usize::BITS / u8::BITS) as usize;

trait MemoryRegion {
    fn new(start: Address, end: Address) -> Self;

    fn start(&self) -> Address;
    fn   end(&self) -> Address;

    fn size(&self) -> Size {
        self.end() - self.start()
    }

    fn parse_map<MR: MemoryRegion>(entry: String) -> Result<MR, Box<dyn Error>> {
        let region = &entry[..entry.find(' ').ok_or("PID {pid} has a corrupted maps file")?];
        let dash   = (region.len() - 1) / 2 + 1;
        let start  = Address::from_str_radix(&region[..dash-1], 16)?;
        let end    = Address::from_str_radix(&region[  dash..], 16)?;

        Ok(MR::new(start, end))
    }
}

struct Stack {
    start: Address,
    end:   Address
}

impl MemoryRegion for Stack {
    fn new(start: Address, end: Address) -> Self {
        Self { start, end }
    }

    fn start(&self) -> Address {
        self.start
    }

    fn end(&self) -> Address {
        self.end
    }
}

impl Display for Stack {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "stack:\t{}\t[{:#x} - {:#x}]", pretty_size(self.size()), self.start, self.end)
    }
}

struct Heap {
    start: Address,
    end:   Address
}

impl MemoryRegion for Heap {
    fn new(start: Address, end: Address) -> Self {
        Self { start, end }
    }

    fn start(&self) -> Address {
        self.start
    }

    fn end(&self) -> Address {
        self.end
    }
}

impl Display for Heap {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "heap:\t{}\t[{:#x} - {:#x}]", pretty_size(self.size()), self.start, self.end)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = args().skip(1);

    if args.len() != 3 {
        eprintln!("usage: sudo -E cargo run --bin red [PID] [VALUE] [NEW VALUE]");
        return Ok(());
    }

    let pid       = args.next().unwrap().parse::<Pid>()?;
    let value     = args.next().unwrap().parse::<usize>()?;
    let new_value = args.next().unwrap().parse::<usize>()?;

    check_pid(pid)?;

    println!("process `{}`", get_pid_name(pid)?);

    let (stack, heap_option) = parse_pid_maps(pid)?;

    println!("\n{stack}");

    if let Some(heap) = heap_option {
        println!("{heap}");
    } else {
        println!("no heap");
    }

    let addresses = find_usize_in_memory_region(pid, value, &stack)?;

    match addresses.len() {
        0 => println!("\n{value} not found in stack"),
        1 => {
            let address = addresses[0];

            println!("\none {value} found in stack at {address:#x}, writing {new_value}");

            if unsafe { ptrace(PTRACE_ATTACH, pid, null_mut::<*mut c_void>(), null_mut::<*mut c_void>()) } == -1 {
                return Err(format!("PTRACE_ATTACH error: {}", IoError::last_os_error()).into());
            }

            let n  = unsafe { waitpid(pid, null_mut(), 0) };
            if  n != pid {
                return Err(format!("waitpid returned {n}: {}", IoError::last_os_error()).into());
            }

            if unsafe { ptrace(PTRACE_POKEDATA, pid, address as *mut c_void, new_value as *mut c_void) } == -1 {
                return Err(format!("PTRACE_POKEDATA error: {}", IoError::last_os_error()).into());
            }

            if unsafe { ptrace(PTRACE_DETACH, pid, null_mut::<*mut c_void>(), null_mut::<*mut c_void>()) } == -1 {
                return Err(format!("PTRACE_DETACH error: {}", IoError::last_os_error()).into());
            }
        },
        len => {
            print!("\n{value} found {len}x in stack at: {{ ");

            for address in addresses {
                print!("{address:#x}, ");
            }

            println!("}}");
        }
    }

    Ok(())
}

fn pretty_size(size: Size) -> String {
    #[expect(clippy::cast_precision_loss)]
    match size {
        0  ..KIB => format!("{size} B"),
        KIB..MIB => format!("{:.2} KiB", size as f64 / KIB as f64),
        MIB..GIB => format!("{:.2} MiB", size as f64 / MIB as f64),
        _        => format!("{:.2} GiB", size as f64 / GIB as f64)
    }
}

fn check_pid(pid: Pid) -> IoResult<()> {
    if !exists(format!("/proc/{pid}/"))? {
        return Err(IoError::new(ErrorKind::NotFound, format!("PID {pid} does not exist")));
    }

    Ok(())
}

fn get_pid_name(pid: Pid) -> IoResult<String> {
    let mut name = read_to_string(format!("/proc/{pid}/comm"))?;
    name.pop();

    Ok(name)
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
                    heap_option = Some(<Heap as MemoryRegion>::parse_map(line)?);

                    if stack_option.is_some() {
                        break;
                    }
                },
                "[stack]" => {
                    stack_option = Some(<Stack as MemoryRegion>::parse_map(line)?);

                    if heap_option.is_some() {
                        break;
                    }
                },
                _ => ()
            }
        }
    }

    let stack = stack_option.ok_or("PID {pid} has a corrupted maps file (no stack)")?;

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

    for (i, chunk) in buffer.chunks(POINTER_SIZE).enumerate() {
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

