use crate::{Config, Error, Result};

/// Rewrite supported containers without decoding their compressed image payloads.
pub fn strip(data: &[u8], config: &Config) -> Result<(Vec<u8>, Vec<String>)> {
    if data.len() as u64 > config.max_input_bytes {
        return Err(Error::TooLarge);
    }
    if config.max_entries == 0 || config.max_output_bytes == 0 || config.max_entry_bytes == 0 {
        return Err(Error::InvalidConfig);
    }
    let mut writer = Writer {
        bytes: Vec::new(),
        retained: Vec::new(),
        config,
        entries: 0,
    };
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(data, &mut writer)?;
    } else if data.starts_with(b"\xff\xd8") {
        jpeg(data, &mut writer)?;
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        webp(data, &mut writer)?;
    } else {
        return Err(Error::UnsupportedFormat);
    }
    Ok((writer.bytes, writer.retained))
}
struct Writer<'a> {
    bytes: Vec<u8>,
    retained: Vec<String>,
    config: &'a Config,
    entries: usize,
}
impl Writer<'_> {
    fn add(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() as u64
            > self
                .config
                .max_output_bytes
                .saturating_sub(self.bytes.len() as u64)
        {
            return Err(Error::ResourceLimit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn entry(&mut self, size: usize) -> Result<()> {
        self.entries += 1;
        if self.entries > self.config.max_entries || size as u64 > self.config.max_entry_bytes {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
    fn retain(&mut self, category: &str) {
        if !self.retained.iter().any(|s| s == category) {
            self.retained.push(category.into());
        }
    }
}
fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}
fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[..4].try_into().unwrap())
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
fn png_write(w: &mut Writer<'_>, kind: &[u8], payload: &[u8]) -> Result<()> {
    w.add(&(payload.len() as u32).to_be_bytes())?;
    let start = w.bytes.len();
    w.add(kind)?;
    w.add(payload)?;
    w.add(&crc32(&w.bytes[start..]).to_be_bytes())
}
fn png(data: &[u8], w: &mut Writer<'_>) -> Result<()> {
    w.add(&data[..8])?;
    let mut pos = 8usize;
    let mut header = false;
    let mut pixels = false;
    let mut animation = false;
    let mut sequence = 0u32;
    let mut color = 0u8;
    let mut depth = 0u8;
    let mut palette_entries = 0usize;
    while pos < data.len() {
        let head = data.get(pos..pos + 8).ok_or(Error::Malformed)?;
        let len = be32(head) as usize;
        w.entry(len)?;
        let end = pos
            .checked_add(12)
            .and_then(|v| v.checked_add(len))
            .ok_or(Error::Malformed)?;
        let chunk = data.get(pos..end).ok_or(Error::Malformed)?;
        let kind = &head[4..8];
        let payload = &chunk[8..8 + len];
        if crc32(&chunk[4..8 + len]) != be32(&chunk[8 + len..]) {
            return Err(Error::Malformed);
        }
        if !header && kind != b"IHDR" {
            return Err(Error::Malformed);
        }
        match kind {
            b"IHDR" => {
                if header
                    || len != 13
                    || be32(payload) == 0
                    || be32(&payload[4..]) == 0
                    || payload[10] != 0
                    || payload[11] != 0
                    || payload[12] > 1
                {
                    return Err(Error::Malformed);
                }
                depth = payload[8];
                color = payload[9];
                let valid_depth = match payload[9] {
                    0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                    2 | 4 | 6 => matches!(depth, 8 | 16),
                    3 => matches!(depth, 1 | 2 | 4 | 8),
                    _ => false,
                };
                if !valid_depth {
                    return Err(Error::Malformed);
                }
                header = true;
                w.add(chunk)?;
            }
            b"IDAT" => {
                pixels = true;
                w.add(chunk)?;
            }
            b"IEND" => {
                if len != 0 || !pixels {
                    return Err(Error::Malformed);
                }
                w.add(chunk)?;
                return Ok(());
            }
            b"PLTE" => {
                if pixels || len == 0 || len > 768 || !len.is_multiple_of(3) {
                    return Err(Error::Malformed);
                }
                palette_entries = len / 3;
                w.add(chunk)?;
            }
            b"acTL" => {
                if animation || pixels || len != 8 || be32(payload) == 0 {
                    return Err(Error::Malformed);
                }
                animation = true;
                w.add(chunk)?;
            }
            b"fcTL" | b"fdAT" => {
                if !animation
                    || (kind == b"fcTL" && len != 26)
                    || (kind == b"fdAT" && len < 4)
                    || len < 4
                    || be32(payload) != sequence
                {
                    return Err(Error::Malformed);
                }
                sequence = sequence.checked_add(1).ok_or(Error::Malformed)?;
                w.add(chunk)?;
            }
            b"tEXt" | b"zTXt" | b"iTXt" | b"tIME" => {}
            b"eXIf" => {
                if let Some(exif) = orientation(payload, w.config)? {
                    png_write(w, kind, &exif)?;
                    w.retain("orientation");
                }
            }
            b"iCCP" => {
                w.add(chunk)?;
                w.retain("icc-profile");
            }
            b"gAMA" | b"cHRM" | b"sRGB" | b"sBIT" | b"tRNS" | b"bKGD" | b"pHYs" | b"cICP"
            | b"mDCV" | b"cLLI" => {
                let valid = match kind {
                    b"gAMA" => len == 4 && be32(payload) != 0,
                    b"cHRM" => len == 32,
                    b"sRGB" => len == 1 && payload[0] <= 3,
                    b"pHYs" => len == 9 && payload[8] <= 1,
                    b"cICP" => len == 4 && payload[3] <= 1,
                    b"mDCV" => len == 24,
                    b"cLLI" => len == 8,
                    b"sBIT" => {
                        let channels = match color {
                            0 => 1,
                            2 | 3 => 3,
                            4 => 2,
                            6 => 4,
                            _ => 0,
                        };
                        len == channels
                            && payload
                                .iter()
                                .all(|&v| v > 0 && v <= if color == 3 { 8 } else { depth })
                    }
                    b"tRNS" => match color {
                        0 => len == 2,
                        2 => len == 6,
                        3 => len > 0 && len <= palette_entries,
                        _ => false,
                    },
                    b"bKGD" => match color {
                        0 | 4 => len == 2,
                        2 | 6 => len == 6,
                        3 => len == 1 && (payload[0] as usize) < palette_entries,
                        _ => false,
                    },
                    _ => false,
                };
                if !valid {
                    return Err(Error::Malformed);
                }
                w.add(chunk)?;
                w.retain("rendering-metadata");
            }
            _ => return Err(Error::UnsupportedFeature),
        }
        pos = end;
    }
    Err(Error::Malformed)
}

/// Read only IFD0's orientation and rebuild a fresh TIFF with no identity fields,
/// offsets to sub-IFDs, maker notes or embedded thumbnails.
fn orientation(data: &[u8], config: &Config) -> Result<Option<Vec<u8>>> {
    let data = data.strip_prefix(b"Exif\0\0").unwrap_or(data);
    if data.len() < 8 {
        return Err(Error::Malformed);
    }
    let little = match &data[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err(Error::Malformed),
    };
    let u16at = |i: usize| -> Result<u16> {
        let a: [u8; 2] = data
            .get(i..i.checked_add(2).ok_or(Error::Malformed)?)
            .ok_or(Error::Malformed)?
            .try_into()
            .unwrap();
        Ok(if little {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        })
    };
    let u32at = |i: usize| -> Result<u32> {
        let a: [u8; 4] = data
            .get(i..i.checked_add(4).ok_or(Error::Malformed)?)
            .ok_or(Error::Malformed)?
            .try_into()
            .unwrap();
        Ok(if little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    };
    if u16at(2)? != 42 {
        return Err(Error::Malformed);
    }
    let offset = u32at(4)? as usize;
    if offset < 8 {
        return Err(Error::Malformed);
    }
    let count = u16at(offset)? as usize;
    if count > config.max_entries {
        return Err(Error::ResourceLimit);
    }
    data.get(offset + 2..offset + 2 + count * 12 + 4)
        .ok_or(Error::Malformed)?;
    let mut value = None;
    for i in 0..count {
        let pos = offset + 2 + i * 12;
        if u16at(pos)? == 0x112 {
            if value.is_some() || u16at(pos + 2)? != 3 || u32at(pos + 4)? != 1 {
                return Err(Error::Malformed);
            }
            let v = u16at(pos + 8)?;
            if !(1..=8).contains(&v) {
                return Err(Error::Malformed);
            }
            value = Some(v);
        }
    }
    Ok(value.map(|v| {
        let mut out =
            b"II\x2a\x00\x08\x00\x00\x00\x01\x00\x12\x01\x03\x00\x01\x00\x00\x00".to_vec();
        out.extend_from_slice(&v.to_le_bytes());
        out.extend_from_slice(&[0; 6]);
        out
    }))
}
fn jpeg_segment(w: &mut Writer<'_>, marker: u8, payload: &[u8]) -> Result<()> {
    w.add(&[0xff, marker])?;
    w.add(&((payload.len() + 2) as u16).to_be_bytes())?;
    w.add(payload)
}
fn jpeg(data: &[u8], w: &mut Writer<'_>) -> Result<()> {
    w.add(&data[..2])?;
    let mut pos = 2;
    let mut scan = false;
    let mut frame = false;
    while pos < data.len() {
        if data[pos] != 0xff {
            return Err(Error::Malformed);
        }
        while data.get(pos) == Some(&0xff) {
            pos += 1;
        }
        let marker = *data.get(pos).ok_or(Error::Malformed)?;
        pos += 1;
        if marker == 0xd9 {
            if !scan || !frame {
                return Err(Error::Malformed);
            }
            w.add(&[0xff, 0xd9])?;
            return Ok(());
        }
        if marker == 0xd8 || marker == 0 || (0xd0..=0xd7).contains(&marker) {
            return Err(Error::Malformed);
        }
        let len = u16::from_be_bytes(
            data.get(pos..pos + 2)
                .ok_or(Error::Malformed)?
                .try_into()
                .unwrap(),
        ) as usize;
        if len < 2 {
            return Err(Error::Malformed);
        }
        w.entry(len)?;
        let payload = data.get(pos + 2..pos + len).ok_or(Error::Malformed)?;
        pos += len;
        match marker {
            0xfe | 0xed => {}
            0xe1 => {
                if payload.starts_with(b"Exif\0\0") {
                    if let Some(tiff) = orientation(payload, w.config)? {
                        let mut exif = b"Exif\0\0".to_vec();
                        exif.extend(tiff);
                        jpeg_segment(w, marker, &exif)?;
                        w.retain("orientation");
                    }
                } else if !payload.starts_with(b"http://ns.adobe.com/") {
                    return Err(Error::UnsupportedFeature);
                }
            }
            0xe0 => {
                if !payload.starts_with(b"JFIF\0") || payload.len() < 14 {
                    return Err(Error::UnsupportedFeature);
                }
                let mut jfif = payload[..14].to_vec();
                jfif[12] = 0;
                jfif[13] = 0;
                jpeg_segment(w, marker, &jfif)?;
            }
            0xe2 => {
                if !payload.starts_with(b"ICC_PROFILE\0") || payload.len() < 14 {
                    return Err(Error::UnsupportedFeature);
                }
                jpeg_segment(w, marker, payload)?;
                w.retain("icc-profile");
            }
            0xee => {
                if payload.len() != 12 || !payload.starts_with(b"Adobe") {
                    return Err(Error::UnsupportedFeature);
                }
                jpeg_segment(w, marker, payload)?;
                w.retain("rendering-metadata");
            }
            0xc0..=0xc2 => {
                if frame
                    || payload.len() < 6
                    || payload[5] == 0
                    || payload.len() != 6 + 3 * payload[5] as usize
                    || payload[1..3] == [0, 0]
                    || payload[3..5] == [0, 0]
                {
                    return Err(Error::Malformed);
                }
                frame = true;
                jpeg_segment(w, marker, payload)?;
            }
            0xda => {
                if !frame
                    || payload.is_empty()
                    || payload[0] == 0
                    || payload.len() != 4 + 2 * payload[0] as usize
                {
                    return Err(Error::Malformed);
                }
                jpeg_segment(w, marker, payload)?;
            }
            0xdd => {
                if payload.len() != 2 {
                    return Err(Error::Malformed);
                }
                jpeg_segment(w, marker, payload)?;
            }
            0xc4 | 0xdb => {
                jpeg_segment(w, marker, payload)?;
            }
            _ => return Err(Error::UnsupportedFeature),
        }
        if marker == 0xda {
            scan = true;
            let start = pos;
            loop {
                let b = *data.get(pos).ok_or(Error::Malformed)?;
                if b != 0xff {
                    pos += 1;
                    continue;
                }
                let mut next = pos + 1;
                while data.get(next) == Some(&0xff) {
                    next += 1;
                }
                let m = *data.get(next).ok_or(Error::Malformed)?;
                if m == 0 || (0xd0..=0xd7).contains(&m) {
                    pos = next + 1;
                } else {
                    break;
                }
            }
            w.add(&data[start..pos])?;
        }
    }
    Err(Error::Malformed)
}
fn webp(data: &[u8], w: &mut Writer<'_>) -> Result<()> {
    let end = (le32(&data[4..8]) as usize)
        .checked_add(8)
        .ok_or(Error::Malformed)?;
    if end < 12 || end > data.len() {
        return Err(Error::Malformed);
    }
    w.add(b"RIFF\0\0\0\0WEBP")?;
    let mut pos = 12;
    let mut image = false;
    let mut extended = None;
    let mut exif_present = false;
    while pos < end {
        let head = data
            .get(pos..pos + 8)
            .filter(|_| pos + 8 <= end)
            .ok_or(Error::Malformed)?;
        let kind = &head[..4];
        let len = le32(&head[4..]) as usize;
        w.entry(len)?;
        let next = pos
            .checked_add(8)
            .and_then(|v| v.checked_add(len))
            .and_then(|v| v.checked_add(len % 2))
            .ok_or(Error::Malformed)?;
        if next > end {
            return Err(Error::Malformed);
        }
        let payload = &data[pos + 8..pos + 8 + len];
        match kind {
            b"EXIF" => {
                if let Some(tiff) = orientation(payload, w.config)? {
                    webp_write(w, kind, &tiff)?;
                    w.retain("orientation");
                    exif_present = true;
                }
            }
            b"XMP " => {}
            b"VP8X" => {
                if pos != 12 || len != 10 || payload[0] & 0xc1 != 0 || payload[1..4] != [0; 3] {
                    return Err(Error::Malformed);
                }
                extended = Some(w.bytes.len() + 8);
                webp_write(w, kind, payload)?;
            }
            b"VP8 " | b"VP8L" => {
                if image {
                    return Err(Error::Malformed);
                }
                image = true;
                webp_write(w, kind, payload)?;
            }
            b"ANMF" => {
                if extended.is_none() || len < 16 {
                    return Err(Error::Malformed);
                }
                validate_frame(&payload[16..], w)?;
                image = true;
                webp_write(w, kind, payload)?;
            }
            b"ICCP" => {
                webp_write(w, kind, payload)?;
                w.retain("icc-profile");
            }
            b"ALPH" => webp_write(w, kind, payload)?,
            b"ANIM" => {
                if len != 6 || extended.is_none() {
                    return Err(Error::Malformed);
                }
                webp_write(w, kind, payload)?;
            }
            _ => return Err(Error::UnsupportedFeature),
        }
        pos = next;
    }
    if !image {
        return Err(Error::Malformed);
    }
    if let Some(p) = extended {
        w.bytes[p] &= !0x0c;
        if exif_present {
            w.bytes[p] |= 0x08;
        }
    }
    let len = u32::try_from(w.bytes.len() - 8).map_err(|_| Error::ResourceLimit)?;
    w.bytes[4..8].copy_from_slice(&len.to_le_bytes());
    Ok(())
}
fn webp_write(w: &mut Writer<'_>, kind: &[u8], payload: &[u8]) -> Result<()> {
    w.add(kind)?;
    w.add(&(payload.len() as u32).to_le_bytes())?;
    w.add(payload)?;
    if !payload.len().is_multiple_of(2) {
        w.add(&[0])?;
    }
    Ok(())
}
fn validate_frame(data: &[u8], w: &mut Writer<'_>) -> Result<()> {
    let mut pos = 0usize;
    let mut image = false;
    while pos < data.len() {
        let head = data.get(pos..pos + 8).ok_or(Error::Malformed)?;
        let len = le32(&head[4..]) as usize;
        w.entry(len)?;
        let next = pos
            .checked_add(8)
            .and_then(|v| v.checked_add(len))
            .and_then(|v| v.checked_add(len % 2))
            .ok_or(Error::Malformed)?;
        if next > data.len() {
            return Err(Error::Malformed);
        }
        match &head[..4] {
            b"VP8 " | b"VP8L" => {
                if image {
                    return Err(Error::Malformed);
                }
                image = true;
            }
            b"ALPH" => {
                if image {
                    return Err(Error::Malformed);
                }
            }
            _ => return Err(Error::UnsupportedFeature),
        }
        pos = next;
    }
    if !image {
        return Err(Error::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_chunk(kind: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
        let mut out = (bytes.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(bytes);
        out.extend_from_slice(&crc32(&out[4..]).to_be_bytes());
        out
    }
    fn png(extra: &[u8]) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend(png_chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0]));
        out.extend(extra);
        out.extend(png_chunk(b"IDAT", &[120, 156, 99, 0, 0, 0, 1, 0, 1]));
        out.extend(png_chunk(b"IEND", &[]));
        out
    }
    fn webp_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            out.push(0);
        }
        out
    }
    fn webp(chunks: &[u8]) -> Vec<u8> {
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&((chunks.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(chunks);
        out
    }
    #[test]
    fn png_removes_text_and_trailer_preserving_pixels() {
        let clean = png(&[]);
        let mut dirty = png(&png_chunk(b"tEXt", b"Author\0CANARY"));
        dirty.extend_from_slice(b"CANARY");
        assert_eq!(strip(&dirty, &Config::default()).unwrap().0, clean);
    }
    #[test]
    fn png_rejects_unknown_chunks_and_invalid_crc() {
        assert_eq!(
            strip(&png(&png_chunk(b"vpAg", b"secret")), &Config::default()),
            Err(Error::UnsupportedFeature)
        );
        let mut bad = png(&[]);
        bad[29] ^= 1;
        assert_eq!(strip(&bad, &Config::default()), Err(Error::Malformed));
    }
    #[test]
    fn jpeg_removes_comments_and_trailer_preserving_scan() {
        let clean = b"\xff\xd8\xff\xc0\x00\x0b\x08\x00\x01\x00\x01\x01\x01\x11\x00\xff\xda\x00\x08\x01\x01\x00\x00\x3f\x00\x01\xff\x00\x02\xff\xd9";
        let mut dirty = clean[..2].to_vec();
        dirty.extend_from_slice(b"\xff\xfe\x00\x08CANARY");
        dirty.extend_from_slice(&clean[2..]);
        dirty.extend_from_slice(b"CANARY");
        assert_eq!(strip(&dirty, &Config::default()).unwrap().0, clean);
    }
    #[test]
    fn webp_removes_xmp_and_updates_flags_and_lengths() {
        let mut header = [0; 10];
        header[0] = 4;
        let mut chunks = webp_chunk(b"VP8X", &header);
        chunks.extend(webp_chunk(b"VP8L", &[0x2f, 0, 0, 0, 0]));
        let mut expected = chunks.clone();
        expected[8] = 0;
        chunks.extend(webp_chunk(b"XMP ", b"CANARY"));
        assert_eq!(
            strip(&webp(&chunks), &Config::default()).unwrap().0,
            webp(&expected)
        );
    }
    #[test]
    fn rejects_invalid_jpeg_frame_and_png_color_type() {
        let invalid = b"\xff\xd8\xff\xc0\x00\x02\xff\xda\x00\x02\x00\xff\xd9";
        assert_eq!(strip(invalid, &Config::default()), Err(Error::Malformed));
        let mut invalid = b"\x89PNG\r\n\x1a\n".to_vec();
        invalid.extend(png_chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 7, 0, 0, 0]));
        invalid.extend(png_chunk(b"IDAT", &[0]));
        invalid.extend(png_chunk(b"IEND", &[]));
        assert_eq!(strip(&invalid, &Config::default()), Err(Error::Malformed));
    }
    #[test]
    fn exif_retains_only_orientation_and_discards_hidden_trailer() {
        let mut tiff = b"II\x2a\x00\x08\x00\x00\x00\x01\x00\x12\x01\x03\x00\x01\x00\x00\x00\x06\x00\x00\x00\x00\x00\x00\x00".to_vec();
        let expected = tiff.clone();
        tiff.extend_from_slice(b"PRIVATE_CANARY");
        let (out, retained) = strip(&png(&png_chunk(b"eXIf", &tiff)), &Config::default()).unwrap();
        assert_eq!(out, png(&png_chunk(b"eXIf", &expected)));
        assert_eq!(retained, vec!["orientation"]);
    }
    #[test]
    fn apng_preserves_animation_controls_and_pixels() {
        let mut chunks = png_chunk(b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
        let mut frame = [0; 26];
        frame[7] = 1;
        frame[11] = 1;
        chunks.extend(png_chunk(b"fcTL", &frame));
        let clean = png(&chunks);
        chunks.extend(png_chunk(b"tEXt", b"Author\0PRIVATE_CANARY"));
        assert_eq!(strip(&png(&chunks), &Config::default()).unwrap().0, clean);
    }
    #[test]
    fn webp_animation_rejects_unknown_nested_payload() {
        let mut header = [0; 10];
        header[0] = 2;
        let mut chunks = webp_chunk(b"VP8X", &header);
        chunks.extend(webp_chunk(b"ANIM", &[0; 6]));
        let mut frame = vec![0; 16];
        frame.extend(webp_chunk(b"JUNK", b"PRIVATE_CANARY"));
        chunks.extend(webp_chunk(b"ANMF", &frame));
        assert_eq!(
            strip(&webp(&chunks), &Config::default()),
            Err(Error::UnsupportedFeature)
        );
    }
    #[test]
    fn rejects_truncation_and_resource_limits() {
        assert_eq!(
            strip(b"\xff\xd8\xff\xe1\xff\xff", &Config::default()),
            Err(Error::Malformed)
        );
        let cfg = Config {
            max_entries: 1,
            ..Config::default()
        };
        assert_eq!(strip(&png(&[]), &cfg), Err(Error::ResourceLimit));
        let cfg = Config {
            max_output_bytes: 8,
            ..Config::default()
        };
        assert_eq!(strip(&png(&[]), &cfg), Err(Error::ResourceLimit));
    }
    #[test]
    fn rejects_identity_payloads_disguised_as_fixed_rendering_chunks() {
        for kind in [
            b"sRGB", b"gAMA", b"cHRM", b"pHYs", b"cICP", b"mDCV", b"cLLI", b"sBIT", b"tRNS",
            b"bKGD",
        ] {
            assert_eq!(
                strip(
                    &png(&png_chunk(kind, b"PRIVATE_CANARY")),
                    &Config::default()
                ),
                Err(Error::Malformed),
                "{kind:?}"
            );
        }
        let mut dri = b"\xff\xd8\xff\xdd\x00\x10PRIVATE_CANARY!".to_vec();
        dri.extend_from_slice(b"\xff\xc0\x00\x0b\x08\x00\x01\x00\x01\x01\x01\x11\x00\xff\xda\x00\x08\x01\x01\x00\x00\x3f\x00\x01\xff\xd9");
        assert_eq!(strip(&dri, &Config::default()), Err(Error::Malformed));
    }
}
