use crate::render::{char_display_w, display_width, Color, Line, Style, Styled};
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

fn heading_style(level: HeadingLevel) -> Style {
    let fg = match level {
        HeadingLevel::H1 | HeadingLevel::H2 => Color::BrightCyan,
        _ => Color::BrightBlue,
    };
    Style {
        fg,
        bg: Color::Reset,
        bold: level <= HeadingLevel::H3,
        italic: false,
        underline: level <= HeadingLevel::H2,
        strikethrough: false,
    }
}

fn code_style() -> Style {
    Style {
        fg: Color::BrightGreen,
        bg: Color::Reset,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
    }
}

fn inline_code_style() -> Style {
    Style {
        fg: Color::BrightGreen,
        bg: Color::Reset,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
    }
}

fn link_style() -> Style {
    Style {
        fg: Color::BrightBlue,
        bg: Color::Reset,
        bold: false,
        italic: false,
        underline: true,
        strikethrough: false,
    }
}

fn plain() -> Style {
    Style::default()
}

/// The rendered document.
#[derive(Debug)]
pub struct Document {
    pub lines: Vec<Line>,
}

/// Accumulates inline (paragraph/heading/list-item) content before wrapping.
#[derive(Default)]
struct Block {
    runs: Vec<Styled>,
}

impl Block {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.runs.last_mut() {
            if last.style == style {
                last.text.push_str(text);
                return;
            }
        }
        self.runs.push(Styled::new(text, style));
    }
}

/// Wrap styled runs into lines of at most `width`, breaking on whitespace.
/// Words wider than `width` are split.
fn wrap_runs(runs: &[Styled], width: usize) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut cur: Vec<Styled> = Vec::new();
    let mut cur_w = 0usize;
    let mut space: Vec<Styled> = Vec::new();
    let mut space_w = 0usize;

    for run in runs {
        for ch in run.text.chars() {
            let cw = char_display_w(ch);
            if ch == ' ' || ch == '\t' {
                space.push(Styled::new(&ch.to_string(), run.style));
                space_w += cw;
                continue;
            }
            if cur_w + space_w + cw > width && !cur.is_empty() {
                lines.push(Line {
                    runs: std::mem::take(&mut cur),
                });
                cur_w = 0;
                space.clear();
                space_w = 0;
            }
            if cur_w + cw > width && !cur.is_empty() {
                lines.push(Line {
                    runs: std::mem::take(&mut cur),
                });
                cur_w = 0;
            }
            for s in space.drain(..) {
                cur.push(s);
            }
            cur_w += space_w;
            space_w = 0;
            cur.push(Styled::new(&ch.to_string(), run.style));
            cur_w += cw;
        }
    }
    for s in space.drain(..) {
        cur.push(s);
    }
    if !cur.is_empty() {
        lines.push(Line {
            runs: std::mem::take(&mut cur),
        });
    }
    if lines.is_empty() {
        lines.push(Line { runs: vec![] });
    }
    lines
}

