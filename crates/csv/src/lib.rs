use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trim {
    None,
    Headers,
    Fields,
    All,
}

#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CSV 오류: {}", self.0)
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self(err.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StringRecord {
    fields: Vec<String>,
}

impl StringRecord {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from<T: Into<String>>(iter: Vec<T>) -> Self {
        Self {
            fields: iter.into_iter().map(Into::into).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.fields.get(index).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct ReaderBuilder {
    has_headers: bool,
    trim: Trim,
    flexible: bool,
    delimiter: u8,
}

impl Default for ReaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReaderBuilder {
    pub fn new() -> Self {
        Self {
            has_headers: true,
            trim: Trim::None,
            flexible: true,
            delimiter: b',',
        }
    }

    pub fn has_headers(&mut self, yes: bool) -> &mut Self {
        self.has_headers = yes;
        self
    }

    pub fn trim(&mut self, trim: Trim) -> &mut Self {
        self.trim = trim;
        self
    }

    pub fn flexible(&mut self, yes: bool) -> &mut Self {
        self.flexible = yes;
        self
    }

    pub fn delimiter(&mut self, delimiter: u8) -> &mut Self {
        self.delimiter = delimiter;
        self
    }

    pub fn from_reader<R: Read>(&self, mut rdr: R) -> Reader {
        let mut buf = Vec::new();
        let read_err = rdr.read_to_end(&mut buf).err();
        Reader::from_bytes(buf, read_err, self.clone())
    }

    pub fn from_path<P: AsRef<Path>>(&self, path: P) -> Result<Reader, Error> {
        let file = File::open(path)?;
        Ok(self.from_reader(file))
    }
}

pub struct Reader {
    headers: Option<StringRecord>,
    rows: Vec<StringRecord>,
    read_err: Option<io::Error>,
    parse_err: Option<String>,
}

impl Reader {
    pub fn from_reader<R: Read>(rdr: R) -> Self {
        ReaderBuilder::new().from_reader(rdr)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        ReaderBuilder::new().from_path(path)
    }

    fn from_bytes(buf: Vec<u8>, read_err: Option<io::Error>, cfg: ReaderBuilder) -> Self {
        if read_err.is_some() {
            return Self {
                headers: None,
                rows: Vec::new(),
                read_err,
                parse_err: None,
            };
        }

        let text = match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return Self {
                    headers: None,
                    rows: Vec::new(),
                    read_err: None,
                    parse_err: Some(format!("UTF-8 디코딩 실패: {e}")),
                };
            }
        };

        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
        match parse_rfc4180(text, cfg.delimiter as char) {
            Ok(mut all_rows) => {
                if cfg.has_headers && !all_rows.is_empty() {
                    let mut hdr = all_rows.remove(0);
                    if matches!(cfg.trim, Trim::Headers | Trim::All) {
                        hdr = trim_record(hdr);
                    }
                    let rows = if matches!(cfg.trim, Trim::Fields | Trim::All) {
                        all_rows.into_iter().map(trim_record).collect()
                    } else {
                        all_rows
                    };
                    if !cfg.flexible {
                        let expected = hdr.len();
                        if let Some((idx, bad)) =
                            rows.iter().enumerate().find(|(_, r)| r.len() != expected)
                        {
                            return Self {
                                headers: Some(hdr),
                                rows: Vec::new(),
                                read_err: None,
                                parse_err: Some(format!(
                                    "{}행 열 개수가 일치하지 않습니다 (예상 {expected}열, 실제 {}열)",
                                    idx + 2,
                                    bad.len()
                                )),
                            };
                        }
                    }
                    Self {
                        headers: Some(hdr),
                        rows,
                        read_err: None,
                        parse_err: None,
                    }
                } else {
                    let rows = if matches!(cfg.trim, Trim::Fields | Trim::All) {
                        all_rows.into_iter().map(trim_record).collect()
                    } else {
                        all_rows
                    };
                    Self {
                        headers: Some(StringRecord::new()),
                        rows,
                        read_err: None,
                        parse_err: None,
                    }
                }
            }
            Err(err) => Self {
                headers: None,
                rows: Vec::new(),
                read_err: None,
                parse_err: Some(err),
            },
        }
    }

    pub fn headers(&mut self) -> Result<&StringRecord, Error> {
        if let Some(ref e) = self.read_err {
            return Err(Error(e.to_string()));
        }
        if let Some(ref e) = self.parse_err {
            return Err(Error(e.clone()));
        }
        Ok(self.headers.get_or_insert_with(StringRecord::new))
    }

    pub fn records(&mut self) -> RecordsIter<'_> {
        let err = self
            .read_err
            .as_ref()
            .map(|e| e.to_string())
            .or_else(|| self.parse_err.clone());
        RecordsIter {
            rows: &self.rows,
            index: 0,
            pending_err: err,
        }
    }
}

pub struct RecordsIter<'a> {
    rows: &'a [StringRecord],
    index: usize,
    pending_err: Option<String>,
}

impl<'a> Iterator for RecordsIter<'a> {
    type Item = Result<StringRecord, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.pending_err.take() {
            return Some(Err(Error(err)));
        }
        if self.index < self.rows.len() {
            let rec = self.rows[self.index].clone();
            self.index += 1;
            Some(Ok(rec))
        } else {
            None
        }
    }
}

fn trim_record(rec: StringRecord) -> StringRecord {
    StringRecord {
        fields: rec
            .fields
            .into_iter()
            .map(|f| f.trim().to_string())
            .collect(),
    }
}

fn parse_rfc4180(input: &str, delimiter: char) -> Result<Vec<StringRecord>, String> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;
    let mut field_started = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }

        if ch == '"' && !field_started && field.is_empty() {
            in_quotes = true;
            field_started = true;
        } else if ch == delimiter {
            current_row.push(std::mem::take(&mut field));
            field_started = false;
        } else if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            current_row.push(std::mem::take(&mut field));
            rows.push(StringRecord {
                fields: std::mem::take(&mut current_row),
            });
            field_started = false;
        } else if ch == '\n' {
            current_row.push(std::mem::take(&mut field));
            rows.push(StringRecord {
                fields: std::mem::take(&mut current_row),
            });
            field_started = false;
        } else {
            field.push(ch);
            field_started = true;
        }
    }

    if in_quotes {
        return Err("닫히지 않은 따옴표가 있습니다.".to_string());
    }

    if field_started || !field.is_empty() || !current_row.is_empty() {
        current_row.push(field);
        rows.push(StringRecord {
            fields: current_row,
        });
    }

    Ok(rows)
}
