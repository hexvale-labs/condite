/// Parse whole byte quantities. SI spellings intentionally use binary multipliers.
pub fn parse_size(input: &str) -> Result<u64, String> {
    let end = input
        .find(|c: char| !c.is_ascii_digit())
        .ok_or("a size suffix is required (B, KB/KiB, MB/MiB, GB/GiB, TB/TiB)")?;
    if end == 0 {
        return Err("size must start with a nonnegative whole number".into());
    }
    let number: u64 = input[..end].parse().map_err(|_| "size is too large")?;
    let multiplier = match input[end..].to_ascii_lowercase().as_str() {
        "b" => 1,
        "kb" | "kib" => 1 << 10,
        "mb" | "mib" => 1 << 20,
        "gb" | "gib" => 1 << 30,
        "tb" | "tib" => 1 << 40,
        _ => return Err("use B, KB/KiB, MB/MiB, GB/GiB or TB/TiB (binary units)".into()),
    };
    number
        .checked_mul(multiplier)
        .filter(|n| *n <= i64::MAX as u64)
        .ok_or_else(|| "size exceeds the supported byte range".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_require_suffixes_and_use_binary_units() {
        for (input, expected) in [
            ("1B", 1),
            ("100mb", 104_857_600),
            ("1GiB", 1_073_741_824),
            ("2KB", 2048),
            ("0gb", 0),
            ("1tib", 1_099_511_627_776),
        ] {
            assert_eq!(parse_size(input).unwrap(), expected, "{input}");
        }
        for input in [
            "",
            "100",
            "-1mb",
            "1.5gb",
            "1m",
            "1 gb",
            "18446744073709551615tb",
            "9223372036854775808b",
        ] {
            assert!(parse_size(input).is_err(), "{input}");
        }
    }
}