/// Parse markdown into styled display lines wrapped to `width`.
pub fn render_markdown(source: &str, width: usize) -> Document {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(source, opts);

    let mut out: Vec<Line> = Vec::new();
    let mut block = Block::default();
    let mut block_marker: Option<String> = None;
    let mut block_indent = 0usize;
    let mut para_styles: Vec<Style> = Vec::new();

    // list state
    let mut list_kind: Vec<bool> = Vec::new(); // true = ordered
    let mut list_counter: Vec<u32> = Vec::new();

    // heading/code
    let mut head_style: Option<Style> = None;
    let mut in_code = false;
    let mut code_buf = String::new();

    // table state
    let mut in_table = false;
    let mut table_aligns: Vec<Alignment> = Vec::new();
    let mut table_rows: Vec<Vec<Vec<Styled>>> = Vec::new();
    let mut table_cur_row: Vec<Vec<Styled>> = Vec::new();

    fn flush_inline(
        out: &mut Vec<Line>,
        block: &mut Block,
        width: usize,
        marker: &mut Option<String>,
        indent: usize,
    ) {
        let runs = std::mem::take(&mut block.runs);
        if runs.is_empty() {
            return;
        }
        let wrap_w = width.saturating_sub(indent + marker.as_ref().map_or(0, |m| display_width(m)));
        let lines = wrap_runs(&runs, wrap_w);
        let mut first = true;
        for mut ln in lines {
            if let Some(m) = marker.take() {
                let mut prefix = Vec::new();
                for ch in m.chars() {
                    prefix.push(Styled::new(&ch.to_string(), plain()));
                }
                for p in prefix.into_iter().rev() {
                    ln.runs.insert(0, p);
                }
            }
            if !first {
                let pad = " ".repeat(indent);
                ln.runs.insert(0, Styled::new(&pad, plain()));
            }
            out.push(ln);
            first = false;
        }
    }

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    head_style = Some(heading_style(level));
                }
                Tag::CodeBlock(_) => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    in_code = true;
                    code_buf.clear();
                }
                Tag::List(start) => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    list_kind.push(start.is_some());
                    list_counter.push(1);
                }
                Tag::Item => {
                    let ordered = list_kind.last().copied().unwrap_or(false);
                    if ordered {
                        let n = list_counter.last().copied().unwrap_or(1);
                        if let Some(c) = list_counter.last_mut() {
                            *c += 1;
                        }
                        block_marker = Some(format!("{n}. "));
                    } else {
                        block_marker = Some("• ".to_string());
                    }
                    block_indent = list_kind.len().saturating_sub(1) * 2 + 2;
                }
                Tag::Paragraph => {}
                Tag::Table(aligns) => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    in_table = true;
                    table_aligns = aligns;
                    table_rows.clear();
                }
                Tag::TableHead => {}
                Tag::TableRow => {
                    table_cur_row.clear();
                }
                Tag::TableCell => {
                    table_cur_row.push(Vec::new());
                }
                Tag::Emphasis => para_styles.push(st_italic()),
                Tag::Strong => para_styles.push(st_bold()),
                Tag::Strikethrough => para_styles.push(st_strike()),
                Tag::Link { .. } => para_styles.push(link_style()),
                Tag::Image { .. } => para_styles.push(st_image()),
                Tag::FootnoteDefinition(_) | Tag::BlockQuote(_) => {}
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    head_style = None;
                    out.push(Line { runs: vec![] });
                }
                TagEnd::CodeBlock => {
                    in_code = false;
                    for ln in code_buf.trim_end_matches('\n').split('\n') {
                        out.push(Line {
                            runs: vec![Styled::new(ln.trim_end_matches(' '), code_style())],
                        });
                    }
                    out.push(Line { runs: vec![] });
                    code_buf.clear();
                }
                TagEnd::List(_) => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    list_kind.pop();
                    list_counter.pop();
                    block_indent = list_kind.len().saturating_mul(2);
                    out.push(Line { runs: vec![] });
                }
                TagEnd::Item => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                }
                TagEnd::Paragraph => {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                    out.push(Line { runs: vec![] });
                }
                TagEnd::Table => {
                    in_table = false;
                    prune_table_rows(&mut table_rows);
                    render_table(&table_rows, &table_aligns, width, &mut out);
                    out.push(Line { runs: vec![] });
                }
                TagEnd::TableHead => {
                    if !table_cur_row.is_empty() {
                        table_rows.push(std::mem::take(&mut table_cur_row));
                    }
                }
                TagEnd::TableRow => {
                    if !table_cur_row.is_empty() {
                        table_rows.push(std::mem::take(&mut table_cur_row));
                    }
                }
                TagEnd::TableCell => {}
                TagEnd::Emphasis => {
                    para_styles.pop();
                }
                TagEnd::Strong => {
                    para_styles.pop();
                }
                TagEnd::Strikethrough => {
                    para_styles.pop();
                }
                TagEnd::Link => {
                    para_styles.pop();
                }
                TagEnd::Image => {
                    para_styles.pop();
                }
                _ => {}
            },
            Event::Code(text) => {
                if in_code {
                    code_buf.push_str(&text);
                    continue;
                }
                let style = inline_code_style();
                if in_table {
                    if let Some(cell) = table_cur_row.last_mut() {
                        cell.push(Styled::new(&text, style));
                    }
                } else {
                    block.push(&text, style);
                }
            }
            Event::Text(text) => {
                if in_code {
                    code_buf.push_str(&text);
                    continue;
                }
                let style = current_inline(&para_styles, head_style);
                if in_table {
                    if let Some(cell) = table_cur_row.last_mut() {
                        cell.push(Styled::new(&text, style));
                    }
                } else {
                    block.push(&text, style);
                }
            }
            Event::SoftBreak => {
                if !in_table && !in_code {
                    block.push(" ", plain());
                }
            }
            Event::HardBreak => {
                if !in_table && !in_code {
                    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                }
            }
            Event::Rule => {
                flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);
                let spans = width.saturating_sub(2).max(1);
                let rule = "─".repeat(spans);
                out.push(Line {
                    runs: vec![Styled::new(
                        &format!("  {rule}"),
                        Style {
                            fg: Color::BrightBlack,
                            bg: Color::Reset,
                            ..plain()
                        },
                    )],
                });
                out.push(Line { runs: vec![] });
            }
            Event::TaskListMarker(checked) => {
                block.push(if checked { "[x] " } else { "[ ] " }, plain());
            }
            Event::InlineMath(s) | Event::DisplayMath(s) => {
                block.push("$", inline_code_style());
                block.push(&s, inline_code_style());
                block.push("$", inline_code_style());
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                block.push(&h, st_html());
            }
            Event::FootnoteReference(_) => {}
        }
    }

    flush_inline(&mut out, &mut block, width, &mut block_marker, block_indent);

    Document { lines: out }
}

