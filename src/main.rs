#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use eframe::egui;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format { Docx, Xlsx, Pptx, Csv, Rtf, Txt }

impl Format {
    fn extension(self) -> &'static str {
        match self { Self::Docx => "docx", Self::Xlsx => "xlsx", Self::Pptx => "pptx", Self::Csv => "csv", Self::Rtf => "rtf", Self::Txt => "txt" }
    }
}

#[derive(Clone)]
struct Config { output: PathBuf, files: usize, max_depth: usize, seed: u64, formats: Vec<Format>, inventory: bool }

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed.max(1)) }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0
    }
    fn range(&mut self, n: usize) -> usize { if n == 0 { 0 } else { (self.next() as usize) % n } }
    fn between(&mut self, low: usize, high: usize) -> usize { low + self.range(high - low + 1) }
}

const FOLDER_KINDS: &[&str] = &["Working Documents", "Approvals", "Meeting Papers", "Reference Material", "Drafts", "Final Deliverables", "Correspondence", "Reports", "Supporting Evidence", "Archive Copy"];
const ORGS: &[&str] = &["Northstar Services", "Harbour Group", "Summit Operations", "Cedar Consulting", "Bluewater Partners", "Westfield Support", "Meridian Works", "Kauri Advisory"];
const FIRST: &[&str] = &[
    // Māori
    "Manu", "Aroha", "Wiremu", "Moana", "Tane", "Hine", "Rawiri", "Mere",

    // Pasifika
    "Sione", "Losa", "Mele", "Tavita", "Faalogo", "Ana", "Peni",

    // Indian
    "Navin", "Jaspreet", "Priya", "Arjun", "Rohit", "Anika", "Kiran", "Deepa",

    // Chinese
    "Yuki", "Wei", "Jun", "Li", "Mei", "Jia", "Xinyi",

    // Korean
    "Minjun", "Jisoo", "Hyun", "Seoyeon",

    // Filipino
    "Jose", "Maria", "Carlo", "Angelica",

    // Anglo / general NZ
    "Alex", "Morgan", "Jamie", "Taylor", "Sam", "Jordan", "Casey",
    "Anne", "Avery", "Mat", "Robin", "Cameron", "Charlotte", "Olivia",
    "Liam", "Noah", "Jack", "Ella", "Amelia", "Sophie", "George",
];

const LAST: &[&str] = &[
    // Māori
    "Ngata", "Te Aho", "Rangihuna", "Kingi", "Tahana", "Wikiriwhi",

    // Pasifika
    "Tupou", "Vaai", "Tuiloma", "Samu", "Fifita", "Latu",

    // Indian
    "Patel", "Singh", "Kaur", "Kumar", "Sharma", "Chand",

    // Chinese
    "Chen", "Wong", "Zhang", "Liu", "Lin", "Huang",

    // Korean
    "Kim", "Lee", "Park", "Choi",

    // Filipino
    "Santos", "Reyes", "Garcia", "Cruz",

    // Anglo / general NZ
    "Smith", "Jones", "Brown", "Wilson", "Taylor", "Martin",
    "Thompson", "Wright", "Mason", "Reed", "Walker", "McKenzie",
    "Anderson", "Campbell", "Young",
];
const FILE_SUFFIXES: &[&str] = &["Draft", "For Review", "Approved", "Working Copy", "Final", "Updated", "Signed Off"];

