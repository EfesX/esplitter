# esplitter

Fast and simple Rust utility for split big files into parts of a given size and then merge this parts into a big file again.

## 🚀 Features
- Splitting a big files into parts and specifying the size in bytes (`B`), kilobytes (`Kb`), megabytes (`Mb`) or gigabytes (`Gb`)
- Parts autonaming format `<filename>.part.<NNN>` (where `NNN` — the number of a part with leading zeros, `001`).
- Merge back into the origin file with name verification and automatic deletion of parts after merging.
- High I/O performance

## 🛠️ Building

```bash
cargo build --release
```
## 🛞 Examples
```bash
esplitter split --path bigfile.7z --size 4096 --units Mb
esplitter merge -p bigfile.7z
```


