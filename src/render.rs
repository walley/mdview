/// ANSI color names supported for styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum Color {
    #[default]
    Reset,
    Unset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightBlack,
    BrightWhite,
    Rgb(u8, u8, u8),
}

impl Color {
    fn fg_ansi(self) -> String {
        match self {
            Color::Black => "30".into(),
            Color::Red => "31".into(),
            Color::Green => "32".into(),
            Color::Yellow => "33".into(),
            Color::Blue => "34".into(),
            Color::Magenta => "35".into(),
            Color::Cyan => "36".into(),
            Color::White => "37".into(),
            Color::BrightBlack => "90".into(),
            Color::BrightRed => "91".into(),
            Color::BrightGreen => "92".into(),
            Color::BrightYellow => "93".into(),
            Color::BrightBlue => "94".into(),
            Color::BrightMagenta => "95".into(),
            Color::BrightCyan => "96".into(),
            Color::BrightWhite => "97".into(),
            Color::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
            Color::Reset => "39".into(),
            Color::Unset => "39".into(),
        }
    }

    fn bg_ansi(self) -> String {
        match self {
            Color::Rgb(r, g, b) => format!("48;2;{r};{g};{b}"),
            _ => format!("4{}", to_bg_code(self)),
        }
    }
}

fn to_bg_code(c: Color) -> u8 {
    match c {
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::White => 7,
        Color::BrightBlack => 8,
        Color::BrightRed => 9,
        Color::BrightGreen => 10,
        Color::BrightYellow => 11,
        Color::BrightBlue => 12,
        Color::BrightMagenta => 13,
        Color::BrightCyan => 14,
        Color::BrightWhite => 15,
        _ => 0,
    }
}

/// A text style: foreground, background, weight variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Style {
    /// Escape sequence to switch from `prev` to this style (empty if unchanged).
    pub fn to_ansi(self, prev: Style) -> String {
        if self == prev {
            return String::new();
        }
        let mut parts = Vec::new();
        if self.fg != prev.fg {
            parts.push(self.fg.fg_ansi());
        }
        if self.bg != prev.bg {
            parts.push(self.bg.bg_ansi());
        }
        if self.bold != prev.bold {
            parts.push(if self.bold { "1".into() } else { "22".into() });
        }
        if self.italic != prev.italic {
            parts.push(if self.italic { "3".into() } else { "23".into() });
        }
        if self.underline != prev.underline {
            parts.push(if self.underline { "4".into() } else { "24".into() });
        }
        if self.strikethrough != prev.strikethrough {
            parts.push(if self.strikethrough {
                "9".into()
            } else {
                "29".into()
            });
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", parts.join(";"))
        }
    }
}

/// A run of text with a single style.
#[derive(Debug, Clone, PartialEq)]
pub struct Styled {
    pub text: String,
    pub style: Style,
}

impl Styled {
    pub fn new(text: &str, style: Style) -> Styled {
        Styled {
            text: text.to_string(),
            style,
        }
    }
}

/// One display line: a list of styled runs.
#[derive(Debug, Clone)]
pub struct Line {
    pub runs: Vec<Styled>,
}

/// Compute the display width of a string, counting wide (CJK, emoji) chars as 2.
pub fn display_width(s: &str) -> usize {
    s.chars().map(char_w).sum()
}

pub fn char_display_w(c: char) -> usize {
    char_w(c)
}

fn char_w(c: char) -> usize {
    if c == '\t' {
        4
    } else {
        unicode_width(c)
    }
}

fn unicode_width(c: char) -> usize {
    let r = c as u32;
    if (0x1100..=0x115F).contains(&r)
        || (0x2E80..=0xA4CF).contains(&r)
        || (0xAC00..=0xD7A3).contains(&r)
        || (0xF900..=0xFAFF).contains(&r)
        || (0xFE30..=0xFE4F).contains(&r)
        || (0xFF00..=0xFF60).contains(&r)
        || (0xFFE0..=0xFFE6).contains(&r)
        || (0x1F300..=0x1F64F).contains(&r)
        || (0x1F680..=0x1F6FF).contains(&r)
        || (0x1F900..=0x1F9FF).contains(&r)
        || (0x20000..=0x2FFFD).contains(&r)
        || (0x30000..=0x3FFFD).contains(&r)
    {
        2
    } else {
        1
    }
}