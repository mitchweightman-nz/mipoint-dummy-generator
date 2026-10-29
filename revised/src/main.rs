#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use mipoint_dummy_generator::engine::{self, Config, Format, Progress, Report};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FORMATS: [Format; 6] = [
    Format::Docx,
    Format::Xlsx,
    Format::Pptx,
    Format::Csv,
    Format::Rtf,
    Format::Txt,
];
const LABELS: [&str; 6] = [
    "Word (.docx)",
    "Excel (.xlsx)",
    "PowerPoint (.pptx)",
    "CSV (.csv)",
    "Rich text (.rtf)",
    "Plain text (.txt)",
];
const PRESETS: [&str; 5] = [
    "Office core",
    "Full business mix",
    "Documents only",
    "Data and reports",
    "Custom",
];
const ACCENT: Color32 = Color32::from_rgb(0, 101, 105);
const MUTED: Color32 = Color32::from_rgb(64, 82, 103);

const HELP: &[(&str, &str)] = &[
 ("Synthetic data only", "All organisations, identities, references, amounts and business statements are fictional test data. Any resemblance to actual people or businesses is coincidental. Classification labels are synthetic metadata, not evidence of real confidential content. Do not use these records for business decisions."),
 ("Folders and depth", "Every run creates a new Dataset-<seed> subfolder. Existing datasets are preserved. Depth counts folders below that run folder, including the fictional organisation. At depth 7, the hierarchy can include organisation / division / department / team / process / financial year / record category. Shallower files stop earlier; deeper paths add working-record categories. Actual depths vary from min(3, maximum) to the maximum."),
 ("Names and contents", "Record IDs, business-specific titles and review statuses form each name. Business profiles keep related divisions, departments, teams and processes together. Word contains a planning brief; Excel contains an action register; PowerPoint contains a business update; CSV has consistent five-column records; RTF and TXT contain a brief and supporting notes. Selected types are shuffled in balanced cycles so every enabled type appears when the count permits."),
 ("File sizes and average", "MB means 1,000,000 bytes. Defaults: minimum 1 MB, average 4.8 MB, maximum 15 MB. Sizes are rounded to the nearest byte. Each shuffled cycle of up to 15 files totals exactly count x rounded average, including the final partial cycle. The average must lie inside the range. Endpoints are bounds and are not guaranteed to appear. If average equals either bound, every file has that size. Cancelled or failed runs may have a different average. The content limit is 0.1 to 256 MB per file."),
 ("How size padding works", "Office files are Open XML ZIP packages with readable business XML plus a declared, uncompressed synthetic binary part. The binary part repeats a seeded 64 KiB block. XML is stored without compression. Text and RTF repeat synthetic notes; CSV uses a quoted supporting-comments field. Spaces complete the final bytes. These are valid synthetic fixtures, not a model of real document entropy, deduplication or compression ratios."),
 ("Seed and repeatability", "Leave the seed blank for a time-derived value, or enter an unsigned 64-bit integer, including zero. The actual seed appears in the settings and status. The same seed AND settings in this version reproduce relative paths, file bytes, sizes and inventory. Run folder suffixes may differ. Filesystem creation/modification timestamps depend on the real run time and are not reproducible. Different app versions need not reproduce earlier output."),
 ("Inventory and metadata", "The optional Structure Inventory.csv is enabled by default. It records each completed content file: relative path, name, extension, exact size and folder depth, plus ten synthetic fields: record ID, business unit, department, team, business process, financial year, record category, owner, status and classification. Metadata is in the CSV, not applied as filesystem attributes or Office custom properties. Paths are relative to the run folder. The inventory uses UTF-8 with BOM for Excel and is additional to the file count and estimated content size."),
 ("Progress, stopping and errors", "Generation runs in a background worker. Status shows completed files, written MB, elapsed time, MB/s and estimated remaining time based on bytes. The estimate improves as work proceeds and depends on storage speed. Pause finishes the current file, or stops retry waiting, and saves the job locally. Transient destination errors are retried every 5 seconds for the configured window (default 120 seconds), then the dataset pauses. Use Resume paused dataset, or Open saved dataset after restarting the app. Checkpoints and the inventory journal are under %LOCALAPPDATA%\\MiPoint Generator\\Checkpoints. The network inventory is published on normal pause or completion when reachable. Windows calls may also wait for OS network timeouts. Existing datasets from version 0.2.0 have no resumable checkpoint. A failed run keeps its partial dataset and reports the path. Choose a new run after checking disk space and permissions. Peak content memory is roughly one maximum-sized file plus small buffers."),
 ("Portable application", "Launch mipoint-dummy-generator.exe by double-clicking it. No command prompt, Office installation, Rust or Python is required to generate data. The portable build targets Windows 11 x64 and statically links the MSVC C runtime. Windows system libraries and a working OpenGL 2.0 graphics driver are required. Output remains local; the application does not upload generated data."),
];

