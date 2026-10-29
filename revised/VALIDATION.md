# Version 0.3.1 rebuild

Rebuilt on 30 October 2026 with the user-edited FIRST and LAST constants copied exactly from the original source into the revised generator. New datasets use these name pools. Existing v0.3.0 checkpoints retain the original pools so resumed content remains consistent. Release tests: 23 passed, including the legacy checkpoint name compatibility check. Formatting and release Clippy passed.

# Version 0.3.0 verification

Verified on 30 October 2026 on the same Windows 11 x64 workstation with Rust 1.99.1.

- Release tests: 22 passed, 0 failed. The two large disk-writing benchmarks remain ignored by default.
- Release Clippy with `--all-targets -- -D warnings` and `cargo fmt --check` passed.
- Ten new fault-injection tests cover transient recovery, retry exhaustion, interruptible waiting, non-transient errors, serialised pause/resume, lost write acknowledgements, partial temporary files, pending inventory publication, corrupt checkpoint fallback, uncommitted journal tails, conflicting existing files and exclusive job locking.
- Resumed sample datasets were compared byte for byte, including inventory, against uninterrupted runs across a partial 15-file size cycle.
- GUI checks exercise collapse and reopen of Configuration while Generate remains rendered, plus the existing width, validation and F1 checks.
- A new 60-file sample again passed the Python structure/format checks and Microsoft Open XML SDK validation for all 30 Office packages.
- One current 288,000,000-byte local sample completed in 1.713167 seconds (168.110 MB/s). This is a single non-comparative observation, not a new speed improvement claim. Per-file destination flushes and durable local checkpoints add I/O; network throughput depends on SMB/server latency. The older three-run figures below describe version 0.2.0 only.

Outages were simulated through injected destination I/O errors. Actual network disconnection/reconnection, a machine reboot during generation, SMB server failover and remote filesystem durability were not exercised. Windows may block inside an OS file call before the retry logic receives an error. The retry window therefore does not impose a hard timeout on outstanding OS calls. Resume checks the pending file but does not audit previously completed files for later external changes.

# Version 0.2.0 verification record (historical)

Verified on 29 October 2026 on this Windows 11 x64 workstation, using Rust 1.99.1 and Cargo 1.99.1. The application targets Rust 1.95 or newer.

## Build and checks

