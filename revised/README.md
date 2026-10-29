# MiPoint Dummy Migration Set Generator

A Windows GUI for creating realistic, repeatable synthetic business datasets for migration testing. All organisations, identities, records, figures and classification labels are fictional test data.

## Launch without a command prompt

1. Open `dist\portable` and double-click `mipoint-dummy-generator.exe`, or double-click `Launch.bat` in this project folder.
2. Browse to an output folder.
3. Choose the file count, folder depth, seed, file types and sizes.
4. Select **Generate dataset**. Use **Open output folder** to inspect the result.

Click **Configuration** to collapse or expand all settings. Progress, pause and resume controls remain visible. Press **F1** or select **Help (F1)** for detailed generation rules. The interface switches from two columns to stacked sections in narrower windows. Scroll vertically to reach all controls. Clipboard paste and Windows accessibility support are enabled.

Each run creates a unique `Dataset-<seed>-<run ID>` folder. Existing datasets are preserved. **Pause dataset** finishes the current file or interrupts retry waiting, then retains a local checkpoint. Closing during generation follows the same pause path before the window closes.

## Settings

| Setting | Behaviour |
| --- | --- |
| Output folder | Absolute local or UNC path. The app creates the parent folder if needed. |
| Files | 1 to 100,000 content files; inventory is additional. Default 50. |
| Maximum depth | 1 to 12 folders below the run folder, including the organisation. Default 7. |
| Random seed | Blank chooses a time-derived seed and displays it. Any unsigned 64-bit integer, including zero, is accepted. |
| Inventory | Enabled by default; one row per completed content file. |
| Network retry window | Default 120 seconds; editable from 5 to 3600 seconds. Retries every 5 seconds, then pauses. |
| Average size | Default 4.8 MB. |
| Minimum / maximum | Default 1 / 15 MB. Each size setting supports 0.1 to 256 MB. |

MB is decimal: 1 MB = 1,000,000 bytes. Values are rounded to the nearest byte. Minimum <= average <= maximum is required; an impossible average is explained and generation is disabled. The estimated total is file count x rounded average, excluding the small inventory.

Each shuffled cycle of up to 15 files, including the final partial cycle, totals exactly cycle count x requested average in bytes. A completed run therefore achieves the requested byte-rounded mean. Individual sizes stay within the selected bounds; bounds are not guaranteed to appear. When the average equals a bound, all files have that size. A stopped or failed run may have a different mean.

## Types and presets

Every type has an individual checkbox. Manual changes select **Custom**. Enabled extensions are listed below the checkboxes. Types are shuffled in balanced cycles, so all enabled types appear when the count permits.

| Preset | Enabled formats |
| --- | --- |
| Office core | DOCX, XLSX, PPTX |
| Full business mix | DOCX, XLSX, PPTX, CSV, RTF, TXT |
| Documents only | DOCX, RTF, TXT |
| Data and reports | XLSX, PPTX, CSV, TXT |

DOCX contains a business brief, XLSX an action register, PPTX a business update, CSV consistent five-column records, and RTF/TXT a brief with supporting notes. Legacy binary DOC/XLS/PPT formats are not emitted.

Office files are genuine Open XML packages. Their small XML parts are stored without compression. A declared, uncompressed binary package part repeats a seed-derived 64 KiB block to reach the target size. Text and RTF repeat synthetic notes; CSV adds a quoted supporting-comments field. ASCII spaces complete the final bytes. This keeps files valid and sizes exact without enormous visible Office documents. The output is intended for migration testing; its compression and deduplication ratios do not model real business files.

## Business hierarchy and repeatability

Business profiles link divisions, departments, teams and processes. At depth 7, paths can contain:

`Organisation / Division / Department / Team / Process / Financial year / Record category`

Shallower files stop earlier; deeper paths add working-record folders. Depth varies from min(3, maximum) to the maximum. Names combine a record ID, a title appropriate to the business area and format, and a review status. Owners are labelled synthetic.

The same seed and all the same settings in version 0.3.1 reproduce relative paths, file bytes, sizes and inventory. Run folder suffixes and filesystem creation/modification times may differ. Different versions need not reproduce earlier output. The app does not upload generated data.

## Inventory

`Structure Inventory.csv` uses UTF-8 with BOM for Excel. Paths are relative to the run folder, with `/` separators. The five structure fields are `RelativePath`, `FileName`, `Extension`, `SizeBytes` and `FolderDepth`.

Ten synthetic metadata fields follow: `RecordID`, `BusinessUnit`, `Department`, `Team`, `BusinessProcess`, `FinancialYear`, `RecordCategory`, `Owner`, `Status` and `Classification`. Metadata is recorded in the CSV; it is not applied as filesystem attributes or Office custom properties. The inventory journal is saved locally after each complete file. Its CSV is published to the destination on completion or a normal pause when the destination is reachable. During an outage, the latest complete inventory remains in the local saved-job folder as `inventory-journal.csv`. Resuming publishes the network inventory without duplicate rows.

## Progress

