use crate::{Conf, table::Disp, wtb};
use emlop_lib::{ArgError, ArgParse, DurationStyle, fmt_utctime};
use log::{debug, warn};
use std::{io::Write as _,
          time::{SystemTime, UNIX_EPOCH}};
use time::{Date, Duration, Month, OffsetDateTime, UtcOffset, Weekday,
           format_description::FormatItem, macros::format_description};

/// Get the UtcOffset to parse/display datetimes with.
/// Needs to be called before starting extra threads.
pub fn get_offset(utc: bool) -> UtcOffset {
    if utc {
        UtcOffset::UTC
    } else {
        UtcOffset::current_local_offset().unwrap_or_else(|e| {
                                             warn!("Falling back to UTC: {e}");
                                             UtcOffset::UTC
                                         })
    }
}

// It'd be nice to support user-defined formats, but lifetimes make this a bit akward.
// See <https://github.com/time-rs/time/issues/429>
#[derive(Clone, Copy)]
pub struct DateStyle(&'static [FormatItem<'static>]);
impl Default for DateStyle {
    fn default() -> Self {
        Self(format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"))
    }
}
impl ArgParse<String, ()> for DateStyle {
    fn parse(s: &String, _: (), src: &'static str) -> Result<Self, ArgError> {
        Ok(Self(match s.as_str() {
            "ymd" | "d" => format_description!("[year]-[month]-[day]"),
            "ymdhms" | "dt" => format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
            "ymdhmso" | "dto" => format_description!("[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"),
            "rfc3339" | "3339" => format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory]:[offset_minute]"),
            "rfc2822" | "2822" => format_description!("[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"),
            "compact" => format_description!("[year][month][day][hour][minute][second]"),
            "unix" => &[],
            _ => return Err(ArgError::new(s, src).pos("ymd d ymdhms dt ymdhmso dto rfc3339 3339 rfc2822 2822 compact unix"))
        }))
    }
}

pub struct FmtDate(pub i64);
/// Format dates according to user preferencess
impl Disp for FmtDate {
    fn out(&self, buf: &mut Vec<u8>, conf: &Conf) -> usize {
        let start = buf.len();
        if conf.date_fmt.0.is_empty() {
            write!(buf, "{}", self.0).expect("write to buf");
        } else {
            OffsetDateTime::from_unix_timestamp(self.0).expect("unix from i64")
                                                       .to_offset(conf.date_offset)
                                                       .format_into(buf, &conf.date_fmt.0)
                                                       .expect("write to buf");
        }
        buf.len() - start
    }
}

pub fn epoch_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

#[derive(Clone, Copy)]
pub enum Timespan {
    Year,
    Month,
    Week,
    Day,
    None,
}
impl ArgParse<String, ()> for Timespan {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "y" | "year" => Ok(Self::Year),
            "m" | "month" => Ok(Self::Month),
            "w" | "week" => Ok(Self::Week),
            "d" | "day" => Ok(Self::Day),
            "n" | "none" => Ok(Self::None),
            _ => Err(ArgError::new(v, s).pos("(y)ear (m)onth (w)eek (d)ay (n)one")),
        }
    }
}
impl Timespan {
    /// Given a unix timestamp, advance to the beginning of the next year/month/week/day.
    pub fn next(&self, ts: i64, offset: UtcOffset) -> i64 {
        let d = OffsetDateTime::from_unix_timestamp(ts).unwrap().to_offset(offset).date();
        let d2 = match self {
            Self::Year => Date::from_calendar_date(d.year() + 1, Month::January, 1).unwrap(),
            Self::Month => {
                let year = if d.month() == Month::December { d.year() + 1 } else { d.year() };
                Date::from_calendar_date(year, d.month().next(), 1).unwrap()
            },
            Self::Week => {
                let til_monday = match d.weekday() {
                    Weekday::Monday => 7,
                    Weekday::Tuesday => 6,
                    Weekday::Wednesday => 5,
                    Weekday::Thursday => 4,
                    Weekday::Friday => 3,
                    Weekday::Saturday => 2,
                    Weekday::Sunday => 1,
                };
                d.checked_add(Duration::days(til_monday)).unwrap()
            },
            Self::Day => d.checked_add(Duration::DAY).unwrap(),
            Self::None => panic!("Called next() on a Timespan::None"),
        };
        let res = d2.with_hms(0, 0, 0).unwrap().assume_offset(offset).unix_timestamp();
        debug!("{} + {} = {}", fmt_utctime(ts), self.name(), fmt_utctime(res));
        res
    }

