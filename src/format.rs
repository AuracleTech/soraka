use std::time::Duration;

pub const BYTE: u8 = 8;
pub const KILOBYTE: u64 = 1024;
pub const MEGABYTE: u64 = KILOBYTE * 1024;
pub const GIGABYTE: u64 = MEGABYTE * 1024;
pub const TERABYTE: u64 = GIGABYTE * 1024;
pub const PETABYTE: u64 = TERABYTE * 1024;

pub fn bytes(size_in_bits: u64) -> String {
    let size = size_in_bits as f64;

    if size < KILOBYTE as f64 {
        format!("~{:.2} bits", size)
    } else if size < MEGABYTE as f64 {
        format!("~{:.2} kilobytes", size / KILOBYTE as f64)
    } else if size < GIGABYTE as f64 {
        format!("~{:.2} megabytes", size / MEGABYTE as f64)
    } else if size < TERABYTE as f64 {
        format!("~{:.2} gigabytes", size / GIGABYTE as f64)
    } else if size < PETABYTE as f64 {
        format!("~{:.2} terabytes", size / TERABYTE as f64)
    } else {
        format!("~{:.2} petabytes", size / PETABYTE as f64)
    }
}

pub fn duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    let micros = duration.as_micros();
    let millis = duration.as_millis();
    let seconds = duration.as_secs();
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let years = days / 365;

    if years > 0 {
        format!("~{} year(s)", years)
    } else if days > 0 {
        format!("~{} day(s)", days)
    } else if hours > 0 {
        format!("~{} hour(s)", hours)
    } else if minutes > 0 {
        format!("~{} minute(s)", minutes)
    } else if seconds > 0 {
        format!("~{} second(s)", seconds)
    } else if millis > 0 {
        format!("~{} millisecond(s)", millis)
    } else if micros > 0 {
        format!("~{} microsecond(s)", micros)
    } else {
        format!("~{} nanosecond(s)", nanos)
    }
}