struct BusinessProfile { division: &'static str, department: &'static str, team: &'static str, process: &'static str, code: &'static str }
const PROFILES: &[BusinessProfile] = &[
    BusinessProfile { division: "Corporate Services", department: "Finance", team: "Financial Planning", process: "Annual Budget", code: "FIN" },
    BusinessProfile { division: "Corporate Services", department: "Finance", team: "Financial Control", process: "Month End Close", code: "FIN" },
    BusinessProfile { division: "Corporate Services", department: "People and Culture", team: "Recruitment", process: "Workforce Planning", code: "PPL" },
    BusinessProfile { division: "Corporate Services", department: "People and Culture", team: "Employee Services", process: "Policy Review", code: "PPL" },
    BusinessProfile { division: "Digital and Technology", department: "Information Services", team: "Business Applications", process: "Customer Platform Renewal", code: "ICT" },
    BusinessProfile { division: "Digital and Technology", department: "Information Services", team: "Service Operations", process: "Service Continuity", code: "ICT" },
    BusinessProfile { division: "Digital and Technology", department: "Information Services", team: "Information Management", process: "Records Review", code: "IM" },
    BusinessProfile { division: "Operations", department: "Customer Services", team: "Contact Centre", process: "Customer Experience Review", code: "OPS" },
    BusinessProfile { division: "Operations", department: "Service Delivery", team: "Field Operations", process: "Service Improvement", code: "OPS" },
    BusinessProfile { division: "Operations", department: "Property and Facilities", team: "Asset Management", process: "Facilities Maintenance", code: "AST" },
    BusinessProfile { division: "Commercial Services", department: "Procurement", team: "Supplier Management", process: "Supplier Transition", code: "PRC" },
    BusinessProfile { division: "Commercial Services", department: "Procurement", team: "Sourcing", process: "Contract Renewal", code: "PRC" },
    BusinessProfile { division: "Governance and Risk", department: "Governance", team: "Committee Support", process: "Board Reporting", code: "GOV" },
    BusinessProfile { division: "Governance and Risk", department: "Legal Services", team: "Advisory", process: "Information Request Review", code: "LEG" },
    BusinessProfile { division: "Governance and Risk", department: "Risk and Assurance", team: "Operational Risk", process: "Risk Assessment", code: "RSK" },
    BusinessProfile { division: "Communications", department: "Communications", team: "Internal Communications", process: "Change Communications", code: "COM" },
];

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;") }
fn pick<'a>(rng: &mut Rng, values: &'a [&'a str]) -> &'a str { values[rng.range(values.len())] }
fn person(rng: &mut Rng) -> String { format!("{} {}", pick(rng, FIRST), pick(rng, LAST)) }
fn business_text(rng: &mut Rng, org: &str, division: &str, dept: &str, team: &str, area: &str, title: &str, owner: &str, record_id: &str, status: &str, classification: &str) -> Vec<String> {
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

fn csv_cell(value: &str) -> String { format!("\"{}\"", value.replace('"', "\"\"")) }
fn csv_row(values: &[String]) -> String { format!("{}\r\n", values.iter().map(|value| csv_cell(value)).collect::<Vec<_>>().join(",")) }

fn write_zip(entries: &[(&str, String)], payload: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, contents) in entries {
        zip.start_file(*name, options).map_err(|e| e.to_string())?;
        zip.write_all(contents.as_bytes()).map_err(|e| e.to_string())?;
    }
    if let Some(bytes) = payload {
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("customData/synthetic-payload.bin", stored).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map(|c| c.into_inner()).map_err(|e| e.to_string())
}

fn zip_package(mut entries: Vec<(&str, String)>, target_bytes: usize, rng: &mut Rng) -> Result<Vec<u8>, String> {
    // Keep the requested size in a harmless, uncompressed package part. This
    // keeps the Office XML readable and the final archive size predictable.
    if let Some((_, types)) = entries.iter_mut().find(|(name, _)| *name == "[Content_Types].xml") {
        types.insert_str(types.find("</Types>").unwrap_or(types.len()), "<Default Extension=\"bin\" ContentType=\"application/octet-stream\"/>");
    }
    let base = write_zip(&entries, None)?;
    let member = "customData/synthetic-payload.bin";
    let overhead = 30 + member.len() + 46 + member.len();
    let payload_len = target_bytes.saturating_sub(base.len() + overhead);
    let mut payload = Vec::with_capacity(payload_len);
    while payload.len() < payload_len {
        let n = rng.next();
        for byte in n.to_le_bytes() {
            if payload.len() == payload_len { break; }
            payload.push(byte);
        }
    }
    write_zip(&entries, Some(&payload))
}

fn pad_text(mut text: String, target_bytes: usize, rng: &mut Rng) -> Vec<u8> {
    const SENTENCES: &[&str] = &[
        "The service owner will confirm the agreed outcome before the next review.",
        "Supporting evidence has been filed with the relevant workstream record.",
        "The project team will check the dependency and provide an update at the next meeting.",
        "Actions remain subject to review of timing, resourcing and operational impact.",
        "The current position reflects information available at the time this synthetic record was created.",
        "Please refer to the approved process and retain the decision with the project documentation.",
    ];
    while text.len() < target_bytes {
        text.push_str(pick(rng, SENTENCES));
        text.push_str(" Reference: ");
        text.push_str(&format!("WS-{:04}-{:02}", rng.range(10000), rng.range(100)));
        text.push_str(".\r\n");
    }
    text.into_bytes()
}

fn docx(lines: &[String], target_bytes: usize, rng: &mut Rng) -> Result<Vec<u8>, String> {
    let body = lines.iter().enumerate().map(|(index, line)| {
        if line.is_empty() { "<w:p/>".to_string() }
        else if index == 0 { format!("<w:p><w:r><w:rPr><w:b/><w:sz w:val=\"20\"/></w:rPr><w:t>{}</w:t></w:r></w:p>", esc(line)) }
        else if index == 1 { format!("<w:p><w:r><w:rPr><w:b/><w:sz w:val=\"32\"/></w:rPr><w:t>{}</w:t></w:r></w:p>", esc(line)) }
        else if ["Purpose", "Executive summary", "Current position", "Key considerations", "Actions"].contains(&line.as_str()) { format!("<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>{}</w:t></w:r></w:p>", esc(line)) }
        else { format!("<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", esc(line)) }
    }).collect::<String>();
    let document = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr></w:body></w:document>", body);
    zip_package(vec![
        ("[Content_Types].xml", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>".into()),
        ("_rels/.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>".into()),
        ("word/document.xml", document),
    ], target_bytes, rng)
}

fn xlsx(rng: &mut Rng, dept: &str, area: &str, target_bytes: usize) -> Result<Vec<u8>, String> {
    let sheet_name = "Status and Actions";
    let mut rows: Vec<Vec<String>> = vec![
        vec!["Work item".into(), "Owner".into(), "Status".into(), "Target date".into(), "Comments".into()],
    ];
    for i in 0..8 {
        rows.push(vec![format!("{} - workstream {:02}", area, i + 1), person(rng), pick(rng, &["Not started", "In progress", "For review", "Complete"]).into(), format!("2026-{:02}-{:02}", rng.between(1, 12), rng.between(1, 28)), pick(rng, &["Progress update due", "Dependency being confirmed", "Evidence filed", "Review at next meeting"]).into()]);
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheetData>");
    for (ri, row) in rows.iter().enumerate() {
        xml.push_str(&format!("<row r=\"{}\">", ri + 1));
        for (ci, value) in row.iter().enumerate() {
            let col = (b'A' + ci as u8) as char;
            xml.push_str(&format!("<c r=\"{}{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", col, ri + 1, esc(value)));
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
    let title = lines.get(1).cloned().unwrap_or_else(|| "Business Update".into());
    let content = lines.iter().skip(2).filter(|s| !s.is_empty()).take(6).cloned().collect::<Vec<_>>();
    let mut shapes = String::new();
    for (i, line) in content.iter().enumerate() {
        let y = 1500000 + (i as i64 * 650000);
        shapes.push_str(&format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Text {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"900000\" y=\"{}\"/><a:ext cx=\"10500000\" cy=\"420000\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-NZ\" sz=\"1800\"/><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-NZ\"/></a:p></p:txBody></p:sp>", i + 2, i + 2, y, esc(line)));
    }
    let slide = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"900000\" y=\"400000\"/><a:ext cx=\"10500000\" cy=\"800000\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-NZ\" sz=\"2800\" b=\"1\"/><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-NZ\"/></a:p></p:txBody></p:sp>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>", esc(&title), shapes);
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slides/slide1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/><Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/><Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/><Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/></Types>";
    let pres = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id=\"256\" r:id=\"rId2\"/></p:sldIdLst><p:sldSz cx=\"9144000\" cy=\"5143500\" type=\"screen16x9\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>";
    let master = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" bg1=\"lt1\" bg2=\"lt2\" tx1=\"dk1\" tx2=\"dk2\" hlink=\"hlink\" folHlink=\"folHlink\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"1\" r:id=\"rId1\"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>";
    let layout = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" type=\"blank\"><p:cSld name=\"Blank\"><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>";
    let theme = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Synthetic Business Theme\"><a:themeElements><a:clrScheme name=\"Synthetic\"><a:dk1><a:srgbClr val=\"000000\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1><a:dk2><a:srgbClr val=\"1F2937\"/></a:dk2><a:lt2><a:srgbClr val=\"F3F4F6\"/></a:lt2><a:accent1><a:srgbClr val=\"2563EB\"/></a:accent1><a:accent2><a:srgbClr val=\"059669\"/></a:accent2><a:accent3><a:srgbClr val=\"D97706\"/></a:accent3><a:accent4><a:srgbClr val=\"7C3AED\"/></a:accent4><a:accent5><a:srgbClr val=\"DB2777\"/></a:accent5><a:accent6><a:srgbClr val=\"0891B2\"/></a:accent6><a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink><a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink></a:clrScheme><a:fontScheme name=\"Synthetic\"><a:majorFont><a:latin typeface=\"Aptos Display\"/></a:majorFont><a:minorFont><a:latin typeface=\"Aptos\"/></a:minorFont></a:fontScheme><a:fmtScheme name=\"Synthetic\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln><a:ln w=\"12700\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln><a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>";
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
    input.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '-' } else { c }).collect()
}

fn title_for(rng: &mut Rng, profile: &BusinessProfile, format: Format) -> &'static str {
    let choices: &[&str] = match format {
        Format::Xlsx | Format::Csv => match profile.code {
            "FIN" => &["Annual Budget Forecast", "Monthly Variance Report", "Cost Centre Summary", "Month End Reconciliation"],
            "PRC" => &["Supplier Evaluation Register", "Contract Renewal Schedule", "Procurement Action Register", "Supplier Performance Review"],
            "RSK" => &["Risk and Issue Register", "Control Assessment Summary", "Assurance Actions Register"],
            _ => &["Service Performance Report", "Project Action Register", "Benefits Tracking Summary", "Workstream Status Report"],
        },
        Format::Pptx => &["Steering Committee Update", "Project Board Briefing", "Service Performance Overview", "Quarterly Business Review", "Change Programme Update"],
        _ => match profile.code {
            "FIN" => &["Budget Planning Brief", "Financial Control Procedure", "Month End Review", "Finance Decision Record"],
            "PPL" => &["Workforce Planning Brief", "Recruitment Campaign Plan", "People Policy Review", "Employee Services Update"],
            "ICT" | "IM" => &["Service Transition Plan", "System Requirements Summary", "Change Impact Assessment", "Service Continuity Procedure"],
            "PRC" => &["Supplier Review Summary", "Contract Renewal Plan", "Procurement Decision Record", "Request for Proposal Evaluation"],
            "GOV" | "LEG" | "RSK" => &["Steering Committee Minutes", "Decision Record", "Risk and Issue Review", "Policy Review"],
            "COM" => &["Change Communications Plan", "Stakeholder Briefing", "Communications Approval Record", "Engagement Summary"],
            _ => &["Service Improvement Plan", "Standard Operating Procedure", "Customer Feedback Analysis", "Quality Review Findings", "Project Status Report"],
        },
    };
    pick(rng, choices)
}

fn category_for(title: &str) -> &'static str {
    if title.contains("Minutes") || title.contains("Committee") || title.contains("Board") { "Meeting Papers" }
    else if title.contains("Budget") || title.contains("Variance") || title.contains("Forecast") { "Financial Planning" }
    else if title.contains("Risk") || title.contains("Control") || title.contains("Assurance") { "Risk and Assurance" }
    else if title.contains("Supplier") || title.contains("Contract") || title.contains("Procurement") { "Supplier Management" }
    else if title.contains("Procedure") || title.contains("Policy") { "Policies and Procedures" }
    else if title.contains("Register") || title.contains("Schedule") || title.contains("Tracking") { "Registers and Schedules" }
    else { "Reports and Briefings" }
}

fn make_file(format: Format, lines: &[String], rng: &mut Rng, dept: &str, area: &str, target_bytes: usize) -> Result<Vec<u8>, String> {
    match format {
        Format::Docx => docx(lines, target_bytes, rng), Format::Xlsx => xlsx(rng, dept, area, target_bytes), Format::Pptx => pptx(lines, target_bytes, rng),
        Format::Txt => Ok(pad_text(format!("{}\r\n\r\nGenerated synthetic business test content.\r\n", lines.join("\r\n")), target_bytes, rng)),
        Format::Csv => {
            let mut csv = String::from("Work item,Owner,Status,Target date,Comments\r\n");
            for i in 0..8 { csv.push_str(&format!("\"{} - workstream {:02}\",\"{}\",\"{}\",2026-{:02}-{:02},\"{}\"\r\n", area, i + 1, person(rng), pick(rng, &["Not started", "In progress", "For review", "Complete"]), rng.between(1, 12), rng.between(1, 28), pick(rng, &["Progress update due", "Dependency being confirmed", "Evidence filed", "Review at next meeting"]))); }
            Ok(pad_text(csv, target_bytes, rng))
        }
        Format::Rtf => {
            let body = lines.iter().map(|line| format!("{}\\par\n", line.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}"))).collect::<String>();
            Ok(pad_text(format!("{{\\rtf1\\ansi\\deff0 {{\\fonttbl {{\\f0 Aptos;}}}}\\f0\\fs22 {}", body), target_bytes.saturating_sub(2), rng).into_iter().chain(b" }}".iter().copied()).collect())
        }
    }
}

fn create_tree(root: &Path, cfg: &Config, rng: &mut Rng, mut on_progress: impl FnMut(usize, usize, usize)) -> Result<usize, String> {
    let organisation = pick(rng, ORGS);
    let organisation_root = root.join(organisation);
    fs::create_dir_all(&organisation_root).map_err(|e| format!("Cannot create {}: {}", organisation_root.display(), e))?;
    let mut inventory_csv = csv_row(&[
        "RelativeFolderPath", "FileName", "Extension", "SizeBytes", "FolderDepth", "RecordID", "BusinessUnit", "Department", "Team", "BusinessProcess", "FiscalYear", "RecordCategory", "Owner", "Status", "Classification",
    ].iter().map(|value| value.to_string()).collect::<Vec<_>>());
    let mut total_bytes = 0usize;
    // Each shuffled 15-file cycle totals exactly 72 MB, giving a 4.8 MB mean.
    // Half-megabyte increments provide a smoother spread, with both endpoints.
    const SIZE_TENTHS_MB: [usize; 15] = [10, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 70, 105, 150];
    let mut size_cycle = SIZE_TENTHS_MB;
    for i in 0..cfg.files {
        if i % size_cycle.len() == 0 {
            for n in (1..size_cycle.len()).rev() { let j = rng.range(n + 1); size_cycle.swap(n, j); }
        }
        let profile = &PROFILES[rng.range(PROFILES.len())];
        let dept = profile.department;
        let area = profile.process;
        let owner = person(rng);
        let format = cfg.formats[rng.range(cfg.formats.len())];
        let title = title_for(rng, profile, format);
        let mut dir = organisation_root.clone();
        let fiscal_start = 2023 + rng.range(4);
        let fiscal_year = format!("FY{}-{:02}", fiscal_start, (fiscal_start + 1) % 100);
        let record_category = category_for(title);
        let path_segments: [&str; 6] = [profile.division, profile.department, profile.team, area, fiscal_year.as_str(), record_category];
        let depth_floor = cfg.max_depth.min(3);
        let chosen_depth = rng.between(depth_floor, cfg.max_depth);
        for segment in path_segments.iter().take(chosen_depth.min(path_segments.len())) { dir.push(*segment); }
        for _ in path_segments.len()..chosen_depth { dir.push(pick(rng, FOLDER_KINDS)); }
        fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
        let suffix = pick(rng, FILE_SUFFIXES);
        let classification = pick(rng, &["Internal", "Confidential - Commercial", "Confidential - Personal", "Public"]);
        let record_id = format!("{}-{}-{:05}", profile.code, fiscal_start, i + 1);
        let stem = format!("{} - {} - {}", record_id, safe_name(title), suffix);
        let file_path = dir.join(format!("{}.{}", stem, format.extension()));
        let lines = business_text(rng, organisation, profile.division, dept, profile.team, area, title, &owner, &record_id, suffix, classification);
        let target_bytes = size_cycle[i % size_cycle.len()] * 100_000;
        let bytes = make_file(format, &lines, rng, dept, area, target_bytes).map_err(|e| format!("Failed to create {}: {}", file_path.display(), e))?;
        let file_size = bytes.len();
        total_bytes += file_size;
        fs::write(&file_path, bytes).map_err(|e| format!("Cannot write {}: {}", file_path.display(), e))?;
        if cfg.inventory {
            let relative_folder = dir.strip_prefix(root).unwrap_or(&dir).display().to_string();
            let values = vec![
                relative_folder, file_path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(), format.extension().to_string(),
                file_size.to_string(), chosen_depth.to_string(), record_id, profile.division.to_string(), dept.to_string(),
                profile.team.to_string(), area.to_string(), fiscal_year, record_category.to_string(), owner, suffix.to_string(), classification.to_string(),
            ];
            inventory_csv.push_str(&csv_row(&values));
        }
        on_progress(i + 1, cfg.files, total_bytes);
    }
    if cfg.inventory {
        let manifest_path = root.join(format!("{} - Structure Inventory.csv", organisation));
        fs::write(&manifest_path, inventory_csv).map_err(|e| format!("Cannot write {}: {}", manifest_path.display(), e))?;
    }
    Ok(total_bytes)
}

enum WorkerMessage {
    Progress { completed: usize, total: usize, bytes: usize },
    Finished { bytes: usize },
    Failed(String),
}

struct GeneratorApp {
    output: String,
    files: usize,
    max_depth: usize,
    seed: String,
    formats: [bool; 6],
    inventory: bool,
    working: bool,
    progress: usize,
    message: String,
    receiver: Option<Receiver<WorkerMessage>>,
}

impl Default for GeneratorApp {
    fn default() -> Self {
        Self {
            output: std::env::var_os("USERPROFILE")
                .map(|profile| PathBuf::from(profile).join("Documents").join("MiPoint Dummy Migration Set").display().to_string())
                .unwrap_or_else(|| "generated_migration_set".to_string()),
            files: 50,
            max_depth: 7,
            seed: String::new(),
            formats: [true; 6],
            inventory: true,
            working: false,
            progress: 0,
            message: "Ready to generate a migration test set.".to_string(),
            receiver: None,
        }
    }
}

impl GeneratorApp {
    fn start_generation(&mut self, ctx: &egui::Context) {
        let output = self.output.trim();
        if output.is_empty() {
            self.message = "Choose an output folder first.".to_string();
            return;
        }
        let formats: Vec<Format> = [Format::Docx, Format::Xlsx, Format::Pptx, Format::Csv, Format::Rtf, Format::Txt]
            .into_iter().zip(self.formats).filter_map(|(format, selected)| selected.then_some(format)).collect();
        if formats.is_empty() {
            self.message = "Select at least one file type.".to_string();
            return;
        }
        let seed_text = self.seed.trim();
        let seed = if seed_text.is_empty() {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64
        } else {
            match seed_text.parse::<u64>() {
                Ok(seed) => seed,
                Err(_) => { self.message = "Seed must contain only digits, or be left blank.".to_string(); return; }
            }
        };
        let config = Config { output: PathBuf::from(output), files: self.files.max(1), max_depth: self.max_depth.clamp(1, 12), seed, formats, inventory: self.inventory };
        let total_files = config.files;
        let (sender, receiver) = mpsc::channel();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let mut rng = Rng::new(config.seed);
            let result = create_tree(&config.output, &config, &mut rng, |completed, total, bytes| {
                let _ = sender.send(WorkerMessage::Progress { completed, total, bytes });
                repaint.request_repaint();
            });
            match result {
                Ok(bytes) => { let _ = sender.send(WorkerMessage::Finished { bytes }); }
                Err(error) => { let _ = sender.send(WorkerMessage::Failed(error)); }
            }
            repaint.request_repaint();
        });
        self.receiver = Some(receiver);
        self.working = true;
        self.progress = 0;
        self.message = format!("Generating {} files with seed {}...", total_files, seed);
    }

    fn receive_worker_messages(&mut self) {
        let messages: Vec<WorkerMessage> = self.receiver.as_ref().map(|rx| rx.try_iter().collect()).unwrap_or_default();
        for message in messages {
            match message {
                WorkerMessage::Progress { completed, total, bytes } => {
                    self.progress = completed;
                    self.message = format!("Generating file {} of {}... {:.0} MB written", completed, total, bytes as f64 / 1_000_000.0);
                }
                WorkerMessage::Finished { bytes } => {
                    self.progress = self.files;
                    let average = bytes as f64 / self.files.max(1) as f64 / 1_000_000.0;
                    let inventory_note = if self.inventory { " Companion structure inventory CSV created." } else { "" };
                    self.message = format!("Done. {} files, {:.2} MB total, averaging {:.2} MB per file.{}", self.files, bytes as f64 / 1_000_000.0, average, inventory_note);
                    self.working = false;
                    self.receiver = None;
                }
                WorkerMessage::Failed(error) => {
                    self.message = format!("Generation stopped: {}", error);
                    self.working = false;
                    self.receiver = None;
                }
            }
        }
    }
}

impl eframe::App for GeneratorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive_worker_messages();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("MiPoint Dummy Migration Set Generator");
            ui.label("Create realistic synthetic business folders and Office files for migration testing.");
            ui.add_space(12.0);

