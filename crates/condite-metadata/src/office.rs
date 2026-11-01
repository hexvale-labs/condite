use crate::{Config, Error, Result};
use quick_xml::{
    Writer,
    events::{BytesStart, Event},
    name::ResolveResult,
    reader::NsReader,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Seek, SeekFrom, Write},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

type Attributes = BTreeMap<String, String>;

// OPC manifests are flat. Reject extensions rather than interpreting unknown schemas.
fn manifest(data: &[u8], namespace: &str, root: &str) -> Result<Vec<(String, Attributes)>> {
    let mut reader = NsReader::from_reader(data);
    let mut depth = 0usize;
    let mut roots = 0;
    let mut entries = Vec::new();
    loop {
        let (ns, event) = reader.read_resolved_event().map_err(|_| Error::Malformed)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if !matches!(ns, ResolveResult::Bound(n) if n.as_ref() == namespace) {
                    return Err(Error::UnsupportedFeature);
                }
                let local = element.local_name().as_ref().to_owned();
                if depth == 0 {
                    roots += 1;
                    if roots != 1 || local != root {
                        return Err(Error::Malformed);
                    }
                } else if depth == 1 {
                    let mut attrs = Attributes::new();
                    for attr in element.attributes() {
                        let attr = attr.map_err(|_| Error::Malformed)?;
                        let key = attr.key.as_ref();
                        if key == "xmlns" || key.starts_with("xmlns:") {
                            continue;
                        }
                        if key.contains(':') {
                            return Err(Error::UnsupportedFeature);
                        }
                        attrs.insert(
                            key.to_owned(),
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|_| Error::Malformed)?
                                .into_owned(),
                        );
                    }
                    entries.push((local, attrs));
                } else {
                    return Err(Error::Malformed);
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(Error::Malformed)?;
            }
            Event::Text(t) if t.as_ref().bytes().all(|b| b.is_ascii_whitespace()) => (),
            Event::Decl(_) | Event::Comment(_) => (),
            Event::Eof if depth == 0 && roots == 1 => break,
            _ => return Err(Error::Malformed),
        }
    }
    Ok(entries)
}

fn attr<'a>(attrs: &'a Attributes, name: &str) -> Result<&'a str> {
    attrs.get(name).map(String::as_str).ok_or(Error::Malformed)
}

fn canonical(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', '%', '?', '#', ':'])
        && name
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

fn target(source: &str, value: &str) -> Result<String> {
    if value.contains(['\\', '%', '?', '#', ':']) {
        return Err(Error::UnsupportedFeature);
    }
    let mut parts: Vec<&str> = if value.starts_with('/') {
        Vec::new()
    } else {
        source.split('/').filter(|p| !p.is_empty()).collect()
    };
    for part in value.trim_start_matches('/').split('/') {
        match part {
            ".." => {
                parts.pop().ok_or(Error::Malformed)?;
            }
            "." => (),
            "" => return Err(Error::Malformed),
            _ => parts.push(part),
        }
    }
    let result = parts.join("/");
    if !canonical(&result) {
        return Err(Error::Malformed);
    }
    Ok(result)
}

fn relation_source(name: &str) -> Result<String> {
    if name == "_rels/.rels" {
        return Ok(String::new());
    }
    let (directory, file) = name.rsplit_once("/_rels/").ok_or(Error::Malformed)?;
    if file.contains('/') || !file.ends_with(".rels") {
        return Err(Error::Malformed);
    }
    Ok(format!("{directory}/{}", file.trim_end_matches(".rels")))
}

fn is_property_type(value: &str) -> bool {
    matches!(
        value,
        "application/vnd.openxmlformats-package.core-properties+xml"
            | "application/vnd.openxmlformats-officedocument.extended-properties+xml"
            | "application/vnd.openxmlformats-officedocument.custom-properties+xml"
    )
}

fn removed_relationship(value: &str) -> bool {
    matches!(
        value,
        "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties"
            | "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"
    ) || value == format!("{OFFICE_REL}extended-properties")
        || value == format!("{OFFICE_REL}custom-properties")
}

fn check_variant(value: &str) -> Result<()> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("digital-signature") || lower.contains("digitalsignature") {
        return Err(Error::Signed);
    }
    if lower.contains("macroenabled") || lower.contains("vbaproject") || lower.contains("activex") {
        return Err(Error::UnsupportedFeature);
    }
    Ok(())
}

