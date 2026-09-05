# mdview Test Document

This is a console markdown viewer written in Rust. It renders **bold** text,
*italic* text, `inline code`, and [links](https://example.com).

## Tables

Tables are supported with column alignment:

| Name      | Language     | Stars  | Notes |
|:----------|:-------------|-------:|-------|
| mdview    | Rust         |    133 | A console viewer |
| opencode  | TypeScript   |    415 | coding agent |
| bat       | Rust         | 49,362 | cat with wings |

## Lists

- first bullet
- second bullet
  - nested bullet
  1. nested ordered
  2. second nested

1. one
2. two

## Code

```rust
fn main() {
    println!("Hello, world!");
}
```

## Wrap test

This is a fairly long paragraph that should wrap around to fit the width of
the terminal. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim
veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
commodo consequat.

---

- [ ] todo item
- [x] done item