enum Message {
    Progress(Progress),
    Done(Report),
    Failed(String),
}
struct GeneratorApp {
    output: String,
    files: usize,
    depth: usize,
    seed: String,
    formats: [bool; 6],
    preset: usize,
    average: f64,
    minimum: f64,
    maximum: f64,
    inventory: bool,
    help: bool,
    receiver: Option<Receiver<Message>>,
    cancel: Arc<AtomicBool>,
    started: Option<Instant>,
    duration: Duration,
    completed: usize,
    total_files: usize,
    close_after_stop: bool,
    bytes: u64,
    run_folder: Option<PathBuf>,
    message: String,
    retry_seconds: u64,
    resume_job: Option<PathBuf>,
    session_start_bytes: u64,
}
impl Default for GeneratorApp {
    fn default() -> Self {
        Self {
            output: std::env::var_os("USERPROFILE")
                .map(|p| {
                    PathBuf::from(p)
                        .join("Documents")
                        .join("MiPoint Dummy Migration Set")
                        .display()
                        .to_string()
                })
                .unwrap_or_else(|| "C:\\Temp\\MiPoint Dummy Migration Set".into()),
            files: 50,
            depth: 7,
            seed: String::new(),
            formats: [true; 6],
            preset: 1,
            average: 4.8,
            minimum: 1.0,
            maximum: 15.0,
            inventory: true,
            help: false,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            started: None,
            duration: Duration::ZERO,
            completed: 0,
            total_files: 50,
            close_after_stop: false,
            bytes: 0,
            run_folder: None,
            message: "Ready to create your synthetic dataset.".into(),
            retry_seconds: 120,
            resume_job: None,
            session_start_bytes: 0,
        }
    }
}
fn card(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(211, 221, 229)))
        .corner_radius(10.0)
        .inner_margin(18.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .size(18.0)
                    .strong()
                    .color(Color32::from_rgb(24, 43, 65)),
            );
            ui.add_space(10.0);
            body(ui);
        });
}
fn note(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(13.0).color(MUTED));
}
fn duration(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "Estimating...".into();
    }
    let s = seconds.max(0.0).ceil() as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
