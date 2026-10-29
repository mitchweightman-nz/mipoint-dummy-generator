use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Docx,
    Xlsx,
    Pptx,
    Csv,
    Rtf,
    Txt,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Csv => "csv",
            Self::Rtf => "rtf",
            Self::Txt => "txt",
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub output: PathBuf,
    pub files: usize,
    pub max_depth: usize,
    pub seed: u64,
    pub formats: Vec<Format>,
    pub inventory: bool,
    pub average_mb: f64,
    pub minimum_mb: f64,
    pub maximum_mb: f64,
}

impl Config {
    pub fn size_bounds(&self) -> Result<(usize, usize, usize), String> {
        let values = [self.minimum_mb, self.average_mb, self.maximum_mb];
        if values
            .iter()
            .any(|v| !v.is_finite() || *v < 0.1 || *v > 256.0)
        {
            return Err(
                "Each size must be between 0.1 and 256 MB (1 MB = 1,000,000 bytes).".into(),
            );
        }
        if self.minimum_mb > self.maximum_mb {
            return Err("Minimum size must not exceed maximum size.".into());
        }
        if self.average_mb < self.minimum_mb || self.average_mb > self.maximum_mb {
            return Err("The requested average cannot fit the selected range. Set minimum <= average <= maximum.".into());
        }
        Ok((
            (self.minimum_mb * 1e6).round() as usize,
            (self.average_mb * 1e6).round() as usize,
            (self.maximum_mb * 1e6).round() as usize,
        ))
    }
    pub fn validate(&self) -> Result<(), String> {
        self.size_bounds()?;
        if self.files == 0 || self.files > 100_000 {
            return Err("Choose between 1 and 100,000 files.".into());
        }
        if !(1..=12).contains(&self.max_depth) {
            return Err("Folder depth must be between 1 and 12.".into());
        }
        if self.formats.is_empty() {
            return Err("Select at least one file type.".into());
        }
        if self.output.as_os_str().is_empty() || !self.output.is_absolute() {
            return Err("Choose an absolute output folder path.".into());
        }
        Ok(())
    }
    pub fn estimated_bytes(&self) -> Result<u64, String> {
        Ok(self.size_bounds()?.1 as u64 * self.files as u64)
    }
}

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e3779b97f4a7c15)
    }
    fn next(&mut self) -> u64 {
        if self.0 == 0 {
            self.0 = 0xa0761d6478bd642f;
        }
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
    fn between(&mut self, low: usize, high: usize) -> usize {
        low + self.range(high - low + 1)
    }
}

const FOLDER_KINDS: &[&str] = &[
    "Working Documents",
    "Approvals",
    "Meeting Papers",
    "Reference Material",
    "Drafts",
    "Final Deliverables",
    "Correspondence",
    "Reports",
    "Supporting Evidence",
    "Archive Copy",
];
const ORGS: &[&str] = &[
    "Northstar Services",
    "Harbour Group",
    "Summit Operations",
    "Cedar Consulting",
    "Bluewater Partners",
    "Westfield Support",
    "Meridian Works",
    "Kauri Advisory",
];
const FIRST: &[&str] = &[
    "Alex", "Morgan", "Jamie", "Taylor", "Sam", "Jordan", "Casey", "Riley", "Avery", "Drew",
    "Robin", "Cameron",
];
const LAST: &[&str] = &[
    "Mason", "Wright", "Patel", "Chen", "Thompson", "Wilson", "Ngata", "Brown", "Singh", "Martin",
    "Taylor", "Reed",
];
const FILE_SUFFIXES: &[&str] = &[
    "Draft",
    "For Review",
    "Approved",
    "Working Copy",
    "Final",
    "Updated",
    "Signed Off",
];

