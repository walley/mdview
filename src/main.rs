mod parse;
mod render;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use render::{Color, Line, Style};
use std::io::{self, Read, Write};

/// Restore the terminal on exit (panic-safe).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = crossterm::execute!(
            stdout,
            crossterm::event::DisableMouseCapture,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        let _ = stdout.flush();
    }
}

fn read_source(args: &[String]) -> io::Result<(String, Option<String>)> {
    let Some(path) = args.first() else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        return Ok((buf, None));
    };
    if path == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        return Ok((buf, None));
    }
    let data = std::fs::read_to_string(path)
        .map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
    Ok((data, Some(path.clone())))
}

struct Viewer {
    source: String,
    path: Option<String>,
    doc: parse::Document,
    scroll: usize,
    search: Option<String>,
    prompt: Option<Prompt>,
}

struct Prompt {
    label: String,
    buf: String,
    on_submit: fn(&mut Prompt) -> Option<String>,
}

impl Viewer {
    fn reload(&mut self, width: usize, view_h: usize) {
        self.doc = parse::render_markdown(&self.source, width);
        self.clamp(view_h);
    }

    fn clamp(&mut self, view_h: usize) {
        let max = self.doc.lines.len().saturating_sub(view_h);
        self.scroll = self.scroll.min(max);
    }

    fn go(&mut self, scroll: usize, view_h: usize) {
        self.scroll = scroll;
        self.clamp(view_h);
    }

    fn scroll_by(&mut self, delta: i64, view_h: usize) {
        let mut s = self.scroll as i64 + delta;
        if s < 0 {
            s = 0;
        }
        self.scroll = s as usize;
        self.clamp(view_h);
    }

    fn find_next(&self, from: usize, step: i64) -> Option<usize> {
        let n = self.doc.lines.len() as i64;
        if n == 0 {
            return None;
        }
        let mut i = from as i64;
        for _ in 0..n {
            i += step;
            if i < 0 {
                i = n - 1;
            } else if i >= n {
                i = 0;
            }
            if line_matches(&self.doc.lines[i as usize], self.search.as_deref()) {
                return Some(i as usize);
            }
        }
        None
    }
}

fn line_matches(line: &Line, q: Option<&str>) -> bool {
    let Some(q) = q else { return false };
    if q.is_empty() {
        return false;
    }
    let plain: String = line.runs.iter().map(|r| r.text.as_str()).collect();
    plain.to_lowercase().contains(&q.to_lowercase())
}

/// Character-index ranges of a case-insensitive query within a line.
fn find_matches(line: &Line, q: &str) -> Vec<(usize, usize)> {
    let mut res = Vec::new();
    let qchars: Vec<char> = q.chars().collect();
    if qchars.is_empty() {
        return res;
    }
    let full: String = line.runs.iter().map(|r| r.text.as_str()).collect();
    let chars: Vec<char> = full.chars().collect();
    let n = chars.len();
    if n < qchars.len() {
        return res;
    }
    let mut i = 0;
    while i + qchars.len() <= n {
        let mut ok = true;
        for k in 0..qchars.len() {
            let a = chars[i + k].to_lowercase().next().unwrap_or(chars[i + k]);
            let b = qchars[k].to_lowercase().next().unwrap_or(qchars[k]);
            if a != b {
                ok = false;
                break;
            }
        }
        if ok {
            res.push((i, i + qchars.len()));
            i += qchars.len();
        } else {
            i += 1;
        }
    }
    res
}

/// Append a line's ANSI rendering to `buf`, highlighting `matches`, tracking
/// the running style in `prev`.
fn write_line_impl(buf: &mut String, line: &Line, matches: &[(usize, usize)], prev: &mut Style) {
    let match_style = Style {
        fg: Color::Black,
        bg: Color::BrightYellow,
        bold: true,
        ..Style::default()
    };
    let mut ci = 0usize;
    let is_match = |ci: usize| matches.iter().any(|&(s, e)| ci >= s && ci < e);
    for run in &line.runs {
        for ch in run.text.chars() {
            let st = if is_match(ci) {
                match_style
            } else {
                run.style
            };
            let esc = st.to_ansi(*prev);
            if !esc.is_empty() {
                buf.push_str(&esc);
                *prev = st;
            }
            buf.push(ch);
            ci += 1;
        }
    }
}

fn status_line(v: &Viewer, view_h: usize, width: usize) -> String {
    let total = v.doc.lines.len();
    let top = v.scroll.min(total);
    let bottom = (v.scroll + view_h).min(total);
    let pct = if total == 0 {
        100
    } else {
        top.saturating_mul(100) / total
    };
    let mut st = String::new();
    st.push_str("\x1b[7m");
    match &v.path {
        Some(p) => st.push_str(&format!(" mdview {p} ")),
        None => st.push_str(" mdview stdin "),
    }
    st.push_str(&format!("{top}–{bottom}/{total} {pct}%"));
    if v.search.is_some() {
        st.push_str(&format!("  /{} [n/N next]", v.search.as_deref().unwrap_or("")));
    }
    if let Some(p) = &v.prompt {
        st.push_str(&format!("  {}{}", p.label, p.buf));
    } else {
        st.push_str("  [j/k scroll · / search · g/G top/bottom · q quit]");
    }
    while render::display_width(&st) < width {
        st.push(' ');
    }
    st.push_str("\x1b[0m");
    st
}