- Release build succeeded with the MSVC C runtime statically linked.
- `cargo test --release --locked`: 11 passed, 0 failed; the two disk-writing benchmarks are ignored during ordinary tests.
- `cargo clippy --release --locked --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- Automated layout checks passed at 560, 620 and 1040 logical pixels wide. They assert no horizontal overflow and that Generate remains visible at 640 pixels high.
- F1 opened help in the native portable GUI; the keyboard shortcut also passed its automated check.
- The native GUI was visually inspected for the light background, readable cards, wrapping text and two-column settings layout.
- PE import-table inspection confirmed an x64 Windows GUI executable with no external VCRUNTIME, MSVCP, CONCRT or api-ms-win-crt imports. Windows system libraries, including OpenGL and UI Automation, remain required.
- The PowerShell build/package path was exercised, including licence collection, ZIP creation and SHA-256 output.

Tests cover all six formats at 0.1, 1 and 15 MB; ZIP entry CRC reads; size-cycle balancing for every count from 1 to 15 at five averages including bounds; zero-seed repeatability; full dataset byte/path/inventory repeatability; exact totals; folder depths; inventory disabled; cancellation after three completed files; invalid configuration; blocked output paths; existing dataset preservation; GUI seed/average validation; F1; and responsive layout.

## Output validity

The representative revised sample has 60 content files and exactly 288,000,000 bytes, plus the inventory. Every enabled type appears ten times.

- `tools/validate_output.py`: all 60 files passed inventory coverage, relative paths, sizes, folder depth, metadata presence, XML parsing, package relationships, ZIP CRCs, CSV column consistency and RTF group checks.
- Microsoft Open XML SDK 3.3.0: all 30 DOCX/XLSX/PPTX packages passed schema validation with zero errors.
- The Office templates also correct inherited duplicate PowerPoint shape IDs, invalid layout IDs, missing theme font elements and mismatched slide geometry.

## Throughput

Both implementations were compiled in release mode with the same toolchain and static-runtime configuration. Each measurement wrote 60 files using all six formats, seed 4242, maximum depth 7 and inventory enabled to a new local folder. Defaults were 1 MB minimum, 4.8 MB average and 15 MB maximum. Three alternating original/revised pairs were run. Process startup and compilation are excluded; content creation, folders, writes, progress callbacks and inventory completion are included.

| Run | Original seconds | Original MB/s | Revised seconds | Revised MB/s |
| --- | ---: | ---: | ---: | ---: |
| 1 | 5.638862 | 51.074 | 3.873128 | 74.358 |
| 2 | 5.221959 | 55.152 | 1.991019 | 144.650 |
| 3 | 3.806233 | 75.666 | 2.483883 | 115.947 |
| Median | 5.221959 | 55.152 | 2.483883 | 115.947 |

Median measured throughput increased approximately 2.10 times. Raw harness output is in [validation/benchmark.txt](validation/benchmark.txt). An earlier exploratory baseline measured 81.392 MB/s; it was excluded from the paired comparison because it preceded the static-runtime build.

The original wrote 288,001,646 bytes because its text padding overshot targets. The revision wrote exactly 288,000,000 bytes. The old implementation chose formats randomly, while the revision balances selected formats. Output contents and paths are therefore not byte-identical between implementations. The comparison represents the same requested workload, not an isolated compression microbenchmark.

These are observations on a workstation in use. Filesystem caching, antivirus activity, other work and destination storage affect the result. Writes are measured when the filesystem accepts them; neither version calls `sync_all` to force durable media flushes. These rates are not a guarantee for network shares or other machines.

## Performance changes

The original Office path compressed all XML twice, constructed a separate full-size random payload byte by byte, then copied that payload into a second complete archive. Text padding repeatedly selected and formatted individual sentences; CSV padding introduced inconsistent columns. Inventory accumulated as one large string and directory creation repeated for previously visited paths.

The revision stores small XML parts directly in one preallocated ZIP buffer, calculates fixed stored-entry overhead, and writes a reusable seeded 64 KiB payload block. Bulk text appends replace per-sentence formatting. Format-aware closing bytes and spaces make every file's size exact. Inventory rows stream through a buffered writer, known directories are cached, and GUI progress messages are limited to roughly ten per second. Cancellation checks occur between complete files.

## Rerun the benchmarks

From the revised project folder, choose new output paths for each invocation:

```powershell
$env:MIPOINT_BENCH_OUTPUT = 'C:\Temp\MiPoint-Baseline-01'
cargo test --release --locked --test baseline -- --ignored --nocapture
$env:MIPOINT_BENCH_OUTPUT = 'C:\Temp\MiPoint-Revised-01'
cargo test --release --locked --test generation revised_throughput -- --ignored --nocapture
```

Both harnesses refuse an existing parent path. Each run writes about 288 MB. Benchmark commands are developer tools; dataset generation for operators remains entirely in the GUI.

## Not validated

- Interactive opening/rendering in Microsoft Word, Excel or PowerPoint, which are not installed at the expected local application paths. Schema validation and package/CRC checks passed, but are not a substitute for testing every Office version.
- Deployment on a second clean PC, signing/SmartScreen reputation, ARM64 Windows or unsupported operating systems.
- Maximum-scale 100,000-file or 256 MB-per-file runs, disk-full recovery, abrupt power loss, and every network-share configuration.
- Compression/deduplication realism: padding deliberately repeats blocks/notes and does not model production file entropy.

Filesystem timestamps are real run times. Synthetic metadata is supplied in the inventory, not written as filesystem attributes or Office custom properties.
