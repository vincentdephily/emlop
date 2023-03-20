/// Simple Ansi escape parser, sufficient to strip text styling.
///
/// More exotic escapes (that shouldn't comme up in build.log) will cause the rest of the string to
/// be interpreted as a sequence, and stripped. There are crates implementing full ansi support, but
/// they seem overkill for our needs.
#[derive(PartialEq)]
pub enum Ansi {
    /// Normal text
    Txt,
    /// Entered escape sequence
    Esc,
    /// Control Sequence Introducer, includes text styling and cursor control
    EscCSI,
    /// Unimplemented escape type, this variant is a dead-end
    EscUnsupported,
    /// Finished the escape sequence, but not Txt yet
    EscEnd,
}
impl Ansi {
    fn step(&mut self, c: char) {
        use Ansi::*;
        *self = match self {
            // Sequence start
            Txt | EscEnd if c == '\x1B' => Esc,
            // Raw unprintable ascii
            Txt | EscEnd if c < ' ' => EscEnd,
            // Continuation, or return to normal text
            Txt | EscEnd => Txt,
            // CSI start
            Esc if c == '[' => EscCSI,
            // Escaped bel/backspace/tab/lf/ff/cr
            Esc if "78\x0A\x0C\x0D".contains(c) => EscEnd,
            // Not a CSI and not a simple char. Just give up: this shouldn't be in a log file.
            Esc => EscUnsupported,
            // CSI end
            EscCSI if ('@'..='~').contains(&c) => EscEnd,
            // CSI continues
            EscCSI => EscCSI,
            // Give up until end of string
            EscUnsupported => EscUnsupported,
        }
    }
    pub fn strip(s: &str, max: usize) -> String {
        let mut out = String::with_capacity(max + 3);
        let mut state = Self::Txt;
        for c in s.trim().chars() {
            state.step(c);
            if state == Self::Txt {
                if !out.is_empty() || !c.is_whitespace() {
                    out.push(c);
                }
                if out.len() >= max {
                    out += "...";
                    break;
                }
            }
        }
        out
    }
}
