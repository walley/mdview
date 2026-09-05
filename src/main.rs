mod parse;
mod render;

use std::env;
use std::io::{self, IsTerminal, Read, Write};

use parse::Document;
use render::Style;

const USAGE: &str = "\
mdview - console markdown viewer

USAGE:
    mdview [OPTIONS] [FILE]
    mdview [OPTIONS] -          read markdown from stdin
    mdview < file.md            read stdin when piped

ARGS:
    FILE          Markdown file to render

OPTIONS:
    -b, --no-colors     Disable ANSI colors
    --color             Always emit ANSI color, even when piped
    -w, --width <COLS>  Force output width (default: terminal width)
    -h, --help          Show this help

With no FILE and no piped input, prints this help. Output fits the terminal
width. Colors are emitted only when stdout is a terminal (unless --color).
Set NO_COLOR to disable color in a terminal.\n";

enum Input {
    File(String),
    Stdin,
}

/// Terminal width via the ioctl TIOCGWINSZ syscall.
fn term_width() -> Option<usize> {
    use std::os::fd::AsRawFd;
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let fd = io::stdout().as_raw_fd();
    let r = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) };
    if r == 0 && ws.ws_col > 0 {
        Some(ws.ws_col as usize)
    } else {
        None
    }
}

fn read_source(input: &Input) -> io::Result<String> {
    match input {
        Input::File(p) => std::fs::read_to_string(p),
        Input::Stdin => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

fn print_document(doc: &Document, color: bool) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut buf = String::new();
    let mut prev = Style::default();
    for line in &doc.lines {
        buf.clear();
        if color {
            for run in &line.runs {
                let esc = run.style.to_ansi(prev);
                if !esc.is_empty() {
                    buf.push_str(&esc);
                    prev = run.style;
                }
                buf.push_str(&run.text);
            }
            if prev != Style::default() {
                buf.push_str("\x1b[0m");
                prev = Style::default();
            }
        } else {
            for run in &line.runs {
                buf.push_str(&run.text);
            }
        }
        buf.push('\n');
        match out.write_all(buf.as_bytes()) {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
            Err(e) => return Err(e),
            Ok(()) => {}
        }
    }
    out.flush()?;
    Ok(())
}

fn print_help() -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(USAGE.as_bytes())?;
    out.flush()
}

fn main() -> io::Result<()> {
    let mut width: Option<usize> = None;
    let mut force_color = false;
    let mut no_colors = false;
    let mut input: Option<Input> = None;
    let args: Vec<String> = env::args().skip(1).collect();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-h" | "--help" => return print_help(),
            "-b" | "--no-colors" => no_colors = true,
            "--color" => force_color = true,
            "-w" | "--width" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--width needs a value"))?;
                width = Some(
                    v.parse()
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid --width value"))?,
                );
            }
            "-" => {
                input = Some(Input::Stdin);
            }
            _ if a.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option: {a}"),
                ));
            }
            _ => input = Some(Input::File(a.clone())),
        }
        i += 1;
    }

    // No source given: read piped stdin, else show help.
    let input = match input {
        Some(inp) => inp,
        None => {
            if io::stdin().is_terminal() {
                return print_help();
            }
            Input::Stdin
        }
    };

    let source = read_source(&input)?;

    let cols = width
        .or_else(term_width)
        .or_else(|| env::var("COLUMNS").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(80);
    // Never let width collapse below something sensible.
    let cols = cols.max(20);

    let stdout_is_tty = io::stdout().is_terminal();
    let no_color_env = env::var_os("NO_COLOR").is_some();
    let color = (force_color || (stdout_is_tty && !no_color_env)) && !no_colors;

    let doc = parse::render_markdown(&source, cols);
    print_document(&doc, color)
}