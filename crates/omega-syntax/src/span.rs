/// Source location tracking for error messages.

/// A position in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    /// Byte offset from the start of the source.
    pub offset: usize,
    /// Line number (1-based).
    pub line: usize,
    /// Column number (1-based).
    pub col: usize,
}

impl Pos {
    pub fn new(offset: usize, line: usize, col: usize) -> Self {
        Pos { offset, line, col }
    }

    pub fn start() -> Self {
        Pos {
            offset: 0,
            line: 1,
            col: 1,
        }
    }
}

/// A span in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub fn new(start: Pos, end: Pos) -> Self {
        Span { start, end }
    }

    pub fn dummy() -> Self {
        Span {
            start: Pos::start(),
            end: Pos::start(),
        }
    }

    pub fn merge(self, other: Span) -> Span {
        let start = if self.start.offset <= other.start.offset {
            self.start
        } else {
            other.start
        };
        let end = if self.end.offset >= other.end.offset {
            self.end
        } else {
            other.end
        };
        Span { start, end }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.start.line, self.start.col)
    }
}

impl std::fmt::Display for Pos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}