impl GeneratorApp {
    fn working(&self) -> bool {
        self.receiver.is_some()
    }
    fn config(&self) -> Result<Config, String> {
        let seed = if self.seed.trim().is_empty() {
            0
        } else {
            self.seed.trim().parse::<u64>().map_err(|_| {
                "Seed must be a whole number from 0 to 18446744073709551615, or blank.".to_string()
            })?
        };
        let cfg = Config {
            output: PathBuf::from(self.output.trim()),
            files: self.files,
            max_depth: self.depth,
            seed,
            formats: FORMATS
                .into_iter()
                .zip(self.formats)
                .filter_map(|(f, yes)| yes.then_some(f))
                .collect(),
            inventory: self.inventory,
            average_mb: self.average,
            minimum_mb: self.minimum,
            maximum_mb: self.maximum,
        };
        cfg.validate()?;
        Ok(cfg)
    }
    fn restore(&mut self, directory: PathBuf, ctx: &egui::Context) {
        match engine::inspect_saved_job(&directory) {
            Ok(info) => {
                self.output = info.config.output.display().to_string();
                self.files = info.config.files;
                self.depth = info.config.max_depth;
                self.seed = info.config.seed.to_string();
                self.formats = FORMATS.map(|f| info.config.formats.contains(&f));
                self.preset = 4;
                self.average = info.config.average_mb;
                self.minimum = info.config.minimum_mb;
                self.maximum = info.config.maximum_mb;
                self.inventory = info.config.inventory;
                self.retry_seconds = info.retry_seconds;
                self.completed = info.completed;
                self.bytes = info.bytes;
                self.session_start_bytes = info.bytes;
                self.total_files = self.files;
                self.run_folder = Some(info.output);
                self.resume_job = Some(directory);
                self.start(ctx, true);
            }
            Err(e) => self.message = e,
        }
    }
    fn start(&mut self, ctx: &egui::Context, resuming: bool) {
        if self.seed.trim().is_empty() {
            self.seed = (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64)
                .to_string();
        }
        let cfg = match self.config() {
            Ok(c) => c,
            Err(e) => {
                self.message = e;
                return;
            }
        };
        let (tx, rx) = mpsc::sync_channel(8);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let ctx = ctx.clone();
        if !resuming {
            self.completed = 0;
            self.bytes = 0;
            self.session_start_bytes = 0;
            self.run_folder = None;
            self.resume_job = None;
        }
        self.total_files = cfg.files;
        let saved_job = if resuming {
            self.resume_job.clone()
        } else {
            None
        };
        let retry_seconds = self.retry_seconds;
        self.started = Some(Instant::now());
        self.duration = Duration::ZERO;
        self.message = format!("Generating with seed {}", cfg.seed);
        self.receiver = Some(rx);
        std::thread::spawn(move || {
            let mut last = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let on_progress = |p: Progress| {
                    if p.completed == 0
                        || p.message.starts_with("Retrying")
                        || last.elapsed() >= Duration::from_millis(100)
                    {
                        let _ = tx.send(Message::Progress(p));
                        ctx.request_repaint();
                        last = Instant::now();
                    }
                };
                if let Some(directory) = saved_job {
                    engine::resume(&directory, &cancel, on_progress)
                } else {
                    engine::start_job(&cfg, &cancel, retry_seconds, on_progress)
                }
            }));
            let message = match result { Ok(Ok(r)) => Message::Done(r), Ok(Err(e)) => Message::Failed(e), Err(_) => Message::Failed("The worker stopped unexpectedly. Check the output folder for a partial dataset.".into()) };
            let _ = tx.send(message);
            ctx.request_repaint();
        });
    }
    fn receive(&mut self) {
        let messages: Vec<_> = self
            .receiver
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        for m in messages {
            match m {
                Message::Progress(p) => {
                    self.completed = p.completed;
                    self.bytes = p.bytes;
                    self.run_folder = Some(p.output);
                    self.resume_job = Some(p.checkpoint);
                    self.message = p.message;
                }
                Message::Done(r) => {
                    self.completed = r.completed;
                    self.bytes = r.bytes;
                    self.run_folder = Some(r.output);
                    self.resume_job = if r.paused { Some(r.checkpoint) } else { None };
                    self.duration = self.started.map(|t| t.elapsed()).unwrap_or_default();
                    self.receiver = None;
                    self.message = format!(
                        "{} {} files, {:.2} MB total, {:.6} MB average. Seed {}.",
                        if r.paused {
                            "Paused after"
                        } else {
                            "Completed:"
                        },
                        r.completed,
                        r.bytes as f64 / 1e6,
                        r.bytes as f64 / r.completed.max(1) as f64 / 1e6,
                        self.seed
                    );
                    if let Some(reason) = r.reason {
                        self.message.push_str(&format!(" {reason}"));
                    }
                    if r.paused {
                        self.message.push_str(
                            " Use Resume paused dataset when the destination is available.",
                        );
                    }
                }
                Message::Failed(e) => {
                    self.duration = self.started.map(|t| t.elapsed()).unwrap_or_default();
                    self.receiver = None;
                    self.message = format!("Generation stopped: {e}");
                }
            }
        }
    }
    fn output_card(&mut self, ui: &mut egui::Ui) {
        card(ui, "01  Output location", |ui| {
            note(ui, "Choose where the dataset will be saved");
            ui.add_space(5.0);
            let w = (ui.available_width() - 100.0).max(100.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.output).desired_width(w));
                if ui.button("Browse...").clicked() {
                    let mut dialog =
                        rfd::FileDialog::new().set_title("Choose dataset output folder");
                    if PathBuf::from(self.output.trim()).is_dir() {
                        dialog = dialog.set_directory(self.output.trim());
                    }
                    if let Some(p) = dialog.pick_folder() {
                        self.output = p.display().to_string();
                    }
                }
            });
            note(
                ui,
                "Each run uses a new subfolder and preserves existing datasets.",
            );
        });
    }
    fn dataset_card(&mut self, ui: &mut egui::Ui) {
        card(ui, "02  Dataset structure", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Files");
                ui.add(
                    egui::DragValue::new(&mut self.files)
                        .range(1..=100_000)
                        .speed(1),
                );
                ui.add_space(14.0);
                ui.label("Maximum depth");
                ui.add(egui::DragValue::new(&mut self.depth).range(1..=12).speed(1));
            });
            ui.add_space(10.0);
            ui.label("Random seed");
            ui.add(
                egui::TextEdit::singleline(&mut self.seed)
                    .desired_width(ui.available_width())
                    .hint_text("Leave blank for a new seed"),
            );
            note(ui, "Reuse the seed and settings to reproduce a dataset. Depth includes the organisation folder.");
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Network retry window (seconds)");
                ui.add(
                    egui::DragValue::new(&mut self.retry_seconds)
                        .range(5..=3600)
                        .speed(5),
                );
            });
            note(
                ui,
                "Retry every 5 seconds, then pause. Resume uses a checkpoint saved on this PC.",
            );
            ui.add_space(10.0);
            ui.checkbox(&mut self.inventory, "Create companion inventory CSV");
            note(
                ui,
                "One row per content file, with paths, sizes and 10 synthetic metadata fields.",
            );
        });
    }
    fn types_card(&mut self, ui: &mut egui::Ui) {
        card(ui, "03  File types", |ui| {
            let previous = self.preset;
            egui::ComboBox::from_id_salt("preset")
                .selected_text(PRESETS[self.preset])
                .width((ui.available_width() - 32.0).min(300.0))
                .show_ui(ui, |ui| {
                    for (i, p) in PRESETS.iter().enumerate() {
                        ui.selectable_value(&mut self.preset, i, *p);
                    }
                });
            if previous != self.preset {
                self.formats = match self.preset {
                    0 => [true, true, true, false, false, false],
                    1 => [true; 6],
                    2 => [true, false, false, false, true, true],
                    3 => [false, true, true, true, false, true],
                    _ => self.formats,
                };
            }
            ui.add_space(8.0);
            for (i, label) in LABELS.iter().enumerate() {
                if ui.checkbox(&mut self.formats[i], *label).changed() {
                    self.preset = 4;
                }
            }
            ui.add_space(6.0);
            let enabled = FORMATS
                .iter()
                .zip(self.formats)
                .filter_map(|(f, on)| on.then_some(f.extension().to_uppercase()))
                .collect::<Vec<_>>()
                .join(", ");
            note(
                ui,
                format!(
                    "Enabled: {}",
                    if enabled.is_empty() { "None" } else { &enabled }
                ),
            );
        });
    }
    fn sizes_card(&mut self, ui: &mut egui::Ui) {
        card(ui, "04  File sizes", |ui| {
            for (label, value) in [
                ("Average MB", &mut self.average),
                ("Minimum MB", &mut self.minimum),
                ("Maximum MB", &mut self.maximum),
            ] {
                ui.horizontal(|ui| {
                    ui.add_sized([110.0, 24.0], egui::Label::new(label));
                    ui.add(
                        egui::DragValue::new(value)
                            .speed(0.1)
                            .range(0.1..=256.0)
                            .max_decimals(6),
                    );
                });
            }
            ui.add_space(8.0);
            note(ui,"Decimal MB. Exact requested mean for each completed cycle, including the final partial cycle.");
            match self.config().and_then(|c| c.estimated_bytes()) {
                Ok(b) => {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("Estimated content: {:.2} MB", b as f64 / 1e6))
                            .size(18.0)
                            .strong()
                            .color(ACCENT),
                    );
                    note(
                        ui,
                        format!(
                            "{:.3} GB, plus a small inventory if enabled",
                            b as f64 / 1e9
                        ),
                    );
                }
                Err(e) => {
                    ui.colored_label(Color32::from_rgb(157, 37, 35), e);
                }
            }
        });
    }
    fn progress_card(&mut self, ui: &mut egui::Ui) {
        let working = self.working();
        let elapsed = if working {
            self.started.map(|t| t.elapsed()).unwrap_or_default()
        } else {
            self.duration
        };
        let secs = elapsed.as_secs_f64();
        let rate = if secs > 0.0 {
            self.bytes.saturating_sub(self.session_start_bytes) as f64 / 1e6 / secs
        } else {
            0.0
        };
        let display_total = if self.started.is_some() {
            self.total_files
        } else {
            self.files
        };
        card(ui, "Generation", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !working && self.config().is_ok(),
                        egui::Button::new(
                            RichText::new("Generate dataset")
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .fill(ACCENT)
                        .min_size(egui::vec2(162.0, 36.0)),
                    )
                    .clicked()
                {
                    self.start(ui.ctx(), false);
                }
                if working
                    && ui
                        .add_enabled(
                            !self.cancel.load(Ordering::Relaxed),
                            egui::Button::new("Pause dataset"),
                        )
                        .clicked()
                {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.message = "Pausing after the current file or retry wait...".into();
                }
                if ui
                    .add_enabled(
                        !working && self.resume_job.is_some(),
                        egui::Button::new("Resume paused dataset"),
                    )
                    .clicked()
                {
                    if let Some(directory) = self.resume_job.clone() {
                        self.restore(directory, ui.ctx());
                    }
                }
                if ui
                    .add_enabled(!working, egui::Button::new("Open saved dataset..."))
                    .clicked()
                {
                    if let Some(directory) = rfd::FileDialog::new()
                        .set_title("Select a saved job folder")
                        .set_directory(engine::checkpoint_directory())
                        .pick_folder()
                    {
                        self.restore(directory, ui.ctx());
                    }
                }
                if let Some(path) = &self.run_folder {
                    if ui.button("Open output folder").clicked() {
                        if let Err(e) = std::process::Command::new("explorer.exe").arg(path).spawn()
                        {
                            self.message = format!("Cannot open output folder: {e}");
                        }
                    }
                }
                if ui.button("Help (F1)").clicked() {
                    self.help = true;
                }
            });
            ui.add_space(8.0);
            ui.add(
                egui::ProgressBar::new(self.completed as f32 / display_total.max(1) as f32)
                    .fill(ACCENT)
                    .text(format!("{} / {} files", self.completed, display_total)),
            );
            ui.add_space(6.0);
            ui.label(&self.message);
            if !working {
                if let Err(error) = self.config() {
                    ui.colored_label(Color32::from_rgb(157, 37, 35), error);
                }
            }
            if working || self.started.is_some() {
                let remaining = self
                    .config()
                    .and_then(|c| c.estimated_bytes())
                    .unwrap_or(0)
                    .saturating_sub(self.bytes);
                let eta = if working && rate > 0.0 && secs >= 0.3 {
                    duration(remaining as f64 / 1e6 / rate)
                } else if working {
                    "Estimating...".into()
                } else {
                    "--".into()
                };
                ui.horizontal_wrapped(|ui| {note(ui,format!("Elapsed {}    |    {:.1} MB/s    |    Remaining {}    |    {:.2} MB written",duration(secs),rate,eta,self.bytes as f64/1e6));});
            }
            if let Some(path) = &self.run_folder {
                note(ui, format!("Dataset: {}", path.display()));
            }
            if !working {
                if let Some(directory) = &self.resume_job {
                    note(ui, format!("Saved job on this PC: {}", directory.display()));
                }
            }
        });
    }
}
impl eframe::App for GeneratorApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::from_rgb(240, 244, 248).to_normalized_gamma_f32()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}
impl GeneratorApp {
    fn draw(&mut self, ui: &mut egui::Ui) {
        self.receive();
        if ui.input(|i| i.viewport().close_requested()) && self.working() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel.store(true, Ordering::Relaxed);
            self.close_after_stop = true;
            self.message = "Finishing the current file before closing...".into();
        }
        if self.close_after_stop && !self.working() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ui.input(|i| i.key_pressed(egui::Key::F1)) {
            self.help = true;
        }
        if self.working() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        egui::Panel::bottom("generation_panel")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(240, 244, 248))
                    .inner_margin(16.0),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("status_scroll")
                    .max_height(260.0)
                    .show(ui, |ui| {
                        self.progress_card(ui);
                    });
            });
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("settings_scroll").auto_shrink([false,false]).show(ui, |ui| {
                ui.label(RichText::new("MiPoint").size(17.0).strong().color(ACCENT));
                ui.label(RichText::new("Migration Set Generator").size(29.0).strong().color(Color32::from_rgb(21,40,61)));
                note(ui,"Realistic business structures. Repeatable migration tests. Entirely synthetic data.");
                ui.add_space(18.0);
                let working = self.working();
                egui::CollapsingHeader::new("Configuration").id_salt("configuration").default_open(true).show(ui, |ui| {
                ui.add_enabled_ui(!working, |ui| {
                    self.output_card(ui);
                    ui.add_space(12.0);
                    if ui.available_width() >= 860.0 {
                        ui.columns(2, |cols| {
                            self.dataset_card(&mut cols[0]);
                            cols[0].add_space(12.0);
                            self.sizes_card(&mut cols[0]);
                            self.types_card(&mut cols[1]);
                            cols[1].add_space(12.0);
                            card(&mut cols[1], "Built for migration testing", |ui| {
                                note(ui,"Business profiles link divisions, departments, teams and processes. Every record is labelled as synthetic. Press F1 for generation and size details.");
                            });
                        });
                    } else {
                        self.dataset_card(ui);
                        ui.add_space(12.0);
                        self.types_card(ui);
                        ui.add_space(12.0);
                        self.sizes_card(ui);
                    }
                });
                });
                ui.add_space(12.0);
            });
        });
        if self.help {
            egui::Window::new("Help - MiPoint Migration Set Generator")
                .open(&mut self.help)
                .default_width(680.0)
                .default_height(600.0)
                .resizable(true)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (title, text) in HELP {
                            ui.heading(*title);
                            ui.label(*text);
                            ui.add_space(14.0);
                        }
                    });
                });
        }
    }
}
fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 640.0])
            .with_clamp_size_to_monitor_size(true)
            .with_min_inner_size([560.0, 500.0]),
        centered: true,
        ..Default::default()
    };
    let result = eframe::run_native(
        "MiPoint Dummy Migration Set Generator",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            cc.egui_ctx.style_mut_of(egui::Theme::Light, |s| {
                s.spacing.item_spacing = egui::vec2(10.0, 8.0);
                s.spacing.button_padding = egui::vec2(12.0, 7.0);
                s.visuals.panel_fill = Color32::from_rgb(240, 244, 248);
                s.visuals.override_text_color = Some(Color32::from_rgb(25, 44, 64));
                s.visuals.selection.bg_fill = ACCENT;
                s.visuals.widgets.inactive.bg_fill = Color32::from_rgb(230, 237, 242);
                s.visuals.widgets.inactive.fg_stroke =
                    egui::Stroke::new(1.0, Color32::from_rgb(25, 44, 64));
                s.text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
                s.text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
            });
            Ok(Box::<GeneratorApp>::default())
        }),
    );
    if let Err(e) = result {
        rfd::MessageDialog::new()
            .set_title("MiPoint could not start")
            .set_description(format!(
                "{e}\nCheck that Windows and your graphics driver are up to date."
            ))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draw_at(app: &mut GeneratorApp, width: f32, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::light());
        draw_on(&ctx, app, width, events)
    }
    fn draw_on(
        ctx: &egui::Context,
        app: &mut GeneratorApp,
        width: f32,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 640.0));
        let mut output = None;
        for frame in 0..16 {
            let input = egui::RawInput {
                screen_rect: Some(rect),
                events: if frame == 2 { events.clone() } else { vec![] },
                ..Default::default()
            };
            output = Some(ctx.run_ui(input, |ui| {
                app.draw(ui);
                assert!(
                    ui.min_rect().right() <= width + 1.0,
                    "Content overflow at width {width}: {:?}",
                    ui.min_rect()
                );
            }));
        }
        output.unwrap()
    }
    #[test]
    fn narrow_and_wide_layout_keep_generate_button_visible() {
        for width in [560.0, 620.0, 1040.0] {
            let output = draw_at(&mut GeneratorApp::default(), width, vec![]);
            let button = output
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Generate dataset" => {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("Generate button must be rendered");
            assert!(button.pos.y >= 0.0 && button.pos.y + button.galley.size().y <= 640.0);
        }
    }
    #[test]
    fn configuration_collapses_while_generation_controls_stay_visible() {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::light());
        let mut app = GeneratorApp::default();
        let expanded = draw_on(&ctx, &mut app, 1040.0, vec![]);
        let text_position = expanded
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Configuration" => {
                    Some(t.pos + egui::vec2(5.0, 5.0))
                }
                _ => None,
            })
            .unwrap();
        let collapsed = draw_on(
            &ctx,
            &mut app,
            1040.0,
            vec![
                egui::Event::PointerMoved(text_position),
                egui::Event::PointerButton {
                    pos: text_position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: text_position,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let texts = collapsed
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Generate dataset"));
        assert!(!texts.contains(&"01  Output location"));
        let reopened = draw_on(
            &ctx,
            &mut app,
            1040.0,
            vec![
                egui::Event::PointerMoved(text_position),
                egui::Event::PointerButton {
                    pos: text_position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: text_position,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(reopened.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="01  Output location")
        ));
    }
    #[test]
    fn f1_opens_help() {
        let mut app = GeneratorApp::default();
        draw_at(
            &mut app,
            1040.0,
            vec![egui::Event::Key {
                key: egui::Key::F1,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.help);
    }
    #[test]
    fn gui_config_rejects_invalid_seed_and_impossible_average() {
        let mut app = GeneratorApp {
            seed: "-1".into(),
            ..Default::default()
        };
        assert!(app.config().err().unwrap().contains("Seed"));
        app.seed = "0".into();
        app.average = 16.0;
        assert!(app.config().err().unwrap().contains("cannot fit"));
        app.average = 4.8;
        assert_eq!(app.config().unwrap().seed, 0);
    }
}
