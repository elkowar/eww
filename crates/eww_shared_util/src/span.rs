#[derive(Eq, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Span(pub usize, pub usize, pub usize);

impl Span {
    pub const DUMMY: Span = Span(usize::MAX, usize::MAX, usize::MAX);

    pub fn point(loc: usize, file_id: usize) -> Self {
        Span(loc, loc, file_id)
    }

    /// Get the span that includes this and the other span completely.
    /// Will panic if the spans are from different file_ids.
    pub fn to(mut self, other: Span) -> Self {
        assert!(other.2 == self.2);
        self.1 = other.1;
        self
    }

    pub fn ending_at(mut self, end: usize) -> Self {
        self.1 = end;
        self
    }

    /// Turn this span into a span only highlighting the point it starts at, setting the length to 0.
    pub fn point_span(mut self) -> Self {
        self.1 = self.0;
        self
    }

    /// Turn this span into a span only highlighting the point it ends at, setting the length to 0.
    pub fn point_span_at_end(mut self) -> Self {
        self.0 = self.1;
        self
    }

    pub fn shifted(mut self, n: isize) -> Self {
        self.0 = isize::max(0, self.0 as isize + n) as usize;
        self.1 = isize::max(0, self.0 as isize + n) as usize;
        self
    }

    pub fn is_dummy(&self) -> bool {
        *self == Self::DUMMY
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_dummy() {
            write!(f, "DUMMY")
        } else {
            write!(f, "{}..{}", self.0, self.1)
        }
    }
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub trait Spanned {
    fn span(&self) -> Span;
}