struct BusinessProfile {
    division: &'static str,
    department: &'static str,
    team: &'static str,
    process: &'static str,
    code: &'static str,
}
const PROFILES: &[BusinessProfile] = &[
    BusinessProfile {
        division: "Corporate Services",
        department: "Finance",
        team: "Financial Planning",
        process: "Annual Budget",
        code: "FIN",
    },
    BusinessProfile {
        division: "Corporate Services",
        department: "Finance",
        team: "Financial Control",
        process: "Month End Close",
        code: "FIN",
    },
    BusinessProfile {
        division: "Corporate Services",
        department: "People and Culture",
        team: "Recruitment",
        process: "Workforce Planning",
        code: "PPL",
    },
    BusinessProfile {
        division: "Corporate Services",
        department: "People and Culture",
        team: "Employee Services",
        process: "Policy Review",
        code: "PPL",
    },
    BusinessProfile {
        division: "Digital and Technology",
        department: "Information Services",
        team: "Business Applications",
        process: "Customer Platform Renewal",
        code: "ICT",
    },
    BusinessProfile {
        division: "Digital and Technology",
        department: "Information Services",
        team: "Service Operations",
        process: "Service Continuity",
        code: "ICT",
    },
    BusinessProfile {
        division: "Digital and Technology",
        department: "Information Services",
        team: "Information Management",
        process: "Records Review",
        code: "IM",
    },
    BusinessProfile {
        division: "Operations",
        department: "Customer Services",
        team: "Contact Centre",
        process: "Customer Experience Review",
        code: "OPS",
    },
    BusinessProfile {
        division: "Operations",
        department: "Service Delivery",
        team: "Field Operations",
        process: "Service Improvement",
        code: "OPS",
    },
    BusinessProfile {
        division: "Operations",
        department: "Property and Facilities",
        team: "Asset Management",
        process: "Facilities Maintenance",
        code: "AST",
    },
    BusinessProfile {
        division: "Commercial Services",
        department: "Procurement",
        team: "Supplier Management",
        process: "Supplier Transition",
        code: "PRC",
    },
    BusinessProfile {
        division: "Commercial Services",
        department: "Procurement",
        team: "Sourcing",
        process: "Contract Renewal",
        code: "PRC",
    },
    BusinessProfile {
        division: "Governance and Risk",
        department: "Governance",
        team: "Committee Support",
        process: "Board Reporting",
        code: "GOV",
    },
    BusinessProfile {
        division: "Governance and Risk",
        department: "Legal Services",
        team: "Advisory",
        process: "Information Request Review",
        code: "LEG",
    },
    BusinessProfile {
        division: "Governance and Risk",
        department: "Risk and Assurance",
        team: "Operational Risk",
        process: "Risk Assessment",
        code: "RSK",
    },
    BusinessProfile {
        division: "Communications",
        department: "Communications",
        team: "Internal Communications",
        process: "Change Communications",
        code: "COM",
    },
];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn pick<'a>(rng: &mut Rng, values: &'a [&'a str]) -> &'a str {
    values[rng.range(values.len())]
}
fn person(rng: &mut Rng) -> String {
    format!("{} {}", pick(rng, FIRST), pick(rng, LAST))
}
struct RecordText<'a> {
    title: &'a str,
    owner: &'a str,
    record_id: &'a str,
    status: &'a str,
    classification: &'a str,
}
fn business_text(
    rng: &mut Rng,
    org: &str,
    profile: &BusinessProfile,
    record: RecordText<'_>,
) -> Vec<String> {
    let BusinessProfile {
        division,
        department: dept,
        team,
        process: area,
        ..
    } = profile;
    let RecordText {
        title,
        owner,
        record_id,
        status,
        classification,
    } = record;
    let budget = rng.between(25, 950) * 1000;
    let date = format!("2026-{:02}-{:02}", rng.between(1, 12), rng.between(1, 28));
    vec![
        format!("{} | {}", org, division),
        format!("{}: {}", title, area),
        String::new(),
        "Purpose".to_string(),
        format!("This document records the current position for {} and supports planning, review and handover across {}.", area, dept),
        String::new(),
        "Executive summary".to_string(),
        format!("{} has reviewed the available information for {}. The recommended next step is to confirm the scope, maintain the delivery schedule and report material changes through the normal governance process.", team, area),
        String::new(),
        "Current position".to_string(),
        format!("The work is coordinated by {} within {}. The current status is {}. The working estimate is ${}, subject to confirmation during detailed planning.", team, dept, status, budget),
        String::new(),
        "Key considerations".to_string(),
        "- Confirm the scope and expected outcome with the service owner.".to_string(),
        "- Record decisions and retain supporting evidence with the project record.".to_string(),
        "- Review dependencies, timing and resourcing at the next planning meeting.".to_string(),
        String::new(),
        "Actions".to_string(),
        format!("1. Confirm the delivery approach | Owner: {} | Due: {}", owner, date),
        format!("2. Update the supporting record | Owner: {} | Status: {}", person(rng), status),
        String::new(),
        format!("Document owner: {} | Team: {}", owner, team),
        format!("Record reference: {} | Classification: {}", record_id, classification),
        "Synthetic test record. All names, organisations, references and figures are fictional.".to_string(),
    ]
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn csv_row(values: &[String]) -> String {
    format!(
        "{}\r\n",
        values
            .iter()
            .map(|value| csv_cell(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn zip_package(
    mut entries: Vec<(&str, String)>,
    target_bytes: usize,
    rng: &mut Rng,
) -> Result<Vec<u8>, String> {
    if let Some((_, types)) = entries
        .iter_mut()
        .find(|(name, _)| *name == "[Content_Types].xml")
    {
        let at = types
            .find("</Types>")
            .ok_or("Missing content types closing element")?;
        types.insert_str(
            at,
            "<Default Extension=\"bin\" ContentType=\"application/octet-stream\"/>",
        );
    }
    let member = "customData/synthetic-payload.bin";
    // Stored entries have fixed local/central headers. No compression, second
    // archive or full-size payload allocation is needed. Tests assert exact sizes.
    let overhead = 22
        + entries
            .iter()
            .map(|(n, s)| 76 + 2 * n.len() + s.len())
            .sum::<usize>()
        + 76
        + 2 * member.len();
    let remaining = target_bytes
        .checked_sub(overhead)
        .ok_or("Target size is smaller than the Office package")?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::with_capacity(target_bytes)));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    for (name, contents) in entries {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(contents.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    zip.start_file(member, options).map_err(|e| e.to_string())?;
    let mut block = [0u8; 65536];
    for chunk in block.as_chunks_mut::<8>().0 {
        chunk.copy_from_slice(&rng.next().to_le_bytes());
    }
    let mut left = remaining;
    while left > 0 {
        let n = left.min(block.len());
        zip.write_all(&block[..n]).map_err(|e| e.to_string())?;
        left -= n;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    if bytes.len() != target_bytes {
        return Err(format!(
            "ZIP size mismatch: {} != {}",
            bytes.len(),
            target_bytes
        ));
    }
    Ok(bytes)
}

fn pad_ascii(mut bytes: Vec<u8>, target: usize, block: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > target {
        return Err("Target size is smaller than the document content".into());
    }
    bytes.reserve(target - bytes.len());
    while target - bytes.len() >= block.len() {
        bytes.extend_from_slice(block);
    }
    // ASCII spaces are valid inside text, RTF bodies and a quoted CSV field.
    bytes.resize(target, b' ');
    Ok(bytes)
}

fn docx(lines: &[String], target_bytes: usize, rng: &mut Rng) -> Result<Vec<u8>, String> {
    let body = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if line.is_empty() {
                "<w:p/>".to_string()
            } else if index == 0 {
                format!(
                    "<w:p><w:r><w:rPr><w:b/><w:sz w:val=\"20\"/></w:rPr><w:t>{}</w:t></w:r></w:p>",
                    esc(line)
                )
            } else if index == 1 {
                format!(
                    "<w:p><w:r><w:rPr><w:b/><w:sz w:val=\"32\"/></w:rPr><w:t>{}</w:t></w:r></w:p>",
                    esc(line)
                )
            } else if [
                "Purpose",
                "Executive summary",
                "Current position",
                "Key considerations",
                "Actions",
            ]
            .contains(&line.as_str())
            {
                format!(
                    "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>{}</w:t></w:r></w:p>",
                    esc(line)
                )
            } else {
                format!(
                    "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                    esc(line)
                )
            }
        })
        .collect::<String>();
    let document = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr></w:body></w:document>", body);
    zip_package(vec![
        ("[Content_Types].xml", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>".into()),
        ("_rels/.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>".into()),
        ("word/document.xml", document),
    ], target_bytes, rng)
}

fn xlsx(rng: &mut Rng, dept: &str, area: &str, target_bytes: usize) -> Result<Vec<u8>, String> {
    let sheet_name = "Status and Actions";
    let mut rows: Vec<Vec<String>> = vec![vec![
        "Work item".into(),
        "Owner".into(),
        "Status".into(),
        "Target date".into(),
        "Comments".into(),
    ]];
    for i in 0..8 {
        rows.push(vec![
            format!("{} - workstream {:02}", area, i + 1),
            person(rng),
            pick(
                rng,
                &["Not started", "In progress", "For review", "Complete"],
            )
            .into(),
            format!("2026-{:02}-{:02}", rng.between(1, 12), rng.between(1, 28)),
            format!(
                "Synthetic test record: {}",
                pick(
                    rng,
                    &[
                        "Progress update due",
                        "Dependency being confirmed",
                        "Evidence filed",
                        "Review at next meeting"
                    ]
                )
            ),
        ]);
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheetData>");
    for (ri, row) in rows.iter().enumerate() {
        xml.push_str(&format!("<row r=\"{}\">", ri + 1));
        for (ci, value) in row.iter().enumerate() {
            let col = (b'A' + ci as u8) as char;
            xml.push_str(&format!(
                "<c r=\"{}{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                col,
                ri + 1,
                esc(value)
            ));
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData><autoFilter ref=\"A1:E9\"/></worksheet>");
    let workbook = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"{}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>", sheet_name);
    let _ = dept;
    zip_package(vec![
        ("[Content_Types].xml", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>".into()),
        ("_rels/.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>".into()),
        ("xl/workbook.xml", workbook),
        ("xl/_rels/workbook.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>".into()),
        ("xl/worksheets/sheet1.xml", xml),
    ], target_bytes, rng)
}

fn pptx(lines: &[String], target_bytes: usize, rng: &mut Rng) -> Result<Vec<u8>, String> {
    let title = lines
        .get(1)
        .cloned()
        .unwrap_or_else(|| "Business Update".into());
    let content = [
        "SYNTHETIC BUSINESS UPDATE - migration testing only".to_string(),
        "Scope: confirm outcomes, dependencies and delivery milestones.".into(),
        "Progress: planning complete; implementation review in progress.".into(),
        "Actions: confirm resources and retain supporting evidence.".into(),
        lines
            .iter()
            .find(|s| s.starts_with("Document owner:"))
            .cloned()
            .unwrap_or_default(),
        lines
            .iter()
            .find(|s| s.starts_with("Record reference:"))
            .cloned()
            .unwrap_or_default(),
    ];
    let mut shapes = String::new();
    for (i, line) in content.iter().enumerate() {
        let y = 1500000 + (i as i64 * 650000);
        shapes.push_str(&format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Text {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"900000\" y=\"{}\"/><a:ext cx=\"10500000\" cy=\"560000\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap=\"square\"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-NZ\" sz=\"1800\"/><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-NZ\"/></a:p></p:txBody></p:sp>", i + 3, i + 3, y, esc(line)));
    }
    let slide = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"900000\" y=\"400000\"/><a:ext cx=\"10500000\" cy=\"800000\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap=\"square\"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-NZ\" sz=\"2800\" b=\"1\"/><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-NZ\"/></a:p></p:txBody></p:sp>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>", esc(&title), shapes);
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slides/slide1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/><Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/><Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/><Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/></Types>";
    let pres = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id=\"256\" r:id=\"rId2\"/></p:sldIdLst><p:sldSz cx=\"12192000\" cy=\"6858000\" type=\"screen16x9\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>";
    let master = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" bg1=\"lt1\" bg2=\"lt2\" tx1=\"dk1\" tx2=\"dk2\" hlink=\"hlink\" folHlink=\"folHlink\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>";
    let layout = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" type=\"blank\"><p:cSld name=\"Blank\"><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>";
    let theme = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Synthetic Business Theme\"><a:themeElements><a:clrScheme name=\"Synthetic\"><a:dk1><a:srgbClr val=\"000000\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1><a:dk2><a:srgbClr val=\"1F2937\"/></a:dk2><a:lt2><a:srgbClr val=\"F3F4F6\"/></a:lt2><a:accent1><a:srgbClr val=\"2563EB\"/></a:accent1><a:accent2><a:srgbClr val=\"059669\"/></a:accent2><a:accent3><a:srgbClr val=\"D97706\"/></a:accent3><a:accent4><a:srgbClr val=\"7C3AED\"/></a:accent4><a:accent5><a:srgbClr val=\"DB2777\"/></a:accent5><a:accent6><a:srgbClr val=\"0891B2\"/></a:accent6><a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink><a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink></a:clrScheme><a:fontScheme name=\"Synthetic\"><a:majorFont><a:latin typeface=\"Aptos Display\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont><a:minorFont><a:latin typeface=\"Aptos\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme><a:fmtScheme name=\"Synthetic\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln><a:ln w=\"12700\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln><a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>";
    zip_package(vec![
        ("[Content_Types].xml", content_types.into()),
        ("_rels/.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/></Relationships>".into()),
        ("ppt/presentation.xml", pres.into()),
        ("ppt/_rels/presentation.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide1.xml\"/></Relationships>".into()),
        ("ppt/slides/slide1.xml", slide),
        ("ppt/slides/_rels/slide1.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/></Relationships>".into()),
        ("ppt/slideLayouts/slideLayout1.xml", layout.into()),
        ("ppt/slideLayouts/_rels/slideLayout1.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"../slideMasters/slideMaster1.xml\"/></Relationships>".into()),
        ("ppt/slideMasters/slideMaster1.xml", master.into()),
        ("ppt/slideMasters/_rels/slideMaster1.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"../theme/theme1.xml\"/></Relationships>".into()),
        ("ppt/theme/theme1.xml", theme.into()),
    ], target_bytes, rng)
}

fn safe_name(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '-'
            } else {
                c
            }
        })
        .collect()
}

fn title_for(rng: &mut Rng, profile: &BusinessProfile, format: Format) -> &'static str {
    let choices: &[&str] = match format {
        Format::Xlsx | Format::Csv => match profile.code {
            "FIN" => &[
                "Annual Budget Forecast",
                "Monthly Variance Report",
                "Cost Centre Summary",
                "Month End Reconciliation",
            ],
            "PRC" => &[
                "Supplier Evaluation Register",
                "Contract Renewal Schedule",
                "Procurement Action Register",
                "Supplier Performance Review",
            ],
            "RSK" => &[
                "Risk and Issue Register",
                "Control Assessment Summary",
                "Assurance Actions Register",
            ],
            _ => &[
                "Service Performance Report",
                "Project Action Register",
                "Benefits Tracking Summary",
                "Workstream Status Report",
            ],
        },
        Format::Pptx => &[
            "Steering Committee Update",
            "Project Board Briefing",
            "Service Performance Overview",
            "Quarterly Business Review",
            "Change Programme Update",
        ],
        _ => match profile.code {
            "FIN" => &[
                "Budget Planning Brief",
                "Financial Control Procedure",
                "Month End Review",
                "Finance Decision Record",
            ],
            "PPL" => &[
                "Workforce Planning Brief",
                "Recruitment Campaign Plan",
                "People Policy Review",
                "Employee Services Update",
            ],
            "ICT" | "IM" => &[
                "Service Transition Plan",
                "System Requirements Summary",
                "Change Impact Assessment",
                "Service Continuity Procedure",
            ],
            "PRC" => &[
                "Supplier Review Summary",
                "Contract Renewal Plan",
                "Procurement Decision Record",
                "Request for Proposal Evaluation",
            ],
            "GOV" | "LEG" | "RSK" => &[
                "Steering Committee Minutes",
                "Decision Record",
                "Risk and Issue Review",
                "Policy Review",
            ],
            "COM" => &[
                "Change Communications Plan",
                "Stakeholder Briefing",
                "Communications Approval Record",
                "Engagement Summary",
            ],
            _ => &[
                "Service Improvement Plan",
                "Standard Operating Procedure",
                "Customer Feedback Analysis",
                "Quality Review Findings",
                "Project Status Report",
            ],
        },
    };
    pick(rng, choices)
}

fn category_for(title: &str) -> &'static str {
    if title.contains("Minutes") || title.contains("Committee") || title.contains("Board") {
        "Meeting Papers"
    } else if title.contains("Budget") || title.contains("Variance") || title.contains("Forecast") {
        "Financial Planning"
    } else if title.contains("Risk") || title.contains("Control") || title.contains("Assurance") {
        "Risk and Assurance"
    } else if title.contains("Supplier")
        || title.contains("Contract")
        || title.contains("Procurement")
    {
        "Supplier Management"
    } else if title.contains("Procedure") || title.contains("Policy") {
        "Policies and Procedures"
    } else if title.contains("Register") || title.contains("Schedule") || title.contains("Tracking")
    {
        "Registers and Schedules"
    } else {
        "Reports and Briefings"
    }
}

fn make_file(
    format: Format,
    lines: &[String],
    rng: &mut Rng,
    dept: &str,
    area: &str,
    target: usize,
) -> Result<Vec<u8>, String> {
    const TEXT: &[u8] = b"Synthetic supporting note: confirm scope, record decisions and review delivery dependencies.\r\n";
    match format {
        Format::Docx => docx(lines, target, rng),
        Format::Xlsx => xlsx(rng, dept, area, target),
        Format::Pptx => pptx(lines, target, rng),
        Format::Txt => pad_ascii(
            format!("{}\r\n\r\nSupporting notes\r\n", lines.join("\r\n")).into_bytes(),
            target,
            TEXT,
        ),
        Format::Csv => {
            let mut csv = String::from("Work item,Owner,Status,Target date,Comments\r\n");
            for i in 0..8 {
                csv.push_str(&csv_row(&[
                    format!("{} - workstream {:02}", area, i + 1),
                    person(rng),
                    pick(
                        rng,
                        &["Not started", "In progress", "For review", "Complete"],
                    )
                    .into(),
                    format!("2026-{:02}-{:02}", rng.between(1, 12), rng.between(1, 28)),
                    "Synthetic test record - fictional figures and identities".into(),
                ]));
            }
            // A quoted final comments field keeps every record at five columns.
            csv.push_str("\"Synthetic supporting evidence\",\"Synthetic team\",\"For review\",\"2026-10-01\",\"");
            let mut bytes = pad_ascii(csv.into_bytes(), target - 3, TEXT)?;
            bytes.extend_from_slice(b"\"\r\n");
            Ok(bytes)
        }
        Format::Rtf => {
            let body = lines
                .iter()
                .map(|line| {
                    format!(
                        "{}\\par\n",
                        line.replace('\\', "\\\\")
                            .replace('{', "\\{")
                            .replace('}', "\\}")
                    )
                })
                .collect::<String>();
            let mut bytes = pad_ascii(
                format!(
                    "{{\\rtf1\\ansi\\deff0 {{\\fonttbl {{\\f0 Calibri;}}}}\\f0\\fs22 {}",
                    body
                )
                .into_bytes(),
                target - 1,
                b"Synthetic supporting note: confirm outcomes and review actions.\\par\n",
            )?;
            bytes.push(b'}');
            Ok(bytes)
        }
    }
}

fn size_cycle(
    count: usize,
    minimum: usize,
    average: usize,
    maximum: usize,
    rng: &mut Rng,
) -> Vec<usize> {
    let mut remaining = average * count;
    let mut sizes = Vec::with_capacity(count);
    for i in 0..count {
        let after = count - i - 1;
        let low = minimum.max(remaining.saturating_sub(after * maximum));
        let high = maximum.min(remaining - after * minimum);
        let size = rng.between(low, high);
        sizes.push(size);
        remaining -= size;
    }
    for n in (1..sizes.len()).rev() {
        let j = rng.range(n + 1);
        sizes.swap(n, j);
    }
    sizes
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub completed: usize,
    pub bytes: u64,
    pub output: PathBuf,
}
#[derive(Debug)]
pub struct Report {
    pub completed: usize,
    pub bytes: u64,
    pub output: PathBuf,
    pub cancelled: bool,
}

fn new_run_folder(parent: &Path, seed: u64) -> Result<PathBuf, String> {
    fs::create_dir_all(parent).map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    for n in 1..=100_000 {
        let name = if n == 1 {
            format!("Dataset-{seed}")
        } else {
            format!("Dataset-{seed}-{n:03}")
        };
        let path = parent.join(name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Cannot create {}: {e}", path.display())),
        }
    }
    Err("Too many runs with this seed in the output folder.".into())
}

pub fn generate(
    cfg: &Config,
    cancel: &Arc<AtomicBool>,
    mut on_progress: impl FnMut(Progress),
) -> Result<Report, String> {
    cfg.validate()?;
    let root = new_run_folder(&cfg.output, cfg.seed)?;
    on_progress(Progress {
        completed: 0,
        bytes: 0,
        output: root.clone(),
    });
    generate_into(cfg, &root, cancel, &mut on_progress)
        .map_err(|e| format!("{e}\nPartial dataset: {}", root.display()))
}

fn generate_into(
    cfg: &Config,
    root: &Path,
    cancel: &AtomicBool,
    on_progress: &mut impl FnMut(Progress),
) -> Result<Report, String> {
    use std::io::BufWriter;
    let (minimum, average, maximum) = cfg.size_bounds()?;
    let mut rng = Rng::new(cfg.seed);
    let mut size_rng = Rng::new(cfg.seed.wrapping_add(0x12345678));
    let organisation = pick(&mut rng, ORGS);
    let mut inventory = if cfg.inventory {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("Structure Inventory.csv"))
            .map_err(|e| e.to_string())?;
        let mut writer = BufWriter::with_capacity(262144, file);
        writer.write_all(b"\xef\xbb\xbfRelativePath,FileName,Extension,SizeBytes,FolderDepth,RecordID,BusinessUnit,Department,Team,BusinessProcess,FinancialYear,RecordCategory,Owner,Status,Classification\r\n").map_err(|e| e.to_string())?;
        Some(writer)
    } else {
        None
    };
    let mut directories = HashSet::new();
    let mut completed = 0;
    let mut bytes = 0u64;
    let mut sizes = Vec::new();
    let mut format_cycle = cfg.formats.clone();
    for i in 0..cfg.files {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if i % 15 == 0 {
            sizes = size_cycle(
                (cfg.files - i).min(15),
                minimum,
                average,
                maximum,
                &mut size_rng,
            );
        }
        if i % format_cycle.len() == 0 {
            for n in (1..format_cycle.len()).rev() {
                let j = rng.range(n + 1);
                format_cycle.swap(n, j);
            }
        }
        let format = format_cycle[i % format_cycle.len()];
        let profile = &PROFILES[rng.range(PROFILES.len())];
        let owner = format!("Synthetic {}", person(&mut rng));
        let title = title_for(&mut rng, profile, format);
        let fiscal_start = 2023 + rng.range(4);
        let financial_year = format!("FY{}-{:02}", fiscal_start, (fiscal_start + 1) % 100);
        let category = category_for(title);
        let depth = rng.between(cfg.max_depth.min(3), cfg.max_depth);
        let mut dir = root.join(organisation);
        let segments = [
            profile.division,
            profile.department,
            profile.team,
            profile.process,
            financial_year.as_str(),
            category,
        ];
        for segment in segments.iter().take(depth - 1) {
            dir.push(segment);
        }
        for _ in 7..depth {
            dir.push(pick(&mut rng, FOLDER_KINDS));
        }
        if directories.insert(dir.clone()) {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
        }
        let status = pick(&mut rng, FILE_SUFFIXES);
        let classification = pick(
            &mut rng,
            &[
                "Internal",
                "Confidential - Commercial",
                "Confidential - Personal",
                "Public",
            ],
        );
        let record_id = format!("{}-{}-{:05}", profile.code, fiscal_start, i + 1);
        let path = dir.join(format!(
            "{} - {} - {}.{}",
            record_id,
            safe_name(title),
            status,
            format.extension()
        ));
        let lines = business_text(
            &mut rng,
            organisation,
            profile,
            RecordText {
                title,
                owner: &owner,
                record_id: &record_id,
                status,
                classification,
            },
        );
        let content = make_file(
            format,
            &lines,
            &mut rng,
            profile.department,
            profile.process,
            sizes[i % 15],
        )?;
        let size = content.len();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
        if let Err(e) = file.write_all(&content) {
            drop(file);
            // Remove only the incomplete file created by this run.
            let _ = fs::remove_file(&path);
            return Err(format!("Cannot write {}: {e}", path.display()));
        }
        if let Some(writer) = inventory.as_mut() {
            let row = csv_row(&[
                path.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                format.extension().into(),
                size.to_string(),
                depth.to_string(),
                record_id,
                profile.division.into(),
                profile.department.into(),
                profile.team.into(),
                profile.process.into(),
                financial_year,
                category.into(),
                owner,
                status.into(),
                classification.into(),
            ]);
            writer
                .write_all(row.as_bytes())
                .map_err(|e| format!("Cannot write inventory: {e}"))?;
        }
        completed += 1;
        bytes += size as u64;
        on_progress(Progress {
            completed,
            bytes,
            output: root.to_path_buf(),
        });
    }
    if let Some(mut writer) = inventory {
        writer
            .flush()
            .map_err(|e| format!("Cannot finish inventory: {e}"))?;
    }
    Ok(Report {
        completed,
        bytes,
        output: root.to_path_buf(),
        cancelled: completed < cfg.files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn cycles_balance_all_counts_and_bounds() {
        let mut rng = Rng::new(42);
        for count in 1..=15 {
            for avg in [100_000, 100_001, 4_800_000, 14_999_999, 15_000_000] {
                for _ in 0..20 {
                    let sizes = size_cycle(count, 100_000, avg, 15_000_000, &mut rng);
                    assert_eq!(sizes.iter().sum::<usize>(), count * avg);
                    assert!(sizes.iter().all(|s| (100_000..=15_000_000).contains(s)));
                }
            }
        }
    }
    #[test]
    fn formats_are_exact_and_zip_crc_is_readable() {
        for format in [
            Format::Docx,
            Format::Xlsx,
            Format::Pptx,
            Format::Csv,
            Format::Rtf,
            Format::Txt,
        ] {
            for target in [100_000, 1_000_000, 15_000_000] {
                let lines = vec![
                    "Synthetic organisation".into(),
                    "Synthetic business update".into(),
                    "Text & <test> \\ {data}".into(),
                ];
                let data = make_file(
                    format,
                    &lines,
                    &mut Rng::new(42),
                    "Finance",
                    "Annual Budget",
                    target,
                )
                .unwrap();
                assert_eq!(data.len(), target, "{format:?}");
                if matches!(format, Format::Docx | Format::Xlsx | Format::Pptx) {
                    let mut archive = zip::ZipArchive::new(Cursor::new(data)).unwrap();
                    for i in 0..archive.len() {
                        let mut entry = archive.by_index(i).unwrap();
                        let mut content = Vec::new();
                        entry.read_to_end(&mut content).unwrap();
                    }
                } else {
                    assert!(std::str::from_utf8(&data).is_ok());
                }
            }
        }
    }
    #[test]
    fn zero_seed_is_distinct_and_repeatable() {
        let mut a = Rng::new(0);
        let mut b = Rng::new(0);
        let mut c = Rng::new(1);
        assert_ne!(a.next(), c.next());
        a = Rng::new(0);
        for _ in 0..100 {
            assert_eq!(a.next(), b.next());
        }
    }
}
