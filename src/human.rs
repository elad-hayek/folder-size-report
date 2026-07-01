use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(pub u64);

impl ByteSize {
    pub fn parse(input: &str) -> Result<Self, String> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err("size cannot be empty".to_string());
        }

        let split = raw
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
            .unwrap_or(raw.len());
        let (number, suffix) = raw.split_at(split);
        if number.is_empty() {
            return Err(format!("invalid size: {input}"));
        }

        let value: f64 = number
            .parse()
            .map_err(|_| format!("invalid size number: {input}"))?;
        if !value.is_finite() || value < 0.0 {
            return Err(format!("invalid size number: {input}"));
        }

        let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
            "" | "b" | "byte" | "bytes" => 1.0,
            "k" | "kb" | "kib" => 1024.0,
            "m" | "mb" | "mib" => 1024.0_f64.powi(2),
            "g" | "gb" | "gib" => 1024.0_f64.powi(3),
            "t" | "tb" | "tib" => 1024.0_f64.powi(4),
            suffix => return Err(format!("unsupported size suffix: {suffix}")),
        };

        let bytes = value * multiplier;
        if bytes > u64::MAX as f64 {
            return Err(format!("size too large: {input}"));
        }
        Ok(Self(bytes.round() as u64))
    }

    pub fn human(self) -> String {
        format_bytes(self.0)
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.human())
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if size >= 100.0 {
        format!("{size:.0} {}", UNITS[unit])
    } else if size >= 10.0 {
        format!("{size:.1} {}", UNITS[unit])
    } else {
        format!("{size:.2} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sizes() {
        assert_eq!(ByteSize::parse("10KB").unwrap().0, 10 * 1024);
        assert_eq!(ByteSize::parse("50MB").unwrap().0, 50 * 1024 * 1024);
        assert_eq!(ByteSize::parse("1GB").unwrap().0, 1024 * 1024 * 1024);
        assert_eq!(ByteSize::parse("123").unwrap().0, 123);
        assert!(ByteSize::parse("abc").is_err());
    }

    #[test]
    fn format_size_ranges() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024_u64.pow(3)), "1.00 GB");
        assert_eq!(format_bytes(1024_u64.pow(4)), "1.00 TB");
    }
}