fn validate_xml(data: &[u8]) -> Result<()> {
    let mut reader = NsReader::from_reader(data);
    let mut depth = 0usize;
    let mut roots = 0usize;
    loop {
        let (ns, event) = reader.read_resolved_event().map_err(|_| Error::Malformed)?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if matches!(ns, ResolveResult::Unknown(_)) {
                    return Err(Error::Malformed);
                }
                if let ResolveResult::Bound(namespace) = ns {
                    if namespace
                        .as_ref()
                        .starts_with("http://purl.oclc.org/ooxml/")
                    {
                        return Err(Error::UnsupportedFeature);
                    }
                    if namespace.as_ref() == "http://www.w3.org/2000/09/xmldsig#"
                        && e.local_name().as_ref() == "Signature"
                    {
                        return Err(Error::Signed);
                    }
                }
                if depth == 0 {
                    roots += 1;
                }
                if roots > 1 {
                    return Err(Error::Malformed);
                }
                if matches!(
                    e.local_name().as_ref(),
                    "documentProtection"
                        | "sheetProtection"
                        | "workbookProtection"
                        | "writeProtection"
                        | "modifyVerifier"
                        | "fileSharing"
                        | "encryptedPackage"
                ) {
                    return Err(Error::UnsupportedFeature);
                }
                for a in e.attributes() {
                    a.map_err(|_| Error::Malformed)?;
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
                if depth > 64 {
                    return Err(Error::ResourceLimit);
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(Error::Malformed)?;
            }
            Event::DocType(_) => return Err(Error::UnsupportedFeature),
            Event::GeneralRef(reference) => {
                if depth == 0 {
                    return Err(Error::Malformed);
                }
                if reference
                    .resolve_char_ref()
                    .map_err(|_| Error::Malformed)?
                    .is_none()
                    && !matches!(reference.as_ref(), "amp" | "lt" | "gt" | "apos" | "quot")
                {
                    return Err(Error::UnsupportedFeature);
                }
            }
            Event::Eof => {
                return if roots == 1 && depth == 0 {
                    Ok(())
                } else {
                    Err(Error::Malformed)
                };
            }
            Event::Text(ref t)
                if depth == 0 && !t.as_ref().bytes().all(|b| b.is_ascii_whitespace()) =>
            {
                return Err(Error::Malformed);
            }
            _ => (),
        }
    }
}

fn render(namespace: &str, root: &str, entries: &[(String, Attributes)]) -> Result<Vec<u8>> {
    let mut writer = Writer::new(Vec::new());
    let mut start = BytesStart::new(root);
    start.push_attribute(("xmlns", namespace));
    writer
        .write_event(Event::Start(start))
        .map_err(|_| Error::Malformed)?;
    for (name, attrs) in entries {
        let mut element = BytesStart::new(name.as_str());
        for (key, value) in attrs {
            element.push_attribute((key.as_str(), value.as_str()));
        }
        writer
            .write_event(Event::Empty(element))
            .map_err(|_| Error::Malformed)?;
    }
    writer
        .write_event(Event::End(quick_xml::events::BytesEnd::new(root)))
        .map_err(|_| Error::Malformed)?;
    Ok(writer.into_inner())
}

