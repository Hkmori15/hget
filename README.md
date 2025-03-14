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
hget [options] [url]
```
- For help use:
```bash
hget --help
```
- For output file:
```bash
hget https://example.com/target.zip --output current_file.zip
```
- For redirects:
```bash
hget --max-redirects or -r 5 https://example.com/
```

Default amount redirects is 10

- For continue interrupt download:
```bash
hget --continue or -c https://example.com/target.zip
```
- For force overwriting existing file:
```bash
hget --force or -f https://example.com/target.zip
```
- For recursive download:
```bash
hget --recursive or -R --max_depth or -l 3 --max_concurrent or -j 3 --same_domain or -d https://example.com/target.zip
```
1. **--recursive or -R** - download recursively
2. **--max_depth or -l <amount>** - max depth for files. Default depth - 5
3. **--max_concurrent or -j <amount>** - concurrent for more speed downloading. Default concurrent - 5
4. **--same_domain or -d** - download files only from this domain 
