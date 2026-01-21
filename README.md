# image-rando

Simple Rust utility to copy all `.jpg` photos from a source folder into a set of numbered folders under a destination folder, in random order, with these constraints:

- **No more than 1200 photos per folder**
- **No more than 4 GiB per folder**

Defaults:

- Source: `/home/jef/Pictures/theframe`
- Destination: `/home/jef/Pictures/display` (creates `/home/jef/Pictures/display/1`, `/2`, ...)

## Run

Build + run (release recommended for speed):

```bash
cargo run --release
```

Optional flags:

```bash
cargo run --release -- --src /path/to/src --dst /path/to/dst --seed 123
```

The program will refuse to run if the destination folder is not empty (to avoid mixing old/new output).