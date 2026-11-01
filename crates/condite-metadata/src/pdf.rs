use crate::{Config, Error, Result};
use lopdf::{Document, Object};

/// Qualified subset: classic cross-reference tables, direct stream lengths, no
/// object streams. Preflight bounds syntax before lopdf allocates parsed objects.
/// These are cooperative structural budgets, not a hard process RAM/time limit.
pub fn strip(data: &[u8], config: &Config) -> Result<(Vec<u8>, Vec<String>)> {
    if data.len() as u64 > config.max_input_bytes {
        return Err(Error::TooLarge);
    }
    preflight(data, config)?;
    let limit = usize::try_from(config.max_entry_bytes.min(config.max_inflated_bytes))
        .map_err(|_| Error::ResourceLimit)?;
    let mut doc = Document::load_mem_with_options(
        data,
        lopdf::LoadOptions {
            strict: true,
            max_decompressed_size: Some(limit),
            ..Default::default()
        },
    )
    .map_err(|_| Error::Malformed)?;
    if doc.xref_recovery.is_some() {
        return Err(Error::Malformed);
    }
    if doc.is_encrypted() || doc.encryption_state.is_some() {
        return Err(Error::Encrypted);
    }
    if doc.objects.len() > config.max_entries {
        return Err(Error::ResourceLimit);
    }
    if let Ok(Object::Reference(id)) = doc.trailer.get(b"Info")
        && let Some(info) = doc.objects.get_mut(id)
    {
        *info = Object::Null;
    }
    let root = doc
        .trailer
        .get(b"Root")
        .map_err(|_| Error::Malformed)?
        .clone();
    // Retain only the document root; rewrite never carries old trailer data.
    doc.trailer = lopdf::dictionary! {"Root" => root};
    let mut metadata_ids = Vec::new();
    for obj in doc.objects.values() {
        metadata_refs(obj, &mut metadata_ids);
    }
    for id in metadata_ids {
        if let Some(obj) = doc.objects.get_mut(&id) {
            *obj = Object::Null;
        }
    }
    for obj in doc.objects.values_mut() {
        clean(obj);
    }
    let mut refs = Vec::new();
    collect_refs(&Object::Dictionary(doc.trailer.clone()), &mut refs);
    let mut reachable = std::collections::HashSet::new();
    while let Some(id) = refs.pop() {
        if reachable.insert(id) {
            let obj = doc.objects.get(&id).ok_or(Error::Malformed)?;
            collect_refs(obj, &mut refs);
            if reachable.len() > config.max_entries {
                return Err(Error::ResourceLimit);
            }
        }
    }
    doc.objects.retain(|id, _| reachable.contains(id));
    // Renumbering prevents attacker-controlled sparse ids from expanding xrefs.
    let ids: std::collections::HashMap<_, _> = doc
        .objects
        .keys()
        .enumerate()
        .map(|(index, id)| (*id, (index as u32 + 1, 0)))
        .collect();
    let old_objects = std::mem::take(&mut doc.objects);
    for (id, mut object) in old_objects {
        remap_refs(&mut object, &ids);
        doc.objects.insert(ids[&id], object);
    }
    let mut trailer = Object::Dictionary(std::mem::take(&mut doc.trailer));
    remap_refs(&mut trailer, &ids);
    doc.trailer = trailer.as_dict().map_err(|_| Error::Malformed)?.clone();
    doc.max_id = doc.objects.len() as u32;
    doc.binary_mark = vec![0xBB, 0xAD, 0xC0, 0xDE];
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        limit: config.max_output_bytes,
    };
    doc.save_to(&mut output).map_err(|_| Error::ResourceLimit)?;
    Ok((
        output.bytes,
        [
            "annotations",
            "forms",
            "attachments",
            "embedded-image-metadata",
            "accessibility-and-page-content",
            "pdf-a-conformance-not-guaranteed",
            "cooperative-limits-no-hard-memory-or-time-bound",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    ))
}