The background worker reports completed files and bytes. The GUI shows elapsed time, MB/s and an estimated remaining time based on written bytes. ETA appears after work begins and varies with disk performance. Completion reports actual count, size, average and seed. A resumed session measures throughput from newly written bytes, excluding bytes from earlier sessions. Paused runs retain completed files and their local saved job.

## Network outages, pause and resume

Transient network failures are retried every 5 seconds for the selected retry window, defaulting to 120 seconds from the first returned error. The GUI reports attempts and elapsed retry time. If the share recovers, generation continues automatically. If the window expires, the job pauses. Permission problems, a full disk or conflicting existing content pause immediately so they can be corrected.

- In the same session, reconnect the destination and select **Resume paused dataset**.
- After restarting the app, select **Open saved dataset...**, then choose the matching saved-job folder under `%LOCALAPPDATA%\MiPoint Generator\Checkpoints`. The local folder name matches the output dataset folder name. Original settings and progress are restored automatically.
- Reconnect mapped drives with the same drive letter before resuming. UNC paths avoid drive-letter differences between sessions.
- Keep the saved-job folder until the dataset is complete. It contains two integrity-checked checkpoint slots, a local inventory journal and the job lock. Resume requires this local state, not just the destination files. Jobs are not removed automatically.

Each content file is written to a temporary sibling, flushed and then renamed into place. Only published files advance the local checkpoint. A lost write acknowledgement is handled by checking the expected bytes of that pending file. Differing existing content is preserved and the job pauses. A corrupt or torn checkpoint can fall back to the previous valid slot; an uncommitted inventory tail is discarded on resume. Only one worker can run a saved job at a time.

Windows file operations may block on OS/SMB timeouts before returning an error. The app cannot interrupt an outstanding Windows call; **Pause dataset** interrupts retry waiting and waits for any outstanding call/current file. The configured window is a retry budget, not a hard wall-clock timeout for a stalled OS call.

Previously generated version 0.2.0 datasets have no saved checkpoints and cannot be resumed with this feature. Resume assumes the destination retains previously acknowledged files; it does not audit every completed file for subsequent external deletion or modification. Version 0.3.1 uses the updated FIRST and LAST name lists for new datasets. Saved v0.3.0 jobs retain their original name pools when resumed, preserving their existing content sequence.

## Build and package on Windows

Build prerequisites: Windows 11 x64, Rust 1.95 or newer with the `x86_64-pc-windows-msvc` toolchain, Visual Studio C++ Build Tools and the Windows SDK. The first build needs internet access to retrieve locked dependencies. Windows PowerShell 5.1 or newer is sufficient for the build scripts.

- Double-click **Build and Run.bat** to build, package and launch.
- Double-click **Build Portable.bat** to build, run the release tests and package.
- Subsequent operation uses only the portable executable or **Launch.bat**.

Run source build scripts from the source project, rather than from an extracted portable package. The scripts use their own project directory, honour `Cargo.lock`, statically link the MSVC C runtime through `.cargo\config.toml`, collect dependency licence notices, and create:

- `dist\portable\mipoint-dummy-generator.exe`
- `dist\portable\LICENSE`, `README.md`, `VALIDATION.md` and `THIRD_PARTY_NOTICES.txt`
- `dist\MiPoint-Generator-Windows-x64.zip`
- `dist\SHA256SUMS.txt`

Copy the entire portable folder, or distribute the ZIP. End users need no Rust, Python, .NET or Office installation. Windows system DLLs and a working OpenGL 2.0 graphics driver are required. The executable has the Windows GUI subsystem, so normal launch does not open a console. It is unsigned.

For developers, the equivalent commands from this folder are:

```powershell
.\Build.ps1 -Validate
.\Build.ps1 -Launch
cargo fmt --check
cargo clippy --release --locked --all-targets -- -D warnings
```

## Validation and measured throughput

See [VALIDATION.md](VALIDATION.md) for actual measurements, checks and limitations. Benchmarks retain the original generator under `tests\support\original.rs` for comparison. They are ignored during ordinary tests because they write substantial data.

Optional output checks, for developers only:

```powershell
python .\tools\validate_output.py 'C:\Temp\Migration Tests\Dataset-4242'
dotnet run --project .\tools\OpenXmlCheck -c Release -- 'C:\Temp\Migration Tests\Dataset-4242'
```

The first command uses Python 3.12+ standard library modules to check inventory, paths, sizes, metadata, ZIP CRCs, XML relationships, CSV columns and RTF groups. The second requires the .NET 9 SDK and restores Microsoft's pinned Open XML SDK 3.3.0 from NuGet to validate Office package schemas. Neither tool is needed to operate the GUI.

## Replace the existing project

This complete revision was created in the `revised` subfolder so the original source files remain intact. Copy its files and `.cargo`, `src`, `tests` and `tools` directories over the original project when ready. Include `dist` if you want the prebuilt portable application. Do not copy `target`, `.validation`, or developer `bin`/`obj` build directories. Keep the original root `.git` directory. After confirming the replacement, the temporary `revised` subfolder can be removed.

The public repository retains the original source at the root and this revision under `revised` until that replacement is made.

## Licence

MIT. See [LICENSE](LICENSE). Dependencies retain their licences; their texts are included in `THIRD_PARTY_NOTICES.txt` and the portable package.
