//! Small helpers that write the Markdown of thread comments, step summaries and
//! PR reviews.
//!
//! These keep the wording and the look of every report the same:
//! a status mark in front of a heading that states the result,
//! counted nouns that agree with their numbers, and user text that cannot
//! break out of a table cell or a code block.

use std::ops::RangeInclusive;

/// Where the status marks are hosted.
///
/// These are the cpp-linter brand mark (an amber circle) with a check, a cross or an
/// exclamation mark inside, see `branding/status/` in the cpp-linter/.github repository.
pub const STATUS_MARK_URL: &str =
    "https://raw.githubusercontent.com/cpp-linter/.github/main/branding/status";

/// The overall result of a report, shown as a mark in front of its heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Nothing to report.
    Passed,
    /// Findings, but nothing that clang reported as an error.
    Warned,
    /// An error from clang-tidy, or a file that clang-format failed to run on.
    Failed,
}

impl Status {
    /// The text glyph for this status, used where an image does not fit (eg. in a table).
    pub fn glyph(self) -> &'static str {
        match self {
            Status::Passed => "✔",
            Status::Warned => "⚠",
            Status::Failed => "✖",
        }
    }

    /// The `<img>` of this status's mark, sized to sit in front of a level 2 heading.
    ///
    /// The `alt` text is the [`Self::glyph`], so the heading still reads
    /// right where images are not shown.
    pub fn mark(self) -> String {
        let name = match self {
            Status::Passed => "passed",
            Status::Warned => "warned",
            Status::Failed => "failed",
        };
        format!(
            "<img alt=\"{}\" width=\"20\" height=\"20\" align=\"top\" src=\"{STATUS_MARK_URL}/{name}.png\">",
            self.glyph()
        )
    }
}

/// Count a noun: `1 file`, `2 files`.
///
/// Only regular plurals are needed (an `s` is appended).
pub fn plural(count: u64, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// Escape text from clang (or from a file name) for Markdown.
///
/// The text keeps its meaning in a paragraph, a list item or a table cell:
/// `*`, `_` and the like are shown as they are instead of starting emphasis,
/// `<` and `>` are not taken as HTML, a `|` does not end a table cell,
/// and line breaks do not end the list item or table row.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.trim().chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\r' => {}
            '\n' => escaped.push(' '),
            '\\' | '`' | '*' | '_' | '[' | ']' | '|' | '#' | '~' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Describe line ranges in words: `line 12`, `lines 3-11, 20`.
///
/// Ranges that touch or overlap are joined.
pub fn describe_lines(ranges: &[RangeInclusive<u32>]) -> String {
    let mut sorted = ranges
        .iter()
        .filter(|r| !r.is_empty())
        .map(|r| (*r.start(), *r.end()))
        .collect::<Vec<_>>();
    sorted.sort_unstable();
    let mut joined: Vec<(u32, u32)> = vec![];
    for (start, end) in sorted {
        match joined.last_mut() {
            Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
            _ => joined.push((start, end)),
        }
    }
    let single = joined.len() == 1 && joined[0].0 == joined[0].1;
    let list = joined
        .iter()
        .map(|(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} {list}", if single { "line" } else { "lines" })
}

/// A fenced code block that its `body` cannot close.
///
/// The fence is one backtick longer than the longest run of backticks in `body`.
pub fn fenced(language: &str, body: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in body.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat((longest + 1).max(3));
    let newline = if body.ends_with('\n') { "" } else { "\n" };
    format!("{fence}{language}\n{body}{newline}{fence}\n")
}

/// Quote a word for a POSIX shell, if it needs quotes.
pub fn shell_quote(word: &str) -> String {
    let is_plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=+:@,".contains(c));
    if is_plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod test {
    use super::{Status, describe_lines, escape, fenced, plural, shell_quote};

    #[test]
    fn plurals() {
        assert_eq!(plural(0, "file"), "0 files");
        assert_eq!(plural(1, "file"), "1 file");
        assert_eq!(plural(2, "warning"), "2 warnings");
    }

    #[test]
    fn escaped_text() {
        assert_eq!(
            escape("conversion 'int *' to 'char *' in a|b <T>\n"),
            r"conversion 'int \*' to 'char \*' in a\|b &lt;T&gt;"
        );
        assert_eq!(escape("line one\r\nline two"), "line one line two");
    }

    #[test]
    fn lines_in_words() {
        assert_eq!(describe_lines(&[12..=12]), "line 12");
        assert_eq!(describe_lines(&[3..=5, 6..=11, 20..=20]), "lines 3-11, 20");
        assert_eq!(describe_lines(&[20..=20, 4..=4]), "lines 4, 20");
    }

    #[test]
    fn fences() {
        assert_eq!(fenced("bash", "ls"), "```bash\nls\n```\n");
        assert_eq!(fenced("text", "a ```` b\n"), "`````text\na ```` b\n`````\n");
    }

    #[test]
    fn quoted_words() {
        assert_eq!(shell_quote("src/demo.cpp"), "src/demo.cpp");
        assert_eq!(shell_quote("my file.cpp"), "'my file.cpp'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn marks() {
        assert!(Status::Passed.mark().contains("alt=\"✔\""));
        assert!(Status::Failed.mark().ends_with("/failed.png\">"));
        assert_eq!(Status::Warned.glyph(), "⚠");
    }
}
