This is a sample program for using Rust to spawn the Ruby debugger rdbg. It is to explore how to do debugger DAP integration for Zed and Helix.

This sample works but it requires that `stdin` is not null and seems to need something to be written to it.

## Build

```
git clone https://github.com/ascarter/rdbg-rs.git
cd rdbg-rs
cargo build
cargo run
```
