use emlop_lib::{ArgError, ArgParse};

#[derive(Clone, Copy)]
pub enum DurationStyle {
    Hms,
    HmsFixed,
    Secs,
    Human,
}
impl ArgParse<String, ()> for DurationStyle {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "hms" => Ok(Self::Hms),
            "hmsfixed" => Ok(Self::HmsFixed),
            "s" | "secs" => Ok(Self::Secs),
            "h" | "human" => Ok(Self::Human),
            _ => Err(ArgError::new(v, s).pos("hms hmsfixed (s)ecs (h)uman")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutStyle {
    Columns,
    Tab,
}
impl ArgParse<String, bool> for OutStyle {
    fn parse(v: &String, isterm: bool, s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "auto" | "a" => Ok(if isterm { Self::Columns } else { Self::Tab }),
            "tab" | "t" => Ok(Self::Tab),
            "columns" | "c" => Ok(Self::Columns),
            _ => Err(ArgError::new(v, s).pos("(c)olumns (t)ab (a)uto")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Tty {
    Auto,
    In,
    Out,
    Inout,
    None,
}
impl ArgParse<String, ()> for Tty {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "a" | "auto" => Ok(Self::Auto),
            "i" | "in" => Ok(Self::In),
            "o" | "out" => Ok(Self::Out),
            "io" | "inout" => Ok(Self::Inout),
            "n" | "none" => Ok(Self::None),
            _ => Err(ArgError::new(v, s).pos("(a)uto (i)n (o)ut (io)nout (n)one")),
        }
    }
}

#[derive(Clone, Copy)]
/// Type of info shown to the user
pub struct Show {
    pub run: bool,
    pub pkg: bool,
    pub tot: bool,
    pub sync: bool,
    pub merge: bool,
    pub unmerge: bool,
}
impl Show {
    /// merge
    pub const fn m() -> Self {
        Self { run: false, pkg: false, tot: false, sync: false, merge: true, unmerge: false }
    }
    /// run, merge, total
    pub const fn rmt() -> Self {
        Self { run: true, pkg: false, tot: true, sync: false, merge: true, unmerge: false }
    }
    /// package
    pub const fn p() -> Self {
        Self { run: false, pkg: true, tot: false, sync: false, merge: false, unmerge: false }
    }
    /// package, total
    pub const fn pt() -> Self {
        Self { run: false, pkg: true, tot: true, sync: false, merge: false, unmerge: false }
    }
}
impl ArgParse<String, &'static str> for Show {
    fn parse(show: &String, valid: &'static str, src: &'static str) -> Result<Self, ArgError> {
        debug_assert!(valid.is_ascii()); // Because we use `chars()` we need to stick to ascii for `valid`.
        if show.chars().all(|c| valid.contains(c)) {
            Ok(Self { run: show.contains('r') || show.contains('a'),
                      pkg: show.contains('p') || show.contains('a'),
                      tot: show.contains('t') || show.contains('a'),
                      sync: show.contains('s') || show.contains('a'),
                      merge: show.contains('m') || show.contains('a'),
                      unmerge: show.contains('u') || show.contains('a') })
        } else {
            Err(ArgError::new(show, src).msg("Invalid letter").pos(valid))
        }
    }
}
impl std::fmt::Display for Show {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sep = "";
        for (b, s) in [(self.run, "run"),
                       (self.pkg, "pkg"),
                       (self.tot, "total"),
                       (self.sync, "sync"),
                       (self.merge, "merge"),
                       (self.unmerge, "unmerge")]
        {
            if b {
                write!(f, "{sep}{s}")?;
                sep = ",";
            }
        }
        Ok(())
    }
}
