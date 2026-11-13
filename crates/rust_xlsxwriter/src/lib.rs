use quick_xml::escape::escape;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug)]
pub struct XlsxError(String);

impl fmt::Display for XlsxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "XLSX 생성 오류: {}", self.0)
    }
}

impl std::error::Error for XlsxError {}

impl From<std::io::Error> for XlsxError {
    fn from(err: std::io::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<zip::result::ZipError> for XlsxError {
    fn from(err: zip::result::ZipError) -> Self {
        Self(err.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Format {
    num_format: Option<String>,
    bold: bool,
    align: Option<FormatAlign>,
}

impl Format {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_num_format(mut self, num_format: impl Into<String>) -> Self {
        self.num_format = Some(num_format.into());
        self
    }

    pub fn set_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn set_align(mut self, align: FormatAlign) -> Self {
        self.align = Some(align);
        self
    }
}

#[derive(Debug, Clone)]
enum CellValue {
    Text(String),
    Number(f64),
}

#[derive(Debug, Clone)]
struct CellData {
    value: CellValue,
    format: Option<Format>,
}

#[derive(Debug, Clone)]
pub struct Worksheet {
    name: String,
    cells: BTreeMap<(u32, u16), CellData>,
    column_widths: BTreeMap<u16, f64>,
    column_formats: BTreeMap<u16, Format>,
}

impl Worksheet {
    fn new(name: String) -> Self {
        Self {
            name,
            cells: BTreeMap::new(),
            column_widths: BTreeMap::new(),
            column_formats: BTreeMap::new(),
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> Result<&mut Self, XlsxError> {
        let name = name.into();
        if name.trim().is_empty() || name.chars().count() > 31 {
            return Err(XlsxError(format!(
                "시트 이름은 1~31자여야 합니다: '{name}'"
            )));
        }
        self.name = name;
        Ok(self)
    }

    pub fn set_column_width(&mut self, col: u16, width: f64) -> Result<&mut Self, XlsxError> {
        self.column_widths.insert(col, width.max(1.0));
        Ok(self)
    }

    pub fn set_column_format(&mut self, col: u16, format: &Format) -> Result<&mut Self, XlsxError> {
        self.column_formats.insert(col, format.clone());
        Ok(self)
    }

    pub fn write_string(
        &mut self,
        row: u32,
        col: u16,
        text: impl AsRef<str>,
    ) -> Result<&mut Self, XlsxError> {
        self.cells.insert(
            (row, col),
            CellData {
                value: CellValue::Text(text.as_ref().to_string()),
                format: None,
            },
        );
        Ok(self)
    }

    pub fn write_string_with_format(
        &mut self,
        row: u32,
        col: u16,
        text: impl AsRef<str>,
        format: &Format,
    ) -> Result<&mut Self, XlsxError> {
        self.cells.insert(
            (row, col),
            CellData {
                value: CellValue::Text(text.as_ref().to_string()),
                format: Some(format.clone()),
            },
        );
        Ok(self)
    }

    pub fn write_number(
        &mut self,
        row: u32,
        col: u16,
        number: f64,
    ) -> Result<&mut Self, XlsxError> {
        if !number.is_finite() {
            return Err(XlsxError("유한하지 않은 숫자는 기록할 수 없습니다.".to_string()));
        }
        self.cells.insert(
            (row, col),
            CellData {
                value: CellValue::Number(number),
                format: None,
            },
        );
        Ok(self)
    }

    pub fn write_number_with_format(
        &mut self,
        row: u32,
        col: u16,
        number: f64,
        format: &Format,
    ) -> Result<&mut Self, XlsxError> {
        if !number.is_finite() {
            return Err(XlsxError("유한하지 않은 숫자는 기록할 수 없습니다.".to_string()));
        }
        self.cells.insert(
            (row, col),
            CellData {
                value: CellValue::Number(number),
                format: Some(format.clone()),
            },
        );
        Ok(self)
    }
}

#[derive(Debug, Default)]
pub struct Workbook {
    worksheets: Vec<Worksheet>,
}

impl Workbook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_worksheet(&mut self) -> &mut Worksheet {
        let idx = self.worksheets.len() + 1;
        self.worksheets.push(Worksheet::new(format!("Sheet{idx}")));
        self.worksheets.last_mut().expect("worksheet pushed")
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), XlsxError> {
        let bytes = self.save_to_buffer()?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    pub fn save_to_buffer(&self) -> Result<Vec<u8>, XlsxError> {
        let sheets: Vec<Worksheet> = if self.worksheets.is_empty() {
            vec![Worksheet::new("Sheet1".to_string())]
        } else {
            self.worksheets.clone()
        };

        let mut styles = StyleRegistry::new();
        for ws in &sheets {
            for fmt in ws.column_formats.values() {
                styles.register(fmt);
            }
            for cell in ws.cells.values() {
                if let Some(ref fmt) = cell.format {
                    styles.register(fmt);
                }
            }
        }

        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        write_zip_entry(&mut zip, "[Content_Types].xml", options, &build_content_types(sheets.len()))?;
        write_zip_entry(&mut zip, "_rels/.rels", options, &build_root_rels())?;
        write_zip_entry(&mut zip, "xl/workbook.xml", options, &build_workbook_xml(&sheets))?;
        write_zip_entry(&mut zip, "xl/_rels/workbook.xml.rels", options, &build_workbook_rels(sheets.len()))?;
        write_zip_entry(&mut zip, "xl/styles.xml", options, &styles.to_xml())?;

        for (idx, ws) in sheets.iter().enumerate() {
            let path = format!("xl/worksheets/sheet{}.xml", idx + 1);
            let xml = build_worksheet_xml(ws, &styles);
            write_zip_entry(&mut zip, &path, options, &xml)?;
        }

        let finished = zip.finish()?;
        Ok(finished.into_inner())
    }
}

fn write_zip_entry(
    zip: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    options: SimpleFileOptions,
    content: &str,
) -> Result<(), XlsxError> {
    zip.start_file(name, options)?;
    zip.write_all(content.as_bytes())?;
    Ok(())
}

struct StyleRegistry {
    formats: Vec<Format>,
    custom_num_fmts: Vec<(u32, String)>,
}

impl StyleRegistry {
    fn new() -> Self {
        Self {
            formats: vec![Format::default()],
            custom_num_fmts: Vec::new(),
        }
    }

    fn register(&mut self, fmt: &Format) -> usize {
        if let Some(pos) = self.formats.iter().position(|f| f == fmt) {
            return pos;
        }
        if let Some(ref nf) = fmt.num_format {
            if nf != "@" && !self.custom_num_fmts.iter().any(|(_, s)| s == nf) {
                let next_id = 164 + self.custom_num_fmts.len() as u32;
                self.custom_num_fmts.push((next_id, nf.clone()));
            }
        }
        self.formats.push(fmt.clone());
        self.formats.len() - 1
    }

    fn index_of(&self, fmt: &Format) -> usize {
        self.formats.iter().position(|f| f == fmt).unwrap_or(0)
    }

    fn num_fmt_id(&self, nf: &str) -> u32 {
        if nf == "@" {
            49
        } else {
            self.custom_num_fmts
                .iter()
                .find(|(_, s)| s == nf)
                .map(|(id, _)| *id)
                .unwrap_or(0)
        }
    }

    fn to_xml(&self) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        );

        if !self.custom_num_fmts.is_empty() {
            xml.push_str(&format!(r#"<numFmts count="{}">"#, self.custom_num_fmts.len()));
            for (id, code) in &self.custom_num_fmts {
                xml.push_str(&format!(
                    r#"<numFmt numFmtId="{id}" formatCode="{}"/>"#,
                    escape(code)
                ));
            }
            xml.push_str("</numFmts>");
        }

        // Fonts: 0 = regular, 1 = bold
        xml.push_str(
            r#"<fonts count="2"><font><sz val="11"/><name val="Malgun Gothic"/></font><font><b/><sz val="11"/><name val="Malgun Gothic"/></font></fonts>"#,
        );
        xml.push_str(r#"<fills count="1"><fill><patternFill patternType="none"/></fill></fills>"#);
        xml.push_str(r#"<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>"#);
        xml.push_str(r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#);

        xml.push_str(&format!(r#"<cellXfs count="{}">"#, self.formats.len()));
        for fmt in &self.formats {
            let font_id = if fmt.bold { 1 } else { 0 };
            let (num_fmt_id, apply_nf) = match &fmt.num_format {
                Some(nf) => (self.num_fmt_id(nf), 1),
                None => (0, 0),
            };
            let align_xml = match fmt.align {
                Some(FormatAlign::Left) => r#" applyAlignment="1"><alignment horizontal="left"/></xf>"#,
                Some(FormatAlign::Center) => r#" applyAlignment="1"><alignment horizontal="center"/></xf>"#,
                Some(FormatAlign::Right) => r#" applyAlignment="1"><alignment horizontal="right"/></xf>"#,
                None => "/>",
            };
            xml.push_str(&format!(
                r#"<xf numFmtId="{num_fmt_id}" fontId="{font_id}" fillId="0" borderId="0" xfId="0" applyNumberFormat="{apply_nf}"{align_xml}"#
            ));
        }
        xml.push_str("</cellXfs></styleSheet>");
        xml
    }
}

fn build_content_types(sheet_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    for i in 1..=sheet_count {
        xml.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
    }
    xml.push_str("</Types>");
    xml
}

fn build_root_rels() -> String {
    String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
}

fn build_workbook_xml(sheets: &[Worksheet]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets>"#,
    );
    for (idx, sheet) in sheets.iter().enumerate() {
        let id = idx + 1;
        xml.push_str(&format!(
            r#"<sheet name="{}" sheetId="{id}" r:id="rId{id}"/>"#,
            escape(&sheet.name)
        ));
    }
    xml.push_str("</sheets></workbook>");
    xml
}

fn build_workbook_rels(sheet_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for i in 1..=sheet_count {
        xml.push_str(&format!(
            r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>"#
        ));
    }
    let style_id = sheet_count + 1;
    xml.push_str(&format!(
        r#"<Relationship Id="rId{style_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    ));
    xml.push_str("</Relationships>");
    xml
}

fn build_worksheet_xml(ws: &Worksheet, styles: &StyleRegistry) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    );

    if !ws.column_widths.is_empty() || !ws.column_formats.is_empty() {
        let mut all_cols: BTreeMap<u16, (Option<f64>, Option<usize>)> = BTreeMap::new();
        for (&col, &w) in &ws.column_widths {
            all_cols.entry(col).or_default().0 = Some(w);
        }
        for (&col, fmt) in &ws.column_formats {
            all_cols.entry(col).or_default().1 = Some(styles.index_of(fmt));
        }

        xml.push_str("<cols>");
        for (col, (width_opt, style_opt)) in all_cols {
            let col_1 = col as u32 + 1;
            let width_attr = width_opt
                .map(|w| format!(r#" width="{w:.2}" customWidth="1""#))
                .unwrap_or_default();
            let style_attr = style_opt
                .map(|s| format!(r#" style="{s}""#))
                .unwrap_or_default();
            xml.push_str(&format!(
                r#"<col min="{col_1}" max="{col_1}"{width_attr}{style_attr}/>"#
            ));
        }
        xml.push_str("</cols>");
    }

    xml.push_str("<sheetData>");
    let mut by_row: BTreeMap<u32, Vec<(u16, &CellData)>> = BTreeMap::new();
    for (&(row, col), cell) in &ws.cells {
        by_row.entry(row).or_default().push((col, cell));
    }

    for (row, cols) in by_row {
        let row_1 = row + 1;
        xml.push_str(&format!(r#"<row r="{row_1}">"#));
        for (col, cell) in cols {
            let cell_ref = format!("{}{row_1}", col_to_letters(col));
            let style_idx = cell
                .format
                .as_ref()
                .map(|f| styles.index_of(f))
                .or_else(|| ws.column_formats.get(&col).map(|f| styles.index_of(f)))
                .unwrap_or(0);
            let style_attr = if style_idx > 0 {
                format!(r#" s="{style_idx}""#)
            } else {
                String::new()
            };

            match &cell.value {
                CellValue::Text(s) => {
                    xml.push_str(&format!(
                        r#"<c r="{cell_ref}"{style_attr} t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                        escape(s)
                    ));
                }
                CellValue::Number(n) => {
                    xml.push_str(&format!(
                        r#"<c r="{cell_ref}"{style_attr}><v>{n}</v></c>"#
                    ));
                }
            }
        }
        xml.push_str("</row>");
    }

    xml.push_str("</sheetData></worksheet>");
    xml
}

fn col_to_letters(mut col: u16) -> String {
    let mut chars = Vec::new();
    loop {
        let rem = (col % 26) as u8;
        chars.push((b'A' + rem) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    chars.into_iter().rev().collect()
}