struct BoundedOutput {
    cursor: Cursor<Vec<u8>>,
    limit: u64,
    exceeded: bool,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.cursor.position().saturating_add(bytes.len() as u64);
        if self.exceeded || end > self.limit {
            self.exceeded = true;
            self.cursor.set_position(end);
            return Ok(bytes.len());
        }
        self.cursor.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Seek for BoundedOutput {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}

// Preflight before ZipArchive allocates one metadata object per central record.
fn preflight(data: &[u8], config: &Config) -> Result<()> {
    if data.len() as u64 > config.max_input_bytes {
        return Err(Error::TooLarge);
    }
    let end = data
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or(Error::Malformed)?;
    let header = data.get(end..end + 22).ok_or(Error::Malformed)?;
    let u16_at = |i| u16::from_le_bytes([header[i], header[i + 1]]);
    if end + 22 + u16_at(20) as usize != data.len() {
        return Err(Error::Malformed);
    }
    if u16_at(4) != 0 || u16_at(6) != 0 || u16_at(8) != u16_at(10) {
        return Err(Error::UnsupportedFeature);
    }
    let count = u16_at(10);
    if count == u16::MAX {
        return Err(Error::UnsupportedFeature);
    }
    if count as usize > config.max_entries {
        return Err(Error::ResourceLimit);
    }
    let offset = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    let mut position = offset;
    let mut actual_count = 0;
    let mut names = BTreeSet::new();
    while position < end {
        let entry = data.get(position..position + 46).ok_or(Error::Malformed)?;
        if &entry[..4] != b"PK\x01\x02" {
            return Err(Error::Malformed);
        }
        actual_count += 1;
        if actual_count > config.max_entries {
            return Err(Error::ResourceLimit);
        }
        let size = |i| u16::from_le_bytes([entry[i], entry[i + 1]]) as usize;
        let name = data
            .get(position + 46..position + 46 + size(28))
            .ok_or(Error::Malformed)?;
        let name = std::str::from_utf8(name).map_err(|_| Error::UnsupportedFeature)?;
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(Error::Malformed);
        }
        position = position
            .checked_add(46 + size(28) + size(30) + size(32))
            .ok_or(Error::Malformed)?;
    }
    if position != end || actual_count != count as usize {
        return Err(Error::Malformed);
    }
    Ok(())
}

// Used after strip has validated the package; family comes from OPC types, not paths.
pub fn mime_type(data: &[u8], config: &Config) -> Result<&'static str> {
    preflight(data, config)?;
    let mut archive = ZipArchive::new(Cursor::new(data)).map_err(|_| Error::Malformed)?;
    let mut bytes = Vec::new();
    let limit = config.max_entry_bytes.min(config.max_inflated_bytes);
    let part = archive
        .by_name("[Content_Types].xml")
        .map_err(|_| Error::Malformed)?;
    if part.size() > limit {
        return Err(Error::ResourceLimit);
    }
    part.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Malformed)?;
    if bytes.len() as u64 > limit {
        return Err(Error::ResourceLimit);
    }
    let entries = manifest(&bytes, CT, "Types")?;
    let mut defaults = BTreeMap::new();
    let mut overrides = BTreeMap::new();
    for (kind, attrs) in entries {
        let value = attr(&attrs, "ContentType")?.to_owned();
        check_variant(&value)?;
        match kind.as_str() {
            "Default" => {
                defaults.insert(attr(&attrs, "Extension")?.to_owned(), value);
            }
            "Override" => {
                overrides.insert(
                    attr(&attrs, "PartName")?.trim_start_matches('/').to_owned(),
                    value,
                );
            }
            _ => return Err(Error::UnsupportedFeature),
        }
    }
    let mut result = None;
    for name in archive.file_names() {
        let content_type = overrides
            .get(name)
            .or_else(|| name.rsplit_once('.').and_then(|(_, ext)| defaults.get(ext)));
        if let Some(mime) = content_type.and_then(|t| main_mime(t))
            && result.replace(mime).is_some()
        {
            return Err(Error::UnsupportedFormat);
        }
    }
    result.ok_or(Error::UnsupportedFormat)
}

fn main_mime(content_type: &str) -> Option<&'static str> {
    match content_type {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml" => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml" => {
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        _ => None,
    }
}