    pub fn at(&self, ts: i64, offset: UtcOffset) -> String {
        let d = OffsetDateTime::from_unix_timestamp(ts).unwrap().to_offset(offset);
        match self {
            Self::Year => d.format(format_description!("[year]")).unwrap(),
            Self::Month => d.format(format_description!("[year]-[month]")).unwrap(),
            Self::Week => d.format(format_description!("[year]-[week_number]")).unwrap(),
            Self::Day => d.format(format_description!("[year]-[month]-[day]")).unwrap(),
            Self::None => String::new(),
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            Self::Year => "Year",
            Self::Month => "Month",
            Self::Week => "Week",
            Self::Day => "Date",
            Self::None => "",
        }
    }
}

/// Wrapper around a duration (seconds) to implement `table::Disp`
///
/// A single integer encodes 3 levels of certainty:
/// * 0..MAX:     Actual value
/// * -2^62..0:   Unknown, rendered as `?`
/// * MIN..-2^62: Unsure, rendered as `{value + 1 + 2^63}?`
pub struct FmtDur(pub i64);
impl FmtDur {
    pub fn unsure(self) -> Self {
        Self(i64::MIN + self.0)
    }
}
impl crate::table::Disp for FmtDur {
    fn out(&self, buf: &mut Vec<u8>, conf: &Conf) -> usize {
        use DurationStyle::*;
        use std::io::Write;
        let sec = self.0;
        let dur = conf.dur.val;
        let qmark = conf.qmark.val;
        let start = buf.len();
        match conf.dur_t {
            _ if sec < i64::MIN / 2 => {
                FmtDur(i64::MAX + sec + 1).out(buf, conf);
                wtb!(buf, "{qmark}?")
            },
            _ if sec < 0 => wtb!(buf, "{qmark}?"),
            Hms if sec >= 3600 => {
                wtb!(buf, "{dur}{}:{:02}:{:02}", sec / 3600, sec % 3600 / 60, sec % 60)
            },
            Hms if sec >= 60 => wtb!(buf, "{dur}{}:{:02}", sec % 3600 / 60, sec % 60),
            Hms | Secs => wtb!(buf, "{dur}{sec}"),
            HmsFixed => wtb!(buf, "{dur}{}:{:02}:{:02}", sec / 3600, sec % 3600 / 60, sec % 60),
            Human if sec == 0 => wtb!(buf, "{dur}0 second"),
            Human => {
                let a = [(sec / 86400, "day"),
                         (sec % 86400 / 3600, "hour"),
                         (sec % 3600 / 60, "minute"),
                         (sec % 60, "second")];
                let mut prefix = dur;
                for (num, what) in a.into_iter().filter(|(n, _)| *n > 0) {
                    wtb!(buf, "{prefix}{num} {what}{}", if num > 1 { "s" } else { "" });
                    prefix = ", ";
                }
            },
        }
        crate::parse::Ansi::len(&buf[start..])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn parse_rfc(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect(s)
    }

    fn ts(t: OffsetDateTime) -> i64 {
        t.unix_timestamp()
    }

    #[test]
    fn timespan_next_() {
        for t in [// input             year       month      week       day
                  "2019-01-01T00:00:00 2020-01-01 2019-02-01 2019-01-07 2019-01-02",
                  "2019-01-01T23:59:59 2020-01-01 2019-02-01 2019-01-07 2019-01-02",
                  "2019-01-30T00:00:00 2020-01-01 2019-02-01 2019-02-04 2019-01-31",
                  "2019-01-31T00:00:00 2020-01-01 2019-02-01 2019-02-04 2019-02-01",
                  "2019-12-31T00:00:00 2020-01-01 2020-01-01 2020-01-06 2020-01-01",
                  "2020-02-28T12:34:00 2021-01-01 2020-03-01 2020-03-02 2020-02-29"]
        {
            // Convert the test string into test data (base input, and results depending on
            // timespan). The same test data works whatever the timezone, but the actual timestamp
            // returned by the function is offset.
            let v: Vec<&str> = t.split_whitespace().collect();
            let (base_s, year_s, month_s, week_s, day_s) = (v[0], v[1], v[2], v[3], v[4]);
            let base_utc = parse_rfc(&format!("{base_s}+00:00"));
            for offset_s in ["+00:00", "+05:00", "-10:30"] {
                let base = parse_rfc(&format!("{base_s}{offset_s}"));
                let year = parse_rfc(&format!("{year_s}T00:00:00{offset_s}"));
                let month = parse_rfc(&format!("{month_s}T00:00:00{offset_s}"));
                let week = parse_rfc(&format!("{week_s}T00:00:00{offset_s}"));
                let day = parse_rfc(&format!("{day_s}T00:00:00{offset_s}"));
                // Check our test data is correct
                let offset = base.offset();
                assert!(base < year && base < month && base < week && base < day,
                        "{base} < {year} / {month} / {week} / {day}");
                assert_eq!(ts(base), ts(base_utc) - offset.whole_seconds() as i64);
                assert_eq!(Month::January, year.month());
                assert_eq!(1, year.day());
                assert_eq!(1, month.day());
                assert_eq!(Weekday::Monday, week.weekday());
                // Check the tested code is correct
                assert_eq!(ts(year), Timespan::Year.next(ts(base), offset), "{base} Y {year}");
                assert_eq!(ts(month), Timespan::Month.next(ts(base), offset), "{base} M {month}");
                assert_eq!(ts(week), Timespan::Week.next(ts(base), offset), "{base} W {week}");
                assert_eq!(ts(day), Timespan::Day.next(ts(base), offset), "{base} D {day}");
            }
        }
    }

    #[test]
    fn duration() {
        for (hms, fixed, secs, human, i) in
            [("0", "0:00:00", "0", "0 second", 0),
             ("1", "0:00:01", "1", "1 second", 1),
             ("59", "0:00:59", "59", "59 seconds", 59),
             ("1:00", "0:01:00", "60", "1 minute", 60),
             ("1:01", "0:01:01", "61", "1 minute, 1 second", 61),
             ("59:59", "0:59:59", "3599", "59 minutes, 59 seconds", 3599),
             ("1:00:00", "1:00:00", "3600", "1 hour", 3600),
             ("48:00:01", "48:00:01", "172801", "2 days, 1 second", 172801),
             ("99:59:59", "99:59:59", "359999", "4 days, 3 hours, 59 minutes, 59 seconds", 359999),
             ("100:00:00", "100:00:00", "360000", "4 days, 4 hours", 360000),
             ("?", "?", "?", "?", -1),
             ("?", "?", "?", "?", -123456),
             ("42?", "0:00:42?", "42?", "42 seconds?", i64::MIN + 42)]
        {
            for (st, exp) in [("hms", hms), ("hmsfixed", fixed), ("secs", secs), ("human", human)] {
                let mut buf = vec![];
                FmtDur(i).out(&mut buf, &Conf::from_str(format!("emlop l --color=n --dur {st}")));
                assert_eq!(exp, &String::from_utf8(buf).unwrap());
            }
        }
    }
}
