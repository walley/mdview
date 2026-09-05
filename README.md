# mdview

A tiny console markdown viewer written in Rust. It renders a markdown file to
your terminal — colored headings, real bold/italic, bordered tables, and text
that wraps to fit the width of your window.

```console
$ mdview README.md
```

```
mdview Test Document
...
┌──────────┬────────────┬────────┬──────────────────┐
│ Name     │ Language   │  Stars │ Notes            │
╞══════════╪════════════╪════════╪══════════════════╡
│ mdview   │ Rust       │    133 │ A console viewer │
├──────────┼────────────┼────────┼──────────────────┤
│ opencode │ TypeScript │    415 │ coding agent     │
└──────────┴────────────┴────────┴──────────────────┘
```

## Features

- **Tables** — box-drawing borders, column alignment (left / right / center),
  a double-line divider under the header, and a separator between every row.
- **Styled headings** — H1/H2 in bright cyan (bold + underline), H3+ in blue.
- **Real markup** — bold, *italic*, `inline code`, <u>links</u>, code blocks,
  and strikethrough, emitted as ANSI only when your terminal (and font)
  support them.
- **Lists** — bullets, ordered lists, nesting, and `[x]`/`[ ]` task markers.
- **Width aware** — paragraphs wrap to your terminal width; wide tables shrink
  to fit instead of running off the screen.

## Install

```console
$ cargo build --release
# binary lands in target/release/mdview
```

Requires Rust 1.82+.

## Usage

```console
mdview [OPTIONS] [FILE]
mdview [OPTIONS] -          # read markdown from stdin
mdview < file.md            # same, when stdin is piped
```

- With **no file and no piped input**, `mdview` prints its help.
- With a **file argument**, renders that file.
- With **piped stdin** (or `-`), renders its input.

### Options

| Option | Description |
| --- | --- |
| `-b`, `--no-colors` | Disable ANSI colors entirely |
| `--color` | Force colors even when stdout is piped |
| `-w`, `--width <COLS>` | Set output width (default: terminal width) |
| `-h`, `--help` | Show help |

Colors are emitted by default only when stdout is a terminal. Setting the
`NO_COLOR` environment variable (as per [no-color.org](https://no-color.org))
disables them in a terminal too.

## Examples

```console
# Render a file
mdview CHANGELOG.md

# Render stdin
cat notes.md | mdview
mdview - < notes.md

# Plain output, for piping elsewhere
mdview --no-colors notes.md | wc -l

# Fix the width when not attached to a terminal
git log -1 --format=%B | mdview -w 100
```

## How it works

Markdown is parsed with [`pulldown-cmark`](https://github.com/pulldown-cmark/pulldown-cmark),
then rendered into styled runs that are wrapped and composed into the final
output. `mdview` reads your terminal size with the `TIOCGWINSZ` ioctl (or the
`COLUMNS` variable) so every line fits the window.

## License

[GNU Affero General Public License v3.0](LICENSE)