pub fn strip(data: &[u8], config: &Config) -> Result<(Vec<u8>, Vec<String>)> {
    preflight(data, config)?;
    let mut archive = ZipArchive::new(Cursor::new(data)).map_err(|_| Error::Malformed)?;
    let mut parts = BTreeMap::new();
    let mut inflated = 0u64;
    let mut names = BTreeSet::new();
    for index in 0..archive.len() {
        let raw = archive.by_index_raw(index).map_err(|_| Error::Malformed)?;
        if raw.encrypted() {
            return Err(Error::Encrypted);
        }
        let name = raw.name().to_owned();
        let lower = name.to_ascii_lowercase();
        if !names.insert(lower.clone()) {
            return Err(Error::Malformed);
        }
        if lower.starts_with("_xmlsignatures/") {
            return Err(Error::Signed);
        }
        check_variant(&name)?;
        if raw.is_dir() {
            continue;
        }
        if !canonical(&name) {
            return Err(Error::UnsupportedFeature);
        }
        let remaining = config.max_inflated_bytes.saturating_sub(inflated);
        let allowed = config.max_entry_bytes.min(remaining);
        if raw.size() > allowed {
            return Err(Error::ResourceLimit);
        }
        drop(raw);
        let mut bytes = Vec::new();
        archive
            .by_index(index)
            .map_err(|_| Error::Malformed)?
            .take(allowed.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Malformed)?;
        if bytes.len() as u64 > allowed {
            return Err(Error::ResourceLimit);
        }
        inflated += bytes.len() as u64;
        parts.insert(name, bytes);
    }
    let mut types = manifest(
        parts
            .get("[Content_Types].xml")
            .ok_or(Error::UnsupportedFormat)?,
        CT,
        "Types",
    )?;
    let mut defaults = BTreeMap::new();
    let mut overrides = BTreeMap::new();
    for (kind, attrs) in &types {
        let content_type = attr(attrs, "ContentType")?;
        check_variant(content_type)?;
        match kind.as_str() {
            "Default" => {
                if defaults
                    .insert(
                        attr(attrs, "Extension")?.to_owned(),
                        content_type.to_owned(),
                    )
                    .is_some()
                {
                    return Err(Error::Malformed);
                }
            }
            "Override" => {
                let name = attr(attrs, "PartName")?
                    .strip_prefix('/')
                    .ok_or(Error::Malformed)?;
                if !canonical(name)
                    || overrides
                        .insert(name.to_owned(), content_type.to_owned())
                        .is_some()
                {
                    return Err(Error::Malformed);
                }
            }
            _ => return Err(Error::UnsupportedFeature),
        }
    }
    let mut removed = BTreeSet::new();
    let mut main = Vec::new();
    for (name, bytes) in &parts {
        if name == "[Content_Types].xml" {
            continue;
        }
        let content_type = overrides
            .get(name)
            .or_else(|| name.rsplit_once('.').and_then(|(_, ext)| defaults.get(ext)))
            .ok_or(Error::Malformed)?;
        if is_property_type(content_type) {
            removed.insert(name.clone());
        }
        if main_mime(content_type).is_some() {
            main.push(name.clone());
        }
        if name.ends_with(".xml")
            || content_type.ends_with("+xml")
            || content_type == "application/xml"
        {
            validate_xml(bytes)?;
        }
    }
    if main.len() != 1 {
        return Err(Error::UnsupportedFormat);
    }
    let mut relationships = BTreeMap::new();
    let mut root_main_count = 0;
    for (name, bytes) in &parts {
        if !name.ends_with(".rels") {
            continue;
        }
        let source = relation_source(name)?;
        if !source.is_empty() && !parts.contains_key(&source) {
            return Err(Error::Malformed);
        }
        let directory = source.rsplit_once('/').map_or("", |(d, _)| d);
        let entries = manifest(bytes, REL, "Relationships")?;
        let mut ids = BTreeSet::new();
        for (kind, attrs) in &entries {
            if kind != "Relationship" || !ids.insert(attr(attrs, "Id")?) {
                return Err(Error::Malformed);
            }
            let rel_type = attr(attrs, "Type")?;
            check_variant(rel_type)?;
            let external = attrs.get("TargetMode").map(String::as_str);
            if external.is_some_and(|mode| mode != "External" && mode != "Internal") {
                return Err(Error::Malformed);
            }
            if external == Some("External") {
                if removed_relationship(rel_type) {
                    return Err(Error::UnsupportedFeature);
                }
                continue;
            }
            let dest = target(directory, attr(attrs, "Target")?)?;
            if !parts.contains_key(&dest) {
                return Err(Error::Malformed);
            }
            if removed_relationship(rel_type) {
                removed.insert(dest.clone());
            }
            if source.is_empty() && rel_type == format!("{OFFICE_REL}officeDocument") {
                if dest != main[0] {
                    return Err(Error::Malformed);
                }
                root_main_count += 1;
            }
        }
        relationships.insert(name.clone(), (directory.to_owned(), entries));
    }
    if root_main_count != 1 {
        return Err(Error::UnsupportedFormat);
    }
    if removed.contains(&main[0]) {
        return Err(Error::Malformed);
    }
    for (name, (directory, mut entries)) in relationships {
        let source = relation_source(&name)?;
        if removed.contains(&source) {
            removed.insert(name);
            continue;
        }
        entries.retain(|(_, attrs)| {
            attrs.get("TargetMode").is_some_and(|m| m == "External")
                || target(&directory, attrs.get("Target").unwrap())
                    .is_ok_and(|t| !removed.contains(&t))
        });
        parts.insert(name, render(REL, "Relationships", &entries)?);
    }
    for name in &removed {
        parts.remove(name);
    }
    types.retain(|(kind, attrs)| {
        if kind == "Override" {
            return attrs
                .get("PartName")
                .is_some_and(|n| !removed.contains(n.trim_start_matches('/')));
        }
        !is_property_type(attrs.get("ContentType").unwrap())
    });
    parts.insert(
        "[Content_Types].xml".to_owned(),
        render(CT, "Types", &types)?,
    );
    let output = BoundedOutput {
        cursor: Cursor::new(Vec::new()),
        limit: config.max_output_bytes,
        exceeded: false,
    };
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (name, bytes) in parts {
        writer
            .start_file(name, options)
            .map_err(|_| Error::ResourceLimit)?;
        writer.write_all(&bytes).map_err(|_| Error::ResourceLimit)?;
    }
    let output = writer.finish().map_err(|_| Error::ResourceLimit)?;
    if output.exceeded {
        return Err(Error::ResourceLimit);
    }
    let output = output.cursor.into_inner();
    Ok((
        output,
        vec![
            "comments-and-revisions".into(),
            "notes-and-document-content".into(),
            "external-links".into(),
            "embedded-objects".into(),
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read, Write};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    fn package(extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let types = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#;
        let rels = br#"<r:Relationships xmlns:r="http://schemas.openxmlformats.org/package/2006/relationships"><r:Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><r:Relationship Id="r2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></r:Relationships>"#;
        for (name, bytes) in [
            ("[Content_Types].xml", types.as_slice()),
            ("_rels/.rels", rels.as_slice()),
            (
                "word/document.xml",
                b"<document><body>visible</body></document>".as_slice(),
            ),
            (
                "docProps/core.xml",
                b"<properties>PRIVATE_CANARY</properties>".as_slice(),
            ),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.set_comment("PRIVATE_CANARY").unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn removes_properties_and_relationships_preserving_content() {
        let input = package(&[("word/comments.xml", b"<comments>RETAINED_AUTHOR</comments>")]);
        let (output, retained) = strip(&input, &Config::default()).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(output)).unwrap();
        assert!(archive.comment().is_empty());
        assert!(archive.by_name("docProps/core.xml").is_err());
        for name in ["[Content_Types].xml", "_rels/.rels"] {
            let mut xml = String::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            assert!(!xml.contains("docProps/core.xml"));
        }
        let mut comments = String::new();
        archive
            .by_name("word/comments.xml")
            .unwrap()
            .read_to_string(&mut comments)
            .unwrap();
        assert!(comments.contains("RETAINED_AUTHOR"));
        assert!(retained.iter().any(|s| s.contains("comments")));
    }

    #[test]
    fn rejects_protected_and_signed_packages() {
        for (name, bytes, expected) in [
            (
                "word/settings.xml",
                b"<settings><documentProtection/></settings>".as_slice(),
                Error::UnsupportedFeature,
            ),
            (
                "_xmlsignatures/sig1.xml",
                b"<signature/>".as_slice(),
                Error::Signed,
            ),
            (
                "word/vbaProject.bin",
                b"macro".as_slice(),
                Error::UnsupportedFeature,
            ),
        ] {
            assert_eq!(
                strip(&package(&[(name, bytes)]), &Config::default()).unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn enforces_entry_and_inflation_limits() {
        let input = package(&[]);
        for config in [
            Config {
                max_entries: 2,
                ..Config::default()
            },
            Config {
                max_entry_bytes: 8,
                ..Config::default()
            },
            Config {
                max_inflated_bytes: 16,
                ..Config::default()
            },
        ] {
            assert_eq!(strip(&input, &config).unwrap_err(), Error::ResourceLimit);
        }
    }
    fn rewrite(input: &[u8], transform: impl Fn(&str, Vec<u8>) -> (String, Vec<u8>)) -> Vec<u8> {
        let mut archive = ZipArchive::new(Cursor::new(input)).unwrap();
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            let (name, bytes) = transform(file.name(), bytes);
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn supports_spreadsheet_and_presentation_packages() {
        for (main, mime) in [
            (
                "xl/workbook.xml",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
            ),
            (
                "ppt/presentation.xml",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
            ),
        ] {
            let input = rewrite(&package(&[]), |name, bytes| {
                let xml = String::from_utf8(bytes).unwrap().replace("word/document.xml", main).replace("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml", mime);
                (name.replace("word/document.xml", main), xml.into_bytes())
            });
            let (output, _) = strip(&input, &Config::default()).unwrap();
            let mut archive = ZipArchive::new(Cursor::new(output)).unwrap();
            assert!(archive.by_name(main).is_ok());
        }
    }

    #[test]
    fn removes_thumbnail_and_properties_at_custom_paths() {
        let input = rewrite(
            &package(&[("preview.png", b"PRIVATE_THUMBNAIL")]),
            |name, bytes| {
                if name == "preview.png" {
                    return (name.into(), bytes);
                }
                let mut xml = String::from_utf8(bytes)
                    .unwrap()
                    .replace("docProps/core.xml", "custom/identity.xml");
                if name == "[Content_Types].xml" {
                    xml = xml.replace(
                        "</Types>",
                        "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>",
                    );
                }
                if name == "_rels/.rels" {
                    xml = xml.replace("</r:Relationships>", "<r:Relationship Id=\"thumb\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail\" Target=\"preview.png\"/></r:Relationships>");
                }
                (
                    name.replace("docProps/core.xml", "custom/identity.xml"),
                    xml.into_bytes(),
                )
            },
        );
        let (output, _) = strip(&input, &Config::default()).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(output)).unwrap();
        assert!(archive.by_name("custom/identity.xml").is_err());
        assert!(archive.by_name("preview.png").is_err());
    }

    #[test]
    fn rejects_duplicate_central_directory_names() {
        let mut input = package(&[("word/comments.xml", b"<comments/>")]);
        let duplicate = b"word/document.xml";
        let original = b"word/comments.xml";
        for i in 0..input.len() - original.len() {
            if &input[i..i + original.len()] == original {
                input[i..i + original.len()].copy_from_slice(duplicate);
            }
        }
        assert_eq!(
            strip(&input, &Config::default()).unwrap_err(),
            Error::Malformed
        );
    }

    #[test]
    fn rejects_xml_depth_and_output_overflow() {
        let deep = format!("{}{}", "<a>".repeat(65), "</a>".repeat(65));
        assert_eq!(
            strip(
                &package(&[("word/deep.xml", deep.as_bytes())]),
                &Config::default()
            )
            .unwrap_err(),
            Error::ResourceLimit
        );
        assert_eq!(
            strip(
                &package(&[]),
                &Config {
                    max_output_bytes: 32,
                    ..Config::default()
                }
            )
            .unwrap_err(),
            Error::ResourceLimit
        );
    }
    #[test]
    fn preserves_normal_xml_character_references() {
        let input = rewrite(&package(&[]), |name, bytes| {
            let bytes = if name == "word/document.xml" {
                b"<document>Fish &amp; chips &#65;</document>".to_vec()
            } else {
                bytes
            };
            (name.into(), bytes)
        });
        assert!(strip(&input, &Config::default()).is_ok());
    }
    #[test]
    fn rejects_strict_ooxml_and_signature_elements() {
        for (xml, expected) in [
            (
                "<document xmlns=\"http://purl.oclc.org/ooxml/wordprocessingml/main\"/>",
                Error::UnsupportedFeature,
            ),
            (
                "<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\"/>",
                Error::Signed,
            ),
        ] {
            let input = rewrite(&package(&[]), |name, bytes| {
                (
                    name.into(),
                    if name == "word/document.xml" {
                        xml.as_bytes().to_vec()
                    } else {
                        bytes
                    },
                )
            });
            assert_eq!(strip(&input, &Config::default()).unwrap_err(), expected);
        }
    }
    #[test]
    fn identifies_mime_from_content_types_at_custom_main_path() {
        let input = rewrite(&package(&[]), |name, bytes| {
            (
                name.replace("word/document.xml", "custom/main.xml"),
                String::from_utf8(bytes)
                    .unwrap()
                    .replace("word/document.xml", "custom/main.xml")
                    .into_bytes(),
            )
        });
        let (output, _) = strip(&input, &Config::default()).unwrap();
        assert_eq!(
            mime_type(&output, &Config::default()).unwrap(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
    }
}
