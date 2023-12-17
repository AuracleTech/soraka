use std::time::Duration;

use soraka::format::{duration, reduce, GIGABYTE, KILOBIT, MEGABYTE, PETABYTE, TERABYTE};

#[test]
fn verify_bytes_formatting() {
    assert_eq!(reduce(0), "~0.00 b");
    assert_eq!(reduce(1), "~1.00 b");
    assert_eq!(reduce(KILOBIT), "~1.00 KB");
    assert_eq!(reduce(KILOBIT + 1), "~1.00 KB");
    assert_eq!(reduce(MEGABYTE), "~1.00 MB");
    assert_eq!(reduce(MEGABYTE + 1), "~1.00 MB");
    assert_eq!(reduce(GIGABYTE), "~1.00 GB");
    assert_eq!(reduce(GIGABYTE + 1), "~1.00 GB");
    assert_eq!(reduce(TERABYTE), "~1.00 TB");
    assert_eq!(reduce(TERABYTE + 1), "~1.00 TB");
    assert_eq!(reduce(PETABYTE), "~1.00 PB");
    assert_eq!(reduce(PETABYTE + 1), "~1.00 PB");
    assert_eq!(reduce(PETABYTE * 1024 - 1), "~1024.00 PB");
}

#[test]
fn verify_duration_formatting() {
    assert_eq!(duration(Duration::from_secs(0)), "~0 nanosecond(s)");
    assert_eq!(duration(Duration::from_nanos(1)), "~1 nanosecond(s)");
    assert_eq!(duration(Duration::from_micros(1)), "~1 microsecond(s)");
    assert_eq!(duration(Duration::from_millis(1)), "~1 millisecond(s)");
    assert_eq!(duration(Duration::from_secs(1)), "~1 second(s)");
    assert_eq!(duration(Duration::from_secs(59)), "~59 second(s)");
    assert_eq!(duration(Duration::from_secs(60)), "~1 minute(s)");
    assert_eq!(duration(Duration::from_secs(119)), "~1 minute(s)");
    assert_eq!(duration(Duration::from_secs(120)), "~2 minute(s)");
    assert_eq!(duration(Duration::from_secs(3599)), "~59 minute(s)");
    assert_eq!(duration(Duration::from_secs(3600)), "~1 hour(s)");
    assert_eq!(duration(Duration::from_secs(7199)), "~1 hour(s)");
    assert_eq!(duration(Duration::from_secs(7200)), "~2 hour(s)");
    assert_eq!(duration(Duration::from_secs(86399)), "~23 hour(s)");
    assert_eq!(duration(Duration::from_secs(86400)), "~1 day(s)");
    assert_eq!(duration(Duration::from_secs(172799)), "~1 day(s)");
    assert_eq!(duration(Duration::from_secs(172800)), "~2 day(s)");
    assert_eq!(duration(Duration::from_secs(31535999)), "~364 day(s)");
    assert_eq!(duration(Duration::from_secs(31536000)), "~1 year(s)");
    assert_eq!(duration(Duration::from_secs(63071999)), "~1 year(s)");
    assert_eq!(duration(Duration::from_secs(63072000)), "~2 year(s)");
    assert_eq!(duration(Duration::from_secs(315359999)), "~9 year(s)");
}