fn metadata_refs(obj: &Object, refs: &mut Vec<lopdf::ObjectId>) {
    match obj {
        Object::Array(items) => items.iter().for_each(|o| metadata_refs(o, refs)),
        Object::Dictionary(dict) => metadata_dict_refs(dict, refs),
        Object::Stream(stream) => metadata_dict_refs(&stream.dict, refs),
        _ => {}
    }
}
fn metadata_dict_refs(dict: &lopdf::Dictionary, refs: &mut Vec<lopdf::ObjectId>) {
    if let Ok(Object::Reference(id)) = dict.get(b"Metadata") {
        refs.push(*id);
    }
    for (_, obj) in dict.iter() {
        metadata_refs(obj, refs);
    }
}
fn clean(obj: &mut Object) {
    if let Object::Stream(stream) = obj
        && stream
            .dict
            .get(b"Type")
            .and_then(Object::as_name)
            .is_ok_and(|v| v == b"Metadata")
    {
        *obj = Object::Null;
        return;
    }
    match obj {
        Object::Array(items) => items.iter_mut().for_each(clean),
        Object::Dictionary(dict) => clean_dict(dict),
        Object::Stream(stream) => clean_dict(&mut stream.dict),
        _ => {}
    }
}
fn clean_dict(dict: &mut lopdf::Dictionary) {
    dict.remove(b"Metadata");
    for (_, value) in dict.iter_mut() {
        clean(value);
    }
}
fn remap_refs(obj: &mut Object, ids: &std::collections::HashMap<lopdf::ObjectId, lopdf::ObjectId>) {
    match obj {
        Object::Reference(id) => *id = ids[id],
        Object::Array(items) => items.iter_mut().for_each(|o| remap_refs(o, ids)),
        Object::Dictionary(d) => d.iter_mut().for_each(|(_, o)| remap_refs(o, ids)),
        Object::Stream(s) => s.dict.iter_mut().for_each(|(_, o)| remap_refs(o, ids)),
        _ => {}
    }
}
fn collect_refs(obj: &Object, refs: &mut Vec<lopdf::ObjectId>) {
    match obj {
        Object::Reference(id) => refs.push(*id),
        Object::Array(items) => items.iter().for_each(|o| collect_refs(o, refs)),
        Object::Dictionary(d) => d.iter().for_each(|(_, o)| collect_refs(o, refs)),
        Object::Stream(s) => s.dict.iter().for_each(|(_, o)| collect_refs(o, refs)),
        _ => {}
    }
}
struct BoundedOutput {
    bytes: Vec<u8>,
    limit: u64,
}
impl std::io::Write for BoundedOutput {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if (self.bytes.len() as u64).saturating_add(data.len() as u64) > self.limit {
            return Err(std::io::Error::other("PDF output limit"));
        }
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn delimiter(b: u8) -> bool {
    b.is_ascii_whitespace() || b == 0 || b"()<>[]{}/%".contains(&b)
}
fn whitespace(data: &[u8], i: &mut usize) {
    while data
        .get(*i)
        .is_some_and(|b| b.is_ascii_whitespace() || *b == 0)
    {
        *i += 1;
    }
}

fn preflight(data: &[u8], config: &Config) -> Result<()> {
    if !data.starts_with(b"%PDF-") {
        return Err(Error::Malformed);
    }
    let mut i = 0;
    let mut depth = 0usize;
    let mut lengths = [None; 65];
    let mut stream_length = None;
    let mut objects = 0usize;
    let mut xref_entries = 0usize;
    let mut tokens = 0usize;
    let mut streams = 0u64;
    let mut object_offsets = std::collections::HashSet::new();
    let mut xref_offsets = std::collections::HashSet::new();
    let mut previous = [(&b""[..], 0usize); 2];
    while i < data.len() {
        whitespace(data, &mut i);
        if i == data.len() {
            break;
        }
        tokens += 1;
        if tokens > config.max_entries.saturating_mul(64) {
            return Err(Error::ResourceLimit);
        }
        let start = i;
        let mut stream_payload = false;
        match data[i] {
            b'%' => {
                while i < data.len() && !b"\r\n".contains(&data[i]) {
                    i += 1;
                }
            }
            b'(' => {
                i += 1;
                let mut nesting = 1usize;
                while i < data.len() && nesting != 0 {
                    match data[i] {
                        b'\\' => {
                            i += 1;
                        }
                        b'(' => {
                            nesting += 1;
                            if nesting > 64 {
                                return Err(Error::ResourceLimit);
                            }
                        }
                        b')' => nesting -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                if nesting != 0 {
                    return Err(Error::Malformed);
                }
            }
            b'<' if data.get(i + 1) != Some(&b'<') => {
                i += 1;
                while i < data.len() && data[i] != b'>' {
                    i += 1;
                }
                if i == data.len() {
                    return Err(Error::Malformed);
                }
                i += 1;
            }
            b'<' | b'[' => {
                depth += 1;
                if depth > 64 {
                    return Err(Error::ResourceLimit);
                }
                lengths[depth] = None;
                i += if data[i] == b'<' { 2 } else { 1 };
            }
            b'>' | b']' => {
                if depth == 0 {
                    return Err(Error::Malformed);
                }
                stream_length = lengths[depth];
                depth -= 1;
                i += if data[i] == b'>' { 2 } else { 1 };
            }
            b'/' => {
                i += 1;
                let mut name = [0u8; 256];
                let mut n = 0;
                while i < data.len() && !delimiter(data[i]) {
                    if n == name.len() {
                        return Err(Error::ResourceLimit);
                    }
                    name[n] = if data[i] == b'#' {
                        let hex = data.get(i + 1..i + 3).ok_or(Error::Malformed)?;
                        let s = std::str::from_utf8(hex).map_err(|_| Error::Malformed)?;
                        i += 2;
                        u8::from_str_radix(s, 16).map_err(|_| Error::Malformed)?
                    } else {
                        data[i]
                    };
                    n += 1;
                    i += 1;
                }
                match &name[..n] {
                    b"Encrypt" | b"Crypt" => return Err(Error::Encrypted),
                    b"Sig" | b"DocTimeStamp" | b"ByteRange" | b"DocMDP" | b"Perms" => {
                        return Err(Error::Signed);
                    }
                    b"ObjStm" | b"XRef" | b"XRefStm" => return Err(Error::UnsupportedFeature),
                    b"Size" => {
                        let mut p = i;
                        whitespace(data, &mut p);
                        let begin = p;
                        while data.get(p).is_some_and(u8::is_ascii_digit) {
                            p += 1;
                        }
                        std::str::from_utf8(&data[begin..p])
                            .ok()
                            .and_then(|s| s.parse::<u32>().ok())
                            .filter(|v| *v != 0)
                            .ok_or(Error::Malformed)?;
                    }
                    b"Prev" => {
                        let mut p = i;
                        whitespace(data, &mut p);
                        let begin = p;
                        while data.get(p).is_some_and(u8::is_ascii_digit) {
                            p += 1;
                        }
                        let offset = std::str::from_utf8(&data[begin..p])
                            .ok()
                            .and_then(|s| s.parse::<usize>().ok())
                            .ok_or(Error::Malformed)?;
                        if !xref_offsets.contains(&offset) {
                            return Err(Error::UnsupportedFeature);
                        }
                    }
                    b"Length" => {
                        let mut p = i;
                        whitespace(data, &mut p);
                        let begin = p;
                        while data.get(p).is_some_and(u8::is_ascii_digit) {
                            p += 1;
                        }
                        let value = std::str::from_utf8(&data[begin..p])
                            .ok()
                            .and_then(|s| s.parse::<usize>().ok())
                            .ok_or(Error::UnsupportedFeature)?;
                        whitespace(data, &mut p);
                        if !matches!(data.get(p), Some(b'/' | b'>')) {
                            return Err(Error::UnsupportedFeature);
                        }
                        lengths[depth] = Some(value);
                    }
                    _ => {}
                }
            }
            _ => {
                while i < data.len() && !delimiter(data[i]) {
                    i += 1;
                }
                if i == start {
                    return Err(Error::Malformed);
                }
                match &data[start..i] {
                    b"obj" => {
                        if depth != 0
                            || previous.iter().any(|(token, _)| {
                                token.is_empty() || !token.iter().all(u8::is_ascii_digit)
                            })
                        {
                            return Err(Error::Malformed);
                        }
                        object_offsets.insert(previous[0].1);
                        objects += 1;
                        if objects > config.max_entries {
                            return Err(Error::ResourceLimit);
                        }
                    }
                    b"xref" => {
                        xref_offsets.insert(start);
                    }
                    b"n" | b"f" if depth == 0 => {
                        xref_entries += 1;
                        if xref_entries > config.max_entries {
                            return Err(Error::ResourceLimit);
                        }
                        if &data[start..i] == b"n" {
                            let offset = std::str::from_utf8(previous[0].0)
                                .ok()
                                .and_then(|s| s.parse::<usize>().ok())
                                .ok_or(Error::Malformed)?;
                            if !object_offsets.contains(&offset) {
                                return Err(Error::UnsupportedFeature);
                            }
                        }
                    }
                    b"startxref" => {
                        let mut p = i;
                        whitespace(data, &mut p);
                        let begin = p;
                        while data.get(p).is_some_and(u8::is_ascii_digit) {
                            p += 1;
                        }
                        let offset = std::str::from_utf8(&data[begin..p])
                            .ok()
                            .and_then(|s| s.parse::<usize>().ok())
                            .ok_or(Error::Malformed)?;
                        if !xref_offsets.contains(&offset) {
                            return Err(Error::UnsupportedFeature);
                        }
                    }
                    b"stream" => {
                        stream_payload = true;
                        let len = stream_length.take().ok_or(Error::UnsupportedFeature)?;
                        if len as u64 > config.max_entry_bytes {
                            return Err(Error::ResourceLimit);
                        }
                        streams = streams
                            .checked_add(len as u64)
                            .ok_or(Error::ResourceLimit)?;
                        if streams > config.max_inflated_bytes {
                            return Err(Error::ResourceLimit);
                        }
                        if data.get(i) == Some(&b'\r') {
                            i += 1;
                        }
                        if data.get(i) != Some(&b'\n') {
                            return Err(Error::Malformed);
                        }
                        i += 1;
                        i = i
                            .checked_add(len)
                            .filter(|end| *end <= data.len())
                            .ok_or(Error::Malformed)?;
                        whitespace(data, &mut i);
                        if !data[i..].starts_with(b"endstream") {
                            return Err(Error::Malformed);
                        }
                        i += 9;
                    }
                    _ => {}
                }
                previous = [previous[1], (&data[start..i], start)];
            }
        }
        // Stream payloads were already checked separately; syntax/string tokens
        // also have a bounded raw representation before parser allocation.
        if !stream_payload && (i - start) as u64 > config.max_entry_bytes {
            return Err(Error::ResourceLimit);
        }
    }
    if depth != 0 {
        return Err(Error::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Stream, dictionary};

    fn fixture() -> Vec<u8> {
        let mut doc = Document::with_version("1.4");
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        let pages = doc.new_object_id();
        let content = doc.add_object(Stream::new(dictionary! {}, b"q 1 0 0 1 0 0 cm Q".to_vec()));
        let xmp = doc.add_object(Stream::new(
            dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
            b"CANARY-XMP".to_vec(),
        ));
        let page = doc.add_object(dictionary! {"Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()], "Contents" => content, "Metadata" => xmp, "Resources" => dictionary!{}});
        doc.objects.insert(
            pages,
            dictionary! {"Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()]}.into(),
        );
        let catalog = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages, "Metadata" => xmp, "MarkInfo" => dictionary!{"Marked" => true}, "Lang" => Object::string_literal("en-US")});
        let info =
            doc.add_object(dictionary! {"Author" => Object::string_literal("CANARY-AUTHOR")});
        doc.add_object(Object::string_literal("CANARY-ORPHAN"));
        doc.trailer.set("Root", catalog);
        doc.trailer.set("Info", info);
        doc.trailer.set(
            "ID",
            vec![
                Object::string_literal("CANARY-ID"),
                Object::string_literal("CANARY-ID"),
            ],
        );
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn removes_properties_orphans_and_ids_preserving_page_operators_and_accessibility() {
        let input = fixture();
        let before = Document::load_mem(&input).unwrap();
        let (out, retained) = strip(&input, &Config::default()).unwrap();
        assert!(!out.windows(6).any(|w| w == b"CANARY"));
        let after = Document::load_mem(&out).unwrap();
        assert_eq!(after.get_pages().len(), 1);
        assert_eq!(
            before.get_page_content(*before.get_pages().values().next().unwrap()),
            after.get_page_content(*after.get_pages().values().next().unwrap())
        );
        assert!(after.catalog().unwrap().has(b"MarkInfo"));
        assert!(after.catalog().unwrap().has(b"Lang"));
        assert!(!after.trailer.has(b"Info"));
        assert!(!after.trailer.has(b"ID"));
        assert!(retained.contains(&"annotations".to_owned()));
    }

    #[test]
    fn checks_input_and_output_limits() {
        let input = fixture();
        let cfg = Config {
            max_input_bytes: input.len() as u64 - 1,
            ..Config::default()
        };
        assert_eq!(strip(&input, &cfg), Err(Error::TooLarge));
        let cfg = Config {
            max_output_bytes: 32,
            ..Config::default()
        };
        assert_eq!(strip(&input, &cfg), Err(Error::ResourceLimit));
    }
    #[test]
    fn removes_historical_revisions_with_a_full_rewrite() {
        let mut input = fixture();
        let original = Document::load_mem(&input).unwrap();
        let root = original
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let offset = input.len() + 1;
        input.extend_from_slice(b"\n99 0 obj\n<</Author (CANARY-REVISION)>>\nendobj\n");
        let xref = input.len();
        input.extend_from_slice(format!("xref\n99 1\n{offset:010} 00000 n \ntrailer\n<</Size 100/Root {} {} R/Info 99 0 R/Prev {}>>\nstartxref\n{xref}\n%%EOF", root.0, root.1, original.xref_start).as_bytes());
        assert!(Document::load_mem(&input).unwrap().trailer.has(b"Info"));
        let (out, _) = strip(&input, &Config::default()).unwrap();
        assert!(!out.windows(6).any(|s| s == b"CANARY"));
        assert_eq!(out.windows(9).filter(|s| *s == b"startxref").count(), 1);
        assert_eq!(Document::load_mem(&out).unwrap().get_pages().len(), 1);
    }

    #[test]
    #[allow(deprecated)]
    fn rejects_actual_empty_password_encryption_before_automatic_decryption() {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let version = lopdf::EncryptionVersion::V1 {
            document: &doc,
            owner_password: "owner",
            user_password: "",
            permissions: lopdf::Permissions::all(),
        };
        let state = lopdf::EncryptionState::try_from(version).unwrap();
        doc.encrypt(&state).unwrap();
        let mut encrypted = Vec::new();
        doc.save_to(&mut encrypted).unwrap();
        assert_eq!(strip(&encrypted, &Config::default()), Err(Error::Encrypted));
    }

    #[test]
    fn rejects_signatures_even_in_orphans_and_escaped_names() {
        for name in ["Sig", "S#69g", "DocTimeStamp"] {
            let mut doc = Document::load_mem(&fixture()).unwrap();
            doc.add_object(dictionary! {"Type" => "Sig", "ByteRange" => vec![0.into(), 1.into(), 2.into(), 3.into()]});
            let mut bytes = Vec::new();
            doc.save_to(&mut bytes).unwrap();
            // Trailer offsets no longer matter: rejection happens in preflight.
            let text = String::from_utf8_lossy(&bytes).replace("/Sig", &format!("/{name}"));
            assert_eq!(
                strip(text.as_bytes(), &Config::default()),
                Err(Error::Signed)
            );
        }
    }

    #[test]
    fn rejects_unqualified_compressed_structural_streams_and_indirect_lengths() {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let mut modern = Vec::new();
        doc.save_modern(&mut modern).unwrap();
        assert_eq!(
            strip(&modern, &Config::default()),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(
            strip(
                b"%PDF-1.4\n1 0 obj <</Length 2 0 R>>stream\n",
                &Config::default()
            ),
            Err(Error::UnsupportedFeature)
        );
    }

    #[test]
    fn enforces_nesting_objects_and_stream_budgets_before_loading() {
        let mut nested = b"%PDF-1.4\n".to_vec();
        nested.extend(std::iter::repeat_n(b'[', 65));
        assert_eq!(
            strip(&nested, &Config::default()),
            Err(Error::ResourceLimit)
        );
        let input = fixture();
        for cfg in [
            Config {
                max_entries: 1,
                ..Config::default()
            },
            Config {
                max_entry_bytes: 4,
                ..Config::default()
            },
            Config {
                max_inflated_bytes: 4,
                ..Config::default()
            },
        ] {
            assert_eq!(strip(&input, &cfg), Err(Error::ResourceLimit));
        }
    }
    #[test]
    fn rejects_invalid_trailer_sizes_and_xref_offsets_before_parser() {
        assert_eq!(
            strip(b"%PDF-1.4\ntrailer<</Size 0>>", &Config::default()),
            Err(Error::Malformed)
        );
        let mut input = fixture();
        let entry = input.windows(9).position(|w| w == b" 00000 n ").unwrap();
        input[entry - 10..entry].copy_from_slice(b"0000000000");
        assert_eq!(
            strip(&input, &Config::default()),
            Err(Error::UnsupportedFeature)
        );
    }

    #[test]
    fn preserves_reachable_cycles_without_page_tree_traversal() {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.objects
            .get_mut(&root)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("CustomCycle", root);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let (out, _) = strip(&bytes, &Config::default()).unwrap();
        let clean = Document::load_mem(&out).unwrap();
        assert_eq!(
            clean.catalog().unwrap().get(b"CustomCycle").unwrap(),
            clean.trailer.get(b"Root").unwrap()
        );
    }
    #[test]
    fn removes_metadata_stream_aliases_even_without_type_marker() {
        let mut doc = Document::load_mem(&fixture()).unwrap();
        let metadata = doc.add_object(Stream::new(dictionary! {}, b"ALIAS-PRIVATE-XMP".to_vec()));
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = doc.objects.get_mut(&root).unwrap().as_dict_mut().unwrap();
        catalog.set("Metadata", metadata);
        catalog.set("Alias", metadata);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let (out, _) = strip(&bytes, &Config::default()).unwrap();
        assert!(
            !out.windows(b"ALIAS-PRIVATE-XMP".len())
                .any(|s| s == b"ALIAS-PRIVATE-XMP")
        );
        let result = Document::load_mem(&out).unwrap();
        let alias = result
            .catalog()
            .unwrap()
            .get(b"Alias")
            .unwrap()
            .as_reference()
            .unwrap();
        assert!(matches!(result.objects[&alias], Object::Null));
    }
}
