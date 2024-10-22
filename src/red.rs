// idk/src/red.rs

use std::{
    env::args,
    error::Error,
    fmt::{Formatter, Result as FmtResult, Display},
    fs::{exists, read_to_string, File},
    io::{BufReader, Error as IoError, ErrorKind, Result as IoResult, BufRead}
};

type Pid     = i32;
type Address = usize;
type Size    = usize;

const KIB: Size = 1024;
const MIB: Size = 1024 * 1024;
const GIB: Size = 1024 * 1024 * 1024;

trait MemoryRegion {
    fn new(start: Address, end: Address) -> Self;

    fn start(&self) -> Address;
    fn   end(&self) -> Address;

    fn size(&self) -> Size {
        self.end() - self.start()
    }

    fn parse_map<T: MemoryRegion>(entry: String) -> Result<T, Box<dyn Error>> {
        let region = &entry[..entry.find(' ').ok_or("PID {pid} has a corrupted maps file")?];
        let dash   = (region.len() - 1) / 2 + 1;
        let start  = Address::from_str_radix(&region[..dash-1], 16)?;
        let end    = Address::from_str_radix(&region[  dash..], 16)?;

        Ok(T::new(start, end))
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

    if args.len() != 1 {
        eprintln!("Usage: cargo run --bin red [PID]");
        return Ok(());
    }

    let pid = args.next().ok_or("arg missing: PID")?.parse::<Pid>()?;

    check(pid)?;

    println!("process `{}`", get_name(pid)?);

    let (stack, heap_option) = parse_maps(pid)?;

    println!("{stack}");

    if let Some(heap) = heap_option {
        println!("{heap}");
    } else {
        println!("No heap");
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

fn check(pid: Pid) -> IoResult<()> {
    if !exists(format!("/proc/{pid}/"))? {
        return Err(IoError::new(ErrorKind::NotFound, format!("PID {pid} does not exist")));
    }

    Ok(())
}

fn get_name(pid: Pid) -> IoResult<String> {
    let mut name = read_to_string(format!("/proc/{pid}/comm"))?;
    name.pop();

    Ok(name)
}

fn parse_maps(pid: Pid) -> Result<(Stack, Option<Heap>), Box<dyn Error>> {
    let file   = File::open(format!("/proc/{pid}/maps"))?;
    let reader = BufReader::new(file);
    let lines  = reader.lines();

    let (mut stack_option, mut heap_option) = (None, None);

    for line_option in lines {
        let line = line_option?;

        if line.chars().last().ok_or("PID {pid} has a corrupted maps file")? == ']' {
            let start = line.rfind('[').ok_or("PID {pid} has a corrupted maps file")?;

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

    let stack = stack_option.ok_or("PID {pid} has no [stack] (this should never happen)")?;

    Ok((stack, heap_option))
}