fn prune_table_rows(rows: &mut Vec<Vec<Vec<Styled>>>) {
    while let Some(last) = rows.last() {
        if last.iter().all(|c| c.is_empty()) {
            rows.pop();
        } else {
            break;
        }
    }
}

fn st_bold() -> Style {
    Style {
        fg: Color::Unset,
        bg: Color::Reset,
        bold: true,
        italic: false,
        underline: false,
        strikethrough: false,
    }
}

fn st_italic() -> Style {
    Style {
        fg: Color::Unset,
        bg: Color::Reset,
        bold: false,
        italic: true,
        underline: false,
        strikethrough: false,
    }
}

fn st_strike() -> Style {
    Style {
        fg: Color::Unset,
        bg: Color::Reset,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: true,
    }
}

fn st_image() -> Style {
    Style {
        fg: Color::BrightMagenta,
        bg: Color::Reset,
        bold: false,
        italic: true,
        underline: true,
        strikethrough: false,
    }
}

fn st_html() -> Style {
    Style {
        fg: Color::BrightBlack,
        bg: Color::Reset,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
    }
}

fn current_inline(para_styles: &[Style], head_style: Option<Style>) -> Style {
    let mut s = head_style.unwrap_or_default();
    for st in para_styles {
        match st.fg {
            Color::Unset => {}
            c => s.fg = c,
        }
        s.bold |= st.bold;
        s.italic |= st.italic;
        s.underline |= st.underline;
        s.strikethrough |= st.strikethrough;
    }
    s
}

