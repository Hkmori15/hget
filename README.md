## hget

Simple and blazing-fast http(s) receiver data utility ⚡

## Building ⭐:

You can use 2 options:
1. Install **hget** widely-system:
```bash
cargo install --git https://github.com/Hkmori15/hget
```
2. Build from source:
```bash
git clone https://github.com/Hkmori15/hget.git
cd/z hget
cargo build --release
cargo install --path .
```

## Usage 🌟:
```bash
Usage: hget [OPTIONS] <URL>

Arguments:
  <URL>

Options:
  -o, --output <OUTPUT>
  -v, --verbose
  -r, --max-redirects <MAX_REDIRECTS>    [default: 10]
      --no-follow
  -c, --continue-download
  -f, --force
  -R, --recursive
  -l, --max-depth <MAX_DEPTH>            [default: 5]
  -j, --max-concurrent <MAX_CONCURRENT>  [default: 5]
  -d, --same-domain
  -h, --help                             Print help
  -V, --version                          Print version 
```
