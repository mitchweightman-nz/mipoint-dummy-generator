# MiPoint Dummy Migration Set Generator

A lightweight Rust desktop application for producing synthetic business folder trees and migration test files. It creates genuine Open XML `.docx`, `.xlsx` and `.pptx` packages, plus `.csv`, `.rtf` and `.txt` files. All generated text is fictional and marked as synthetic test content.

The generated tree uses a fictional organisation with business divisions, departments, teams, processes, financial years and record categories. File titles are matched to their format and business area, with plausible references, owners, status, actions and review details.

Files are generated from a shuffled 15-file size cycle spanning 1 MB to 15 MB. Each full cycle totals 72 MB, for an average of 4.8 MB per file. Office packages include an uncompressed synthetic payload part to reach the target size without making the visible document text enormous.

## Build and launch

Install Rust with `rustup` once, then double-click **Build and Run.bat**. It builds the Windows desktop app and opens the GUI. After that, you can launch `target\release\mipoint-dummy-generator.exe` directly.

Choose the output folder, number of files, maximum folder depth and file types in the window, then select **Generate files**. Leave the seed blank for a new dataset or enter a seed to reproduce a run. Generation progress and the resulting average file size appear in the window.

The **Create companion structure inventory CSV** option is enabled by default. It adds one CSV beside the organisation folder, with a row for every generated content file. The inventory includes the relative folder path, file name, extension, size and folder depth, plus ten metadata values: record ID, business unit, department, team, process, financial year, record category, owner, status and classification. The inventory is an extra file and is not included in the requested content file count.

The default output folder is `Documents\MiPoint Dummy Migration Set`. Existing files with matching names can be overwritten, so choose an empty folder for a clean dataset.

## Scope

The Office formats are modern Open XML formats. Legacy binary `.doc`, `.xls` and `.ppt` files are not emitted. The organisation, names, references and financial values are fictional.