/// Render a table. `rows[0]` is the header row.
fn render_table(rows: &[Vec<Vec<Styled>>], aligns: &[Alignment], width: usize, out: &mut Vec<Line>) {
    if rows.is_empty() {
        return;
    }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols == 0 {
        return;
    }

    // per-column widths
    let mut col_w = vec![0usize; ncols];
    for row in rows {
        for (ci, cell) in row.iter().enumerate() {
            let w: usize = cell.iter().map(|r| display_width(&r.text)).sum();
            if w > col_w[ci] {
                col_w[ci] = w;
            }
        }
    }

    // Each cell gets one space of padding left and right.
    for w in col_w.iter_mut() {
        *w += 2;
    }

    // Table visual width = sum(col_w) + ncols + 1 (vertical separators).
    let border_w = ncols + 1;
    let avail = width.saturating_sub(border_w);
    let total: usize = col_w.iter().sum();
    if total > avail {
        shrink_columns(&mut col_w, avail);
    }

    let header_style = Style {
        fg: Color::BrightYellow,
        bg: Color::Reset,
        bold: true,
        italic: false,
        underline: false,
        strikethrough: false,
    };
    let sepc = Style {
        fg: Color::BrightBlack,
        bg: Color::Reset,
        ..plain()
    };

    let top = table_border('┌', '┬', '┐', '─', &col_w);
    let header_div = table_border('╞', '╪', '╡', '═', &col_w);
    let row_div = table_border('├', '┼', '┤', '─', &col_w);
    let bottom = table_border('└', '┴', '┘', '─', &col_w);

    out.push(Line {
        runs: vec![Styled::new(&top, sepc)],
    });

    for (ri, row) in rows.iter().enumerate() {
        let is_header = ri == 0;
        let mut lruns: Vec<Styled> = Vec::new();
        lruns.push(Styled::new("│", sepc));
        for (ci, &cw) in col_w.iter().enumerate() {
            let inner = cw.saturating_sub(2).max(1);
            let cell = row.get(ci).cloned().unwrap_or_default();
            let mut packed = pack_cell(&cell, inner);
            if is_header {
                for r in packed.iter_mut() {
                    r.style.bold = true;
                    if r.style.fg == Color::Reset || r.style.fg == Color::Unset {
                        r.style.fg = header_style.fg;
                    }
                }
            }
            let content_w: usize = packed.iter().map(|r| display_width(&r.text)).sum();
            let pad = inner.saturating_sub(content_w);
            let align = aligns.get(ci).copied().unwrap_or(Alignment::None);
            let (pl, pr) = match align {
                Alignment::Right => (pad, 0),
                Alignment::Center => (pad / 2, pad - pad / 2),
                _ => (0, pad),
            };
            lruns.push(Styled::new(" ", plain()));
            lruns.push(Styled::new(&" ".repeat(pl), plain()));
            lruns.extend(packed);
            lruns.push(Styled::new(&" ".repeat(pr), plain()));
            lruns.push(Styled::new(" ", plain()));
            lruns.push(Styled::new("│", sepc));
        }
        out.push(Line { runs: lruns });
        if is_header {
            out.push(Line {
                runs: vec![Styled::new(&header_div, sepc)],
            });
        } else if ri < rows.len() - 1 {
            out.push(Line {
                runs: vec![Styled::new(&row_div, sepc)],
            });
        }
    }
    out.push(Line {
        runs: vec![Styled::new(&bottom, sepc)],
    });
}

/// Truncate/spill a cell's runs to fit exactly `cw` display columns.
fn pack_cell(cell: &[Styled], cw: usize) -> Vec<Styled> {
    let mut out: Vec<Styled> = Vec::new();
    let mut w = 0usize;
    'outer: for run in cell {
        for ch in run.text.chars() {
            let cwch = char_display_w(ch);
            if w + cwch > cw {
                break 'outer;
            }
            if let Some(last) = out.last_mut() {
                if last.style == run.style {
                    last.text.push(ch);
                    w += cwch;
                    continue;
                }
            }
            out.push(Styled::new(&ch.to_string(), run.style));
            w += cwch;
        }
    }
    out
}

fn table_border(left: char, junction: char, right: char, fill: char, col_w: &[usize]) -> String {
    let mut s = String::new();
    s.push(left);
    for (i, w) in col_w.iter().enumerate() {
        s.extend(std::iter::repeat_n(fill, *w));
        if i < col_w.len() - 1 {
            s.push(junction);
        }
    }
    s.push(right);
    s
}

