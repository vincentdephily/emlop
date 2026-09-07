use emlop_lib::{Ansi, ArgError};

/// Wrapper for `&str` containing non-displayable ansi control chars
pub struct AnsiStr {
    pub val: &'static str,
    /// Visible len excluding control chars
    pub len: usize,
}
impl From<&'static str> for AnsiStr {
    fn from(val: &'static str) -> Self {
        Self { val, len: Ansi::len(val.as_bytes()) }
    }
}
impl crate::table::Disp for AnsiStr {
    fn out(&self, buf: &mut Vec<u8>, _conf: &crate::config::Conf) -> usize {
        buf.extend_from_slice(self.val.as_bytes());
        self.len
    }
}


/// Struct to hold themable colors
///
/// They are parsed at runtime, but leaked to simplify lifetimes for downstream.
pub struct Theme {
    pub merge: &'static str,
    pub binmerge: &'static str,
    pub unmerge: &'static str,
    pub sync: &'static str,
    pub duration: &'static str,
    pub count: &'static str,
    pub qmark: &'static str,
    pub skip: &'static str,
}
impl Theme {
    pub const fn new() -> Self {
        Self { merge: "\x1B[1;32m",
               binmerge: "\x1B[0;32m",
               unmerge: "\x1B[1;31m",
               sync: "\x1B[1;36m",
               duration: "\x1B[1;35m",
               count: "\x1B[0;33m",
               qmark: "\x1B[0m",
               skip: "\x1B[3;37m" }
    }
    /// Parse "(&lt;field>:&lt;SGR> )+" string to update Self's fields
    ///
    /// &lt;Field> must match a known set, and &lt;SGR> is just checked for valid chars
    pub fn update(mut self, kvs: Option<&String>, src: &'static str) -> Result<Self, ArgError> {
        if let Some(kvs) = kvs {
            for kv in kvs.split_whitespace() {
                let (k, v) =
                    kv.split_once(':').ok_or(ArgError::new(kv, src).msg("Expected <key>:<SGR>"))?;
                if v.chars().any(|c| !"0123456789;".contains(c)) {
                    return Err(ArgError::new(kv, src).msg("Unexpected chars in Ansi SGR sequence"));
                }
                let val = format!("\x1B[{v}m");
                match k {
                    "merge" => self.merge = val.leak(),
                    "binmerge" => self.binmerge = val.leak(),
                    "unmerge" => self.unmerge = val.leak(),
                    "sync" => self.sync = val.leak(),
                    "duration" => self.duration = val.leak(),
                    "count" => self.count = val.leak(),
                    "qmark" => self.qmark = val.leak(),
                    "skip" => self.skip = val.leak(),
                    _ => {
                        let p = "merge binmerge unmerge sync duration count qmark skip";
                        return Err(ArgError::new(kv, src).msg("Unexpected key").pos(p));
                    },
                }
            }
        }
        Ok(self)
    }
}
