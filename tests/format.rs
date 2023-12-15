use std::time::Duration;

use soraka::format::{
    format_bytes, format_duration, GIGABYTE, KILOBYTE, MEGABYTE, PETABYTE, TERABYTE,
};

#[test]
fn verify_bytes_formatting() {
    assert_eq!(format_bytes(0), "~0.00 bits");
    assert_eq!(format_bytes(1), "~1.00 bits");
    assert_eq!(format_bytes(KILOBYTE), "~1.00 kilobytes");
    assert_eq!(format_bytes(KILOBYTE + 1), "~1.00 kilobytes");
    assert_eq!(format_bytes(MEGABYTE), "~1.00 megabytes");
    assert_eq!(format_bytes(MEGABYTE + 1), "~1.00 megabytes");
    assert_eq!(format_bytes(GIGABYTE), "~1.00 gigabytes");
    assert_eq!(format_bytes(GIGABYTE + 1), "~1.00 gigabytes");
    assert_eq!(format_bytes(TERABYTE), "~1.00 terabytes");
    assert_eq!(format_bytes(TERABYTE + 1), "~1.00 terabytes");
    assert_eq!(format_bytes(PETABYTE), "~1.00 petabytes");
    assert_eq!(format_bytes(PETABYTE + 1), "~1.00 petabytes");
    assert_eq!(format_bytes(PETABYTE * 1024 - 1), "~1024.00 petabytes");
}

#[test]
fn verify_duration_formatting() {
    assert_eq!(format_duration(Duration::from_secs(0)), "~0 second(s)");
    assert_eq!(format_duration(Duration::from_secs(1)), "~1 second(s)");
    assert_eq!(format_duration(Duration::from_secs(59)), "~59 second(s)");
    assert_eq!(format_duration(Duration::from_secs(60)), "~1 minute(s)");
    assert_eq!(format_duration(Duration::from_secs(119)), "~1 minute(s)");
    assert_eq!(format_duration(Duration::from_secs(120)), "~2 minute(s)");
    assert_eq!(format_duration(Duration::from_secs(3599)), "~59 minute(s)");
    assert_eq!(format_duration(Duration::from_secs(3600)), "~1 hour(s)");
    assert_eq!(format_duration(Duration::from_secs(7199)), "~1 hour(s)");
    assert_eq!(format_duration(Duration::from_secs(7200)), "~2 hour(s)");
    assert_eq!(format_duration(Duration::from_secs(86399)), "~23 hour(s)");
    assert_eq!(format_duration(Duration::from_secs(86400)), "~1 day(s)");
    assert_eq!(format_duration(Duration::from_secs(172799)), "~1 day(s)");
    assert_eq!(format_duration(Duration::from_secs(172800)), "~2 day(s)");
    assert_eq!(
        format_duration(Duration::from_secs(31535999)),
        "~364 day(s)"
    );
    assert_eq!(format_duration(Duration::from_secs(31536000)), "~1 year(s)");
    assert_eq!(format_duration(Duration::from_secs(63071999)), "~1 year(s)");
    assert_eq!(format_duration(Duration::from_secs(63072000)), "~2 year(s)");
    assert_eq!(
        format_duration(Duration::from_secs(315359999)),
        "~9 year(s)"
    );
}
