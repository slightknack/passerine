use crate::common::span::Span;
use std::fmt;

/// Represents a runtime error, i.e. a traceback
#[derive(Debug, PartialEq, Eq)]
pub struct Trace {
    kind: String, // TODO: enum?
    message: String,
    spans: Vec<Span>,
}

impl Trace {
    /// Creates a new traceback
    pub fn error(kind: &str, message: &str, spans: Vec<Span>) -> Trace {
        Trace {
            kind: kind.to_string(),
            message: message.to_string(),
            spans,
        }
    }

    /// Used to add context (i.e. function calls) while unwinding the stack.
    pub fn add_context(&mut self, span: Span) {
        self.spans.push(span);
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: better message?
        writeln!(f, "Traceback, most recent call last:")?;

        for span in self.spans.iter().rev() {
            fmt::Display::fmt(span, f)?;
        }

        write!(f, "Runtime {} Error: {}", self.kind, self.message)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::common::source::Source;
    use std::rc::Rc;

    #[test]
    fn traceback() {
        // TODO: this method of checking source code is ugly

        let source = Rc::new(Source::source(
            "incr = x -> x + 1
dub_incr = z -> (incr x) + (incr x)
forever = a -> a = a + (dub_incr a)
forever RandomLabel
",
        ));
        let target = "\
            Traceback, most recent call last:\n\
            In ./source:4:1\n   \
               |\n \
             4 | forever RandomLabel\n   \
               | ^^^^^^^^^^^^^^^^^^^\n   \
               |\n\
            In ./source:3:24\n   \
               |\n \
             3 | forever = a -> a = a + (dub_incr a)\n   \
               |                        ^^^^^^^^^^^^\n   \
               |\n\
            In ./source:2:17\n   \
               |\n \
             2 | dub_incr = z -> (incr x) + (incr x)\n   \
               |                 ^^^^^^^^\n   \
               |\n\
            In ./source:1:13\n   \
               |\n \
             1 | incr = x -> x + 1\n   \
               |             ^^^^^\n   \
               |\n\
            Runtime Type Error: Can\'t add Label to Label\
        ";

        let traceback = Trace::error(
            "Type",
            "Can't add Label to Label",
            vec![
                (Span::new(&source, 12, 5)),
                (Span::new(&source, 34, 8)),
                (Span::new(&source, 77, 12)),
                (Span::new(&source, 90, 19)),
            ],
        );

        let result = format!("{}", traceback);
        assert_eq!(result, target);
    }
}
