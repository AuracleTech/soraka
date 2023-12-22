use std::time::Duration;

#[allow(dead_code)]
pub const BYTE: u8 = 8;
pub const KILOBIT: u64 = 1024;
pub const KILOBYTE: u64 = 1024 * BYTE as u64;
pub const MEGABIT: u64 = 1024 * KILOBIT;
pub const MEGABYTE: u64 = 1024 * KILOBYTE;
pub const GIGABIT: u64 = 1024 * MEGABIT;
pub const GIGABYTE: u64 = 1024 * MEGABYTE;
pub const TERABIT: u64 = 1024 * GIGABIT;
pub const TERABYTE: u64 = 1024 * GIGABYTE;
pub const PETABIT: u64 = 1024 * TERABIT;
pub const PETABYTE: u64 = 1024 * TERABYTE;

pub fn bits(bit_size: f64) -> String {
    if bit_size < KILOBIT as f64 {
        format!("~{:.2} b", bit_size)
    } else if bit_size < MEGABYTE as f64 {
        format!("~{:.2} KB", bit_size / KILOBIT as f64)
    } else if bit_size < GIGABYTE as f64 {
        format!("~{:.2} MB", bit_size / MEGABYTE as f64)
    } else if bit_size < TERABYTE as f64 {
        format!("~{:.2} GB", bit_size / GIGABYTE as f64)
    } else if bit_size < PETABYTE as f64 {
        format!("~{:.2} TB", bit_size / TERABYTE as f64)
    } else {
        format!("~{:.2} PB", bit_size / PETABYTE as f64)
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