fn draw(v: &Viewer, rows: usize) -> String {
    let width = v.doc.width;
    let view_h = rows.saturating_sub(1);
    let mut buf = String::new();
    buf.push_str("\x1b[H\x1b[2J");
    let mut prev = Style::default();
    for y in 0..view_h {
        let li = v.scroll + y;
        buf.push_str(&format!("\x1b[{};1H", y + 1));
        if li < v.doc.lines.len() {
            let line = &v.doc.lines[li];
            let matches = match &v.search {
                Some(q) if !q.is_empty() => find_matches(line, q),
                _ => Vec::new(),
            };
            write_line_impl(&mut buf, line, &matches, &mut prev);
        }
        buf.push_str("\x1b[0m\x1b[K");
        prev = Style::default();
    }
    if rows >= 1 {
        buf.push_str(&format!("\x1b[{};1H", rows));
        buf.push_str(&status_line(v, view_h, width));
    }
    buf.push_str("\x1b[0m");
    buf
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (source, path) = read_source(&args)?;

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::cursor::Hide
    )?;
    stdout.flush()?;
    let _guard = TerminalGuard;

    let (w, h) = terminal::size()
        .map(|(w, h)| {
            let w = if w == 0 { 80 } else { w };
            let h = if h == 0 { 24 } else { h };
            (w, h)
        })
        .unwrap_or((80, 24));
    let mut width = w as usize;
    let mut rows = h as usize;

    let doc = parse::render_markdown(&source, width);
    let mut v = Viewer {
        source,
        path,
        doc,
        scroll: 0,
        search: None,
        prompt: None,
    };

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout);
    let mut running = true;

    while running {
        out.write_all(draw(&v, rows).as_bytes())?;
        out.flush()?;

        let ev = match event::read() {
            Ok(e) => e,
            Err(_) => break,
        };

        let view_h = rows.saturating_sub(1);

        match ev {
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                modifiers,
                ..
            }) => {
                if let Some(prompt) = &mut v.prompt {
                    match code {
                        KeyCode::Char(c) => {
                            prompt.buf.push(c);
                        }
                        KeyCode::Backspace => {
                            prompt.buf.pop();
                        }
                        KeyCode::Esc => {
                            v.prompt = None;
                        }
                        KeyCode::Enter => {
                            let result = (prompt.on_submit)(prompt);
                            v.prompt = None;
                            match result {
                                Some(q) => {
                                    if q.is_empty() {
                                        v.search = None;
                                    } else {
                                        v.search = Some(q);
                                        v.scroll = v.find_next(0, 1).unwrap_or(0);
                                        v.clamp(view_h);
                                    }
                                }
                                None => {
                                    if let Some(_) = &v.search {
                                        v.scroll = v.find_next(v.scroll, 1).unwrap_or(v.scroll);
                                        v.clamp(view_h);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                let quit = match code {
                    KeyCode::Char('q') | KeyCode::Esc => true,
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => true,
                    KeyCode::Char('j') | KeyCode::Down => {
                        v.scroll_by(1, view_h);
                        false
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        v.scroll_by(-1, view_h);
                        false
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') | KeyCode::Char('f')
                    | KeyCode::Char('d') => {
                        v.scroll_by(view_h as i64, view_h);
                        false
                    }
                    KeyCode::PageUp | KeyCode::Char('b') | KeyCode::Char('u') => {
                        v.scroll_by(-(view_h as i64), view_h);
                        false
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        v.go(0, view_h);
                        false
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        v.go(usize::MAX, view_h);
                        false
                    }
                    KeyCode::Char('/') => {
                        v.prompt = Some(Prompt {
                            label: "Search: ".into(),
                            buf: String::new(),
                            on_submit: |p: &mut Prompt| Some(p.buf.clone()),
                        });
                        false
                    }
                    KeyCode::Char('n') => {
                        v.scroll = v.find_next(v.scroll, 1).unwrap_or(v.scroll);
                        v.clamp(view_h);
                        false
                    }
                    KeyCode::Char('N') => {
                        v.scroll = v.find_next(v.scroll, -1).unwrap_or(v.scroll);
                        v.clamp(view_h);
                        false
                    }
                    KeyCode::Char('r') => {
                        if v.path.is_some() {
                            let width = v.doc.width;
                            if let Ok(src) = std::fs::read_to_string(v.path.clone().unwrap()) {
                                v.source = src;
                                v.reload(width, view_h);
                            }
                        }
                        false
                    }
                    _ => false,
                };
                if quit {
                    running = false;
                }
            }
            Event::Mouse(me) => match me.kind {
                MouseEventKind::ScrollUp => {
                    v.scroll_by(-3, view_h);
                }
                MouseEventKind::ScrollDown => {
                    v.scroll_by(3, view_h);
                }
                _ => {}
            },
            Event::Resize(w, h) => {
                width = w as usize;
                rows = h as usize;
                v.reload(width, rows.saturating_sub(1));
            }
            _ => {}
        }
    }

    drop(out);
    drop(_guard);
    Ok(())
}