            ui.group(|ui| {
                ui.heading("Output");
                ui.horizontal(|ui| {
                    ui.label("Save files to");
                    ui.add_enabled(!self.working, egui::TextEdit::singleline(&mut self.output).desired_width(430.0));
                    if ui.add_enabled(!self.working, egui::Button::new("Browse..." )).clicked() {
                        let initial = PathBuf::from(self.output.trim());
                        let dialog = if initial.is_dir() { rfd::FileDialog::new().set_directory(initial) } else { rfd::FileDialog::new() };
                        if let Some(path) = dialog.pick_folder() { self.output = path.display().to_string(); }
                    }
                });
                ui.small("Files are created in this folder. Existing files with matching names may be overwritten.");
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.heading("Dataset size");
                    ui.horizontal(|ui| { ui.label("Number of files"); ui.add_enabled(!self.working, egui::DragValue::new(&mut self.files).range(1..=100_000).speed(1)); });
                    ui.horizontal(|ui| { ui.label("Maximum folder depth"); ui.add_enabled(!self.working, egui::DragValue::new(&mut self.max_depth).range(1..=12).speed(1)); });
                    ui.horizontal(|ui| {
                        ui.label("Seed (optional)");
                        ui.add_enabled(!self.working, egui::TextEdit::singleline(&mut self.seed).desired_width(150.0).hint_text("Random each run"));
                    });
                    ui.small("A supplied seed makes a run repeatable. Leave blank for a new random seed.");
                    ui.add_enabled(!self.working, egui::Checkbox::new(&mut self.inventory, "Create companion structure inventory CSV"));
                });
                ui.group(|ui| {
                    ui.heading("File types");
                    ui.add_enabled_ui(!self.working, |ui| {
                        ui.horizontal(|ui| { ui.checkbox(&mut self.formats[0], "Word (.docx)"); ui.checkbox(&mut self.formats[1], "Excel (.xlsx)"); });
                        ui.horizontal(|ui| { ui.checkbox(&mut self.formats[2], "PowerPoint (.pptx)"); ui.checkbox(&mut self.formats[3], "CSV (.csv)"); });
                        ui.horizontal(|ui| { ui.checkbox(&mut self.formats[4], "Rich Text (.rtf)"); ui.checkbox(&mut self.formats[5], "Text (.txt)"); });
                    });
                });
            });

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.heading("File sizes");
                ui.label("Sizes range from 1 MB to 15 MB. Each complete 15-file group averages 4.8 MB per file.");
                ui.label(format!("Estimated total for this run: about {:.0} MB.", self.files as f64 * 4.8));
                ui.label("Word, Excel and PowerPoint output is generated in modern Open XML format.");
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.working, egui::Button::new("Generate files")).clicked() { self.start_generation(ui.ctx()); }
                if self.working { ui.spinner(); }
                ui.label(&self.message);
            });
            if self.working {
                let fraction = self.progress as f32 / self.files.max(1) as f32;
                ui.add(egui::ProgressBar::new(fraction).show_percentage().text(format!("{} / {} files", self.progress, self.files)));
            }
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([850.0, 600.0]).with_min_inner_size([740.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MiPoint Dummy Migration Set Generator",
        options,
        Box::new(|_creation_context| Ok(Box::<GeneratorApp>::default())),
    )
}