/// Reduce column widths until their sum is <= `avail`, shrinking the widest
/// columns first.
fn shrink_columns(col_w: &mut [usize], avail: usize) {
    loop {
        let total: usize = col_w.iter().sum();
        if total <= avail || col_w.iter().all(|&w| w == 0) {
            break;
        }
        if let Some(maxi) = col_w
            .iter()
            .enumerate()
            .filter(|(_, &w)| w > 0)
            .max_by_key(|(_, &w)| w)
            .map(|(i, _)| i)
        {
            col_w[maxi] -= 1;
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(line: &Line) -> String {
        line.runs.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn renders_table_with_borders() {
        let md = "| A | B |\n|---|---|\n| 1 | 22 |\n| 333 | 4 |\n";
        let doc = render_markdown(md, 60);
        let lines: Vec<String> = doc.lines.iter().map(plain).collect();
        assert!(lines.iter().any(|l| l.starts_with("┌")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("╞")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("├")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("└")), "{lines:?}");
        // header text must be present
        assert!(lines.iter().any(|l| l.contains("A") && l.contains("B")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("333")), "{lines:?}");
        // every output row must fit the terminal width
        for l in &lines {
            assert!(display_width(l) <= 60, "too wide: {l:?}");
        }
    }

    #[test]
    fn shrinks_wide_table_to_fit_terminal() {
        let md = "| ColOne | ColTwo | ColThree |\n|---|---|---|\n| a | bbb | ccccc |\n";
        let doc = render_markdown(md, 20);
        for l in &doc.lines {
            let p = plain(l);
            assert!(display_width(&p) <= 20, "too wide: {p:?}");
        }
    }

    #[test]
    fn table_rows_have_vertical_borders() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let doc = render_markdown(md, 60);
        let lines: Vec<String> = doc.lines.iter().map(plain).collect();
        let row = lines.iter().find(|l| l.contains("A") && l.contains("B")).unwrap();
        assert!(row.starts_with("│"), "{row}");
        assert!(row.ends_with("│"), "{row}");
        let body = lines.iter().find(|l| l.contains('1') && l.contains('2')).unwrap();
        assert!(body.starts_with("│ ") && body.ends_with(" │"), "{body}");
        // double-line header divider
        assert!(
            lines.iter().any(|l| l.starts_with('╞') && l.ends_with('╡')),
            "{lines:?}"
        );
    }

    #[test]
    fn wraps_paragraphs_to_width() {
        let md = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod.";
        let doc = render_markdown(md, 24);
        assert!(doc.lines.len() > 1, "expected wrapping");
        for l in &doc.lines {
            assert!(display_width(&plain(l)) <= 24, "line too wide");
        }
    }

    #[test]
    fn styles_headings_differently() {
        let md = "# Big\n\n## Medium\n\n### Small\n";
        let doc = render_markdown(md, 60);
        let h1 = doc.lines.iter().find(|l| plain(l) == "Big").unwrap();
        let h2 = doc.lines.iter().find(|l| plain(l) == "Medium").unwrap();
        let h3 = doc.lines.iter().find(|l| plain(l) == "Small").unwrap();
        assert_eq!(h1.runs[0].style.fg, Color::BrightCyan);
        assert_eq!(h2.runs[0].style.fg, Color::BrightCyan);
        assert_eq!(h3.runs[0].style.fg, Color::BrightBlue);
        assert!(h1.runs[0].style.bold);
        assert!(h1.runs[0].style.underline);
        assert!(!h3.runs[0].style.underline);
    }

    #[test]
    fn handles_ordered_and_bullet_lists() {
        let md = "- alpha\n- beta\n\n1. first\n2. second\n";
        let doc = render_markdown(md, 60);
        let lines: Vec<String> = doc.lines.iter().map(plain).collect();
        assert!(lines.iter().any(|l| l.contains("• alpha")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("1. first")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("2. second")), "{lines:?}");
    }
}