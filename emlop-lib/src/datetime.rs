use crate::types::{ArgError, ArgParse};
use atoi::FromRadix10;
use snafu::prelude::*;
use std::str::FromStr;
use time::{Date, Duration, OffsetDateTime, UtcOffset, format_description::FormatItem,
           macros::format_description, parsing::Parsed};

#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone, Copy)]
/// Represents a position to read from/to in `emerge.log`
///
/// See [get_hist()](fn.get_hist.html) and the [ArgParse] impl.
pub enum HistBound {
    /// Unbounded
    None,
    /// Bound by unix timestamp
    Unix(i64),
    /// Bound by time of nth first/last emerge run
    Run(usize),
}

/// Parse HistBound from various formats
///
/// * A plain unix timestamp
/// * An absolute date (rfc3339-like, interpreted with the given [UtcOffset], see `parse_date_yyyymmdd()`)
/// * A relative date (see `parse_date_ago()`)
/// * An emerge command number (see `parse_command_num()`)
impl ArgParse<String, UtcOffset> for HistBound {
    fn parse(val: &String, offset: UtcOffset, src: &'static str) -> Result<Self, ArgError> {
        let s = val.trim();
        if let Ok(i) = i64::from_str(s) {
            return Ok(Self::Unix(i));
        }
        if let Some(i) = parse_command_num(s) {
            return Ok(Self::Run(i));
        }
        let ea = match parse_date_yyyymmdd(s, offset) {
            Ok(i) => return Ok(Self::Unix(i)),
            Err(e) => e,
        };
        match parse_date_ago(s, OffsetDateTime::now_utc()) {
            Ok(i) => Ok(Self::Unix(i)),
            Err(er) => {
                let m = format!("Not a unix timestamp, absolute date ({ea}), relative date ({er}), or command index");
                Err(ArgError::new(val, src).msg(m))
            },
        }
    }
}

#[derive(Snafu, Debug)]
#[snafu(context(suffix(Err)))]
enum MyErr<'a> {
    /// {val:?}: {msg}
    Base { msg: &'static str, val: &'a str },
    #[snafu(transparent)]
    ComponentRange { source: time::error::ComponentRange },
    #[snafu(transparent)]
    TryFromParsed { source: time::error::TryFromParsed },
    #[snafu(transparent)]
    ParseFromDescription { source: time::error::ParseFromDescription },
}

/// Parse a command index (parse as 1-based, return as 0-based)
fn parse_command_num(s: &str) -> Option<usize> {
    let (num, pos) = usize::from_radix_10(s.as_bytes());
    match s[pos..].trim() {
        "c" | "command" | "commands" if num > 0 => Some(num - 1),
        "c" | "command" if pos == 0 => Some(0),
        _ => None,
    }
}

/// Parse a number of day/years/hours/etc in the past, relative to current time
fn parse_date_ago(mut s: &str, mut now: OffsetDateTime) -> Result<i64, MyErr<'_>> {
    // Handle no span case
    ensure!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == ','),
            BaseErr { msg: "bad char", val: s });
    ensure!(s.chars().any(|c| c.is_ascii_alphabetic()), BaseErr { msg: "no span", val: s });

    // Handle naked span case
    if let Ok(now) = parse_date_span(1, s.trim(), now) {
        return Ok(now.unix_timestamp());
    }

    while !s.is_empty() {
        // Parse number
        s = s.trim_start_matches([' ', ',']);
        let (num, pos) = i32::from_radix_10(s.as_bytes());
        ensure!(pos != 0, BaseErr { msg: "not a number", val: &s[..(std::cmp::min(s.len(), 5))] });
        // Parse span
        s = s[pos..].trim_start_matches([' ', ',']);
        let (span, _) = s.split_once(|c: char| !c.is_ascii_alphabetic()).unwrap_or((s, ""));
        now = parse_date_span(num, span, now)?;
        // Continue
        s = &s[span.len()..];
    }
    Ok(now.unix_timestamp())
}

fn parse_date_span(num: i32, span: &str, now: OffsetDateTime) -> Result<OffsetDateTime, MyErr<'_>> {
    Ok(match span {
        "y" | "year" | "years" => {
            let year = now.year() - num;
            let month = now.month();
            let d = Date::from_calendar_date(year, month, now.day().min(month.length(year)))?;
            now.replace_date(d)
        },
        "m" | "month" | "months" => {
            let mut month = now.month();
            let mut year = now.year();
            for _ in 0..num {
                month = month.previous();
                if month == time::Month::December {
                    year -= 1;
                }
            }
            let d = Date::from_calendar_date(year, month, now.day().min(month.length(year)))?;
            now.replace_date(d)
        },
        "w" | "week" | "weeks" => now - num * Duration::WEEK,
        "d" | "day" | "days" => now - num * Duration::DAY,
        "h" | "hour" | "hours" => now - num * Duration::HOUR,
        "min" | "mins" | "minute" | "minutes" => now - num * Duration::MINUTE,
        "s" | "sec" | "secs" | "second" | "seconds" => now - num * Duration::SECOND,
        o => return Err(MyErr::Base { msg: "bad span", val: o }),
    })
}

/// Parse rfc3339-like format with added flexibility
fn parse_date_yyyymmdd(s: &str, offset: UtcOffset) -> Result<i64, MyErr<'_>> {
    let mut p = Parsed::new().with_hour_24(0)
                             .unwrap()
                             .with_minute(0)
                             .unwrap()
                             .with_second(0)
                             .unwrap()
                             .with_offset_hour(offset.whole_hours())
                             .unwrap()
                             .with_offset_minute_signed(offset.minutes_past_hour())
                             .unwrap()
                             .with_offset_second_signed(offset.seconds_past_minute())
                             .unwrap();
    const FMT: &[FormatItem<'_>] = format_description!(
                                                       version = 2,
                                                       "[year]-[month]-[day]\
        [optional [[first [T][ ]][hour]:[minute][optional [:[second]]]]]\
        [optional [[first [Z][[offset_hour]:[offset_minute]]]]]"
    );
    let rest = p.parse_items(s.as_bytes(), FMT)?;
    ensure!(rest.is_empty(), BaseErr { msg: "junk at end", val: &s[(s.len() - rest.len())..] });
    Ok(OffsetDateTime::try_from(p)?.unix_timestamp())
}

/// Unix timestamp (i64) wrapper that `Display`s as `YYYY-MM-DDTHH:MM:SSZ`
pub struct FmtUtc(pub i64);
impl std::fmt::Display for FmtUtc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match time::Timestamp::from_seconds(self.0) {
            Ok(t) => write!(f, "{}T{:02}:{:02}:{:02}Z", t.date(), t.hour(), t.minute(), t.second()),
            Err(_) => write!(f, "#{}#", self.0),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn parse_rfc(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect(s)
    }
    fn parse_fromto(s: &str, o: UtcOffset) -> Result<HistBound, ArgError> {
        HistBound::parse(&String::from(s), o, "")
    }
    fn parse_ago(ago: &str, now: &str) -> String {
        parse_date_ago(ago, parse_rfc(now)).map(|i| format!("{}", FmtUtc(i))).unwrap()
    }
    fn ts(t: OffsetDateTime) -> i64 {
        t.unix_timestamp()
    }

    #[test]
    fn date() {
        let tb_unix = |rfc| HistBound::Unix(ts(parse_rfc(rfc)));
        let utc = UtcOffset::UTC;

        // Absolute dates
        assert_eq!(Ok(tb_unix("2018-04-03T00:00:00Z")), parse_fromto(" 1522713600 ", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T00:00:00Z")), parse_fromto(" 2018-04-03 ", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:00Z")), parse_fromto("2018-04-03 01:02", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:03Z")), parse_fromto("2018-04-03 01:02:03", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:03Z")), parse_fromto("2018-04-03T01:02:03", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:03Z")), parse_fromto("2018-04-03T01:02:03Z", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T00:02:03Z")),
                   parse_fromto("2018-04-03T01:02:03+01:00", utc));

        // Different timezone (not calling `get_utcoffset()` because tests are threaded, which makes
        // `UtcOffset::current_local_offset()` error out)
        for secs in [3600, -7200, 5400, -5400, 42, -42] {
            let offset = UtcOffset::from_whole_seconds(secs.try_into().unwrap()).unwrap();
            let tb = HistBound::Unix(ts(parse_rfc("2018-04-03T00:00:00Z")) - secs);
            assert_eq!(Ok(tb), parse_fromto("2018-04-03T00:00", offset));
        }

        // Relative dates
        assert_eq!("2025-05-03T11:57:56Z",
                   parse_ago("1 hour, 3 days  45sec", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-04-01T12:58:41Z", parse_ago("5 weeks", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-05-04T11:58:41Z", parse_ago("2d1h", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-04-29T12:58:41Z", parse_ago("w", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-02-28T01:02:03Z", parse_ago("2m", "2025-04-29T01:02:03Z"));
        assert_eq!("2024-02-29T01:02:03Z", parse_ago("2m", "2024-04-29T01:02:03Z"));
        assert_eq!("2023-02-28T01:02:03Z", parse_ago("y", "2024-02-29T01:02:03Z"));

        // Failure cases
        assert!(parse_fromto("", utc).is_err());
        assert!(parse_fromto(" ", utc).is_err());
        assert!(parse_fromto(",", utc).is_err());
        assert!(parse_fromto("junk2018-04-03T01:01:01", utc).is_err());
        assert!(parse_fromto("2018-04-03T01:01:01junk", utc).is_err());
        assert!(parse_fromto("2018-02-29T01:01:01", utc).is_err());
        assert!(parse_fromto("152271000o", utc).is_err());
        assert!(parse_fromto("1 day 3 centuries", utc).is_err());
        assert!(parse_fromto("a while ago", utc).is_err());
    }

    #[test]
    fn command_num() {
        assert_eq!(parse_command_num("1c"), Some(0));
        assert_eq!(parse_command_num("5c"), Some(4));
        assert_eq!(parse_command_num("c"), Some(0));
        assert_eq!(parse_command_num("1 commands "), Some(0));
        assert_eq!(parse_command_num(""), None);
        assert_eq!(parse_command_num("0c"), None);
        assert_eq!(parse_command_num("0"), None);
        assert_eq!(parse_command_num("1cool"), None);
        assert_eq!(parse_command_num("-1c"), None);
    }
}

#[cfg(feature = "unstable")]
#[cfg(test)]
mod bench {
    extern crate test;
    use super::*;

    #[bench]
    fn parse_ago(b: &mut test::Bencher) {
        let now = OffsetDateTime::now_utc();
        b.iter(move || {
             let _ = parse_date_ago("1 hour, 3 days  45sec", now);
             let _ = parse_date_ago("5 weeks", now);
             let _ = parse_date_ago("2d1h", now);
             let _ = parse_date_ago("w", now);
             let _ = parse_date_ago("2m", now);
             let _ = parse_date_ago("2m", now);
             let _ = parse_date_ago("y", now);
         });
    }
}
