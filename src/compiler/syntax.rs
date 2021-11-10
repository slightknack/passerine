use crate::common::span::Span;
use std::fmt;

// TODO: rename to Static?
/// Represents a static error (syntax, semantics, etc.) found at compile time
#[derive(Debug, PartialEq, Eq)]
pub struct Syntax {
    pub message: String,
    pub span: Span,
}

impl Syntax {
    /// Creates a new static error.
    pub fn error(message: &str, span: &Span) -> Syntax {
        Syntax {
            message: message.to_string(),
            span: span.clone(),
        }
    }
}

impl fmt::Display for Syntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.span.is_empty() {
            fmt::Display::fmt(&self.span, f)?
        };
        write!(f, "Syntax Error: {}", self.message)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::common::source::Source;
    use std::rc::Rc;

    #[test]
    fn error() {
        // This is just a demo to check formatting
        // might not coincide with an actual Passerine error
        let source = Rc::new(Source::source("x = \"Hello, world\" -> y + 1"));
        let error = Syntax::error(
            "Unexpected token '\"Hello, world!\"'",
            &Span::new(&source, 4, 14),
        );

        let target = "In ./source:1:5
   |
 1 | x = \"Hello, world\" -> y + 1
   |     ^^^^^^^^^^^^^^
   |
Syntax Error: Unexpected token '\"Hello, world!\"'\
";

        let result = format!("{}", error);
        assert_eq!(result, target);
    }
}
