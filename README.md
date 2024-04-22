# Rusty OBEGRÄNSAD
Display custom animations on IKEA's OBEGRÄNSAD using Rust on the Raspberry Pi Pico.

## Generating a UF2 binary
Ensure that Rust is up-to-date, target support for `thumbv6m-none-eabi` is provided, and elf2uf2-rs is installed:
```
rustup self update
rustup update stable
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs
```

Execute `cargo run --release` to generate the UF2 binary at `target/thumbv6m-none-eabi/release/rusty-obegraensad.uf2`.


## git setup

### GitHub noreply email
```
git config user.name "a-johanson"
git config user.email "a-johanson@users.noreply.github.com"
```

### GitHub tokens
```
git remote add origin https://a-johanson:<TOKEN>@github.com/a-johanson/rusty-obegraensad.git
git push -u origin master
```
