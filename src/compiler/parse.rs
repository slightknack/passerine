use std::{
    mem,
    convert::TryFrom,
};

use crate::common::{
    span::{Span, Spanned},
    data::Data,
};

use crate::compiler::{
    syntax::Syntax,
    token::Token,
    ast::{AST, ASTPattern, ArgPattern},
};

/// Simple function that parses a token stream into an AST.
/// Exposes the functionality of the `Parser`.
pub fn parse(tokens: Vec<Spanned<Token>>) -> Result<Spanned<AST>, Syntax> {
    let mut parser = Parser::new(tokens);
    let ast = parser.body(Token::End)?;
    parser.consume(Token::End)?;
    Ok(Spanned::new(ast, Span::empty()))
}

/// We're using a Pratt parser, so this little enum
/// defines different precedence levels.
/// Each successive level is higher, so, for example,
/// `* > +`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Prec {
    None = 0,
    Assign,
    Pair,
    Lambda,

    Logic,

    AddSub,
    MulDiv,
    Pow,

    Compose, // TODO: where should this be, precedence-wise?
    Call,
    End,
}

impl Prec {
    /// Increments precedence level to cause the
    /// parser to associate infix operators to the left.
    /// For example, addition is left-associated:
    /// ```build
    /// Prec::Addition.associate_left()
    /// ```
    /// `a + b + c` left-associated becomes `(a + b) + c`.
    /// By default, the parser accociates right.
    pub fn associate_left(&self) -> Prec {
        if let Prec::End = self { panic!("Can not associate further left") }
        unsafe { mem::transmute(*self as u8 + 1) }
    }
}

/// Constructs an `AST` from a token stream.
/// Note that this struct should not be controlled manually,
/// use the `parse` function instead.
#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    index:  usize,
}

impl Parser {
    /// Create a new `parser`.
    pub fn new(tokens: Vec<Spanned<Token>>) -> Parser {
        Parser { tokens, index: 0 }
    }

    // Cookie Monster's Helper Functions:

    // NOTE: Maybe don't return bool?
    /// Consumes all seperator tokens, returning whether there were any.
    pub fn sep(&mut self) -> bool {
        if self.tokens[self.index].item != Token::Sep { false } else {
            while self.tokens[self.index].item == Token::Sep {
                self.index += 1;
            };
            true
        }
    }

    // TODO: merge with sep?
    /// Returns the next non-sep tokens,
    /// without advancing the parser.
    pub fn draw(&self) -> &Spanned<Token> {
        let mut offset = 0;

        while self.tokens[self.index + offset].item == Token::Sep {
            offset += 1;
        }

        &self.tokens[self.index + offset]
    }

    /// Returns the current token then advances the parser.
    pub fn advance(&mut self) -> &Spanned<Token> {
        self.index += 1;
        &self.tokens[self.index - 1]
    }

    /// Returns the first token.
    pub fn current(&self) -> &Spanned<Token> {
        &self.tokens[self.index]
    }

    /// Returns the first non-Sep token.
    pub fn skip(&mut self) -> &Spanned<Token> {
        self.sep();
        self.current()
    }

    /// Throws an error if the next token is unexpected.
    /// I get one funny error message and this is it.
    /// The error message returned by this function will be changed frequently
    pub fn unexpected(&self) -> Syntax {
        let token = self.current();
        Syntax::error(
            &format!("Oopsie woopsie, what's {} doing here?", token.item),
            &token.span
        )
    }

    /// Consumes a specific token then advances the parser.
    /// Can be used to consume Sep tokens, which are normally skipped.
    pub fn consume(&mut self, token: Token) -> Result<&Spanned<Token>, Syntax> {
        self.index += 1;
        let current = &self.tokens[self.index - 1];
        if current.item != token {
            self.index -= 1;
            Err(Syntax::error(&format!("Expected {}, found {}", token, current.item), &current.span))
        } else {
            Ok(current)
        }
    }

    // Core Pratt Parser:

    /// Looks at the current token and parses an infix expression
    pub fn rule_prefix(&mut self) -> Result<Spanned<AST>, Syntax> {
        #[rustfmt::skip]
        match self.skip().item {
            Token::End         => Ok(Spanned::new(AST::Block(vec![]), Span::empty())),

            Token::Syntax      => self.syntax(),
            Token::OpenParen   => self.group(),
            Token::OpenBracket => self.block(),
            Token::Symbol      => self.symbol(),
            Token::Magic       => self.magic(),
            Token::Label       => self.label(),
            Token::Keyword(_)  => self.keyword(),
            Token::Sub         => self.neg(),

            Token::Unit
            | Token::Number(_)
            | Token::String(_)
            | Token::Boolean(_) => self.literal(),

            Token::Sep => unreachable!(),
            _          => Err(Syntax::error("Expected an expression", &self.current().span)),
        }
    }

    /// Looks at the current token and parses the right side of any infix expressions.
    pub fn rule_infix(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        #[rustfmt::skip]
        match self.skip().item {
            Token::Assign  => self.assign(left),
            Token::Lambda  => self.lambda(left),
            Token::Pair    => self.pair(left),
            Token::Compose => self.compose(left),

            Token::Add => self.add(left),
            Token::Sub => self.sub(left),
            Token::Mul => self.mul(left),
            Token::Div => self.div(left),
            Token::Rem => self.rem(left),
            Token::Pow => self.pow(left),

            Token::Equal => self.equal(left),

            Token::End => Err(self.unexpected()),
            Token::Sep => unreachable!(),
            _          => self.call(left),
        }
    }

    /// Looks at the current operator token and determines the precedence
    pub fn prec(&mut self) -> Result<Prec, Syntax> {
        let next = self.draw().item.clone();
        let current = self.current().item.clone();
        let sep = next != current;

        #[rustfmt::skip]
        let prec = match next {
            // infix
            Token::Assign  => Prec::Assign,
            Token::Lambda  => Prec::Lambda,
            Token::Pair    => Prec::Pair,
            Token::Compose => Prec::Compose,

            Token::Equal => Prec::Logic,

              Token::Add
            | Token::Sub => Prec::AddSub,

              Token::Mul
            | Token::Div
            | Token::Rem => Prec::MulDiv,

            Token::Pow => Prec::Pow,

            // postfix
              Token::End
            | Token::CloseParen
            | Token::CloseBracket => Prec::End,

            // prefix
              Token::OpenParen
            | Token::OpenBracket
            | Token::Unit
            | Token::Syntax
            | Token::Magic
            | Token::Symbol
            | Token::Keyword(_)
            | Token::Label
            | Token::Number(_)
            | Token::String(_)
            | Token::Boolean(_) => Prec::Call,

            Token::Sep => unreachable!(),
        };

        if sep && prec == Prec::Call {
            Ok(Prec::End)
        } else {
            Ok(prec)
        }
    }

    // TODO: factor out? only group uses the skip sep flag...
    /// Uses some pratt parser magic to parse an expression.
    /// It's essentially a fold-left over tokens
    /// based on the precedence and content.
    /// Cool stuff.
    pub fn expression(&mut self, prec: Prec, skip_sep: bool) -> Result<Spanned<AST>, Syntax> {
        let mut left = self.rule_prefix()?;

        while {
            if skip_sep { self.sep(); }
            let p = self.prec()?;
            p >= prec && p != Prec::End
        } {
            left = self.rule_infix(left)?;
        }

        Ok(left)
    }

    // Rule Definitions:

    // Prefix:

    /// Constructs an AST for a symbol.
    pub fn symbol(&mut self) -> Result<Spanned<AST>, Syntax> {
        let symbol = self.consume(Token::Symbol)?;

        // nothing is more permanant, but
        // temporary workaround until prelude
        let name = symbol.span.contents();
        if name == "println" {
            return Ok(Spanned::new(
                AST::lambda(
                    Spanned::new(ASTPattern::Symbol("n".into()), Span::empty()),
                    Spanned::new(AST::ffi(
                        "println",
                        Spanned::new(AST::Symbol("n".into()), Span::empty()),
                    ), Span::empty()),
                ),
                symbol.span.clone(),
            ));
        }

        Ok(Spanned::new(AST::Symbol(symbol.span.contents()), symbol.span.clone()))
    }

    /// Parses a keyword.
    /// Note that this is wrapped in a CSTPattern node.
    pub fn keyword(&mut self) -> Result<Spanned<AST>, Syntax> {
        if let Spanned { item: Token::Keyword(name), span } = self.advance() {
            let wrapped = AST::ArgPattern(ArgPattern::Keyword(name.clone()));
            Ok(Spanned::new(wrapped, span.clone()))
        } else {
            unreachable!("Expected a keyword");
        }
    }

    /// Constructs the AST for a literal, such as a number or string.
    pub fn literal(&mut self) -> Result<Spanned<AST>, Syntax> {
        let Spanned { item: token, span } = self.advance();

        #[rustfmt::skip]
        let leaf = match token {
            Token::Unit       => AST::Data(Data::Unit),
            Token::Number(n)  => AST::Data(n.clone()),
            Token::String(s)  => AST::Data(s.clone()),
            Token::Boolean(b) => AST::Data(b.clone()),
            unexpected => return Err(Syntax::error(
                &format!("Expected a literal, found {}", unexpected),
                span
            )),
        };

        Ok(Spanned::new(leaf, span.clone()))
    }

    /// Constructs the ast for a group,
    /// i.e. an expression between parenthesis.
    pub fn group(&mut self) -> Result<Spanned<AST>, Syntax> {
        let start = self.consume(Token::OpenParen)?.span.clone();
        let ast   = self.expression(Prec::None.associate_left(), true)?;
        let end   = self.consume(Token::CloseParen)?.span.clone();
        Ok(Spanned::new(AST::group(ast), Span::combine(&start, &end)))
    }

    /// Parses the body of a block.
    /// A block is one or more expressions, separated by separators.
    /// This is more of a helper function, as it serves as both the
    /// parser entrypoint while still being recursively nestable.
    pub fn body(&mut self, end: Token) -> Result<AST, Syntax> {
        let mut expressions = vec![];

        while self.skip().item != end {
            let ast = self.expression(Prec::None, false)?;
            expressions.push(ast);
            if self.consume(Token::Sep).is_err() {
                break;
            }
        }

        Ok(AST::Block(expressions))
    }

    /// Parse a block as an expression,
    /// Building the appropriate `AST`.
    /// Just a body between curlies.
    pub fn block(&mut self) -> Result<Spanned<AST>, Syntax> {
        let start = self.consume(Token::OpenBracket)?.span.clone();
        let ast = self.body(Token::CloseBracket)?;
        let end = self.consume(Token::CloseBracket)?.span.clone();
        Ok(Spanned::new(ast, Span::combine(&start, &end)))
    }

    // TODO: unwrap from outside in to prevent nesting
    /// Parse a macro definition.
    /// `syntax`, followed by a pattern, followed by a `block`
    pub fn syntax(&mut self) -> Result<Spanned<AST>, Syntax> {
        let start = self.consume(Token::Syntax)?.span.clone();
        let mut after = self.expression(Prec::Call, false)?;

        let mut form = match after.item {
            AST::Form(p) => p,
            _ => return Err(Syntax::error(
                "Expected a pattern and a block after 'syntax'",
                &after.span,
            )),
        };

        let last = form.pop().unwrap();
        after.span = Spanned::build(&form);
        after.item = AST::Form(form);
        let block = match (last.item, last.span) {
            (b @ AST::Block(_), s) => Spanned::new(b, s),
            _ => return Err(Syntax::error(
                "Expected a block after the syntax pattern",
                &after.span,
            )),
        };

        let arg_pat = Parser::arg_pat(after)?;
        let span = Span::join(vec![
            start,
            arg_pat.span.clone(),
            block.span.clone()
        ]);

        Ok(Spanned::new(AST::syntax(arg_pat, block), span))
    }

    /// Parse an `extern` statement.
    /// used for compiler magic and other glue.
    /// takes the form:
    /// ```ignore
    /// magic "FFI String Name" data_to_pass_out
    /// ```
    /// and evaluates to the value returned by the ffi function.
    pub fn magic(&mut self) -> Result<Spanned<AST>, Syntax> {
        let start = self.consume(Token::Magic)?.span.clone();

        let Spanned { item: token, span } = self.advance();
        let name = match token {
            Token::String(Data::String(s))  => s.clone(),
            unexpected => return Err(Syntax::error(
                &format!("Expected a string, found {}", unexpected),
                span
            )),
        };

        let ast = self.expression(Prec::End, false)?;
        let end = ast.span.clone();

        Ok(Spanned::new(
            AST::ffi(&name, ast),
            Span::combine(&start, &end),
        ))
    }

    /// Parse a label.
    /// A label takes the form of `<Label> <expression>`
    pub fn label(&mut self) -> Result<Spanned<AST>, Syntax> {
        let start = self.consume(Token::Label)?.span.clone();
        let ast = self.expression(Prec::End, false)?;
        let end = ast.span.clone();
        Ok(Spanned::new(
            AST::Label(start.contents(), Box::new(ast)),
            Span::combine(&start, &end),
        ))
    }

    pub fn neg(&mut self) -> Result<Spanned<AST>, Syntax> {
        let start = self.consume(Token::Sub)?.span.clone();
        let ast = self.expression(Prec::End, false)?;
        let end = ast.span.clone();

        Ok(
            Spanned::new(AST::ffi("neg", ast),
            Span::combine(&start, &end))
        )
    }

    // Infix:

    /// Parses an argument pattern,
    /// Which converts an `AST` into an `ArgPattern`.
    pub fn arg_pat(ast: Spanned<AST>) -> Result<Spanned<ArgPattern>, Syntax> {
        let item = match ast.item {
            AST::Symbol(s) => ArgPattern::Symbol(s),
            AST::ArgPattern(p) => p,
            AST::Form(f) => {
                let mut mapped = vec![];
                for a in f { mapped.push(Parser::arg_pat(a)?); }
                ArgPattern::Group(mapped)
            }
            _ => return Err(Syntax::error(
                "Unexpected construct inside argument pattern",
                &ast.span
            )),
        };

        Ok(Spanned::new(item, ast.span))
    }

    // TODO: assign and lambda are similar... combine?

    /// Parses an assignment, associates right.
    pub fn assign(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        let left_span = left.span.clone();
        let pattern = left.map(ASTPattern::try_from)
            .map_err(|e| Syntax::error(&e, &left_span))?;

        self.consume(Token::Assign)?;
        let expression = self.expression(Prec::Assign, false)?;
        let combined   = Span::combine(&pattern.span, &expression.span);
        Ok(Spanned::new(AST::assign(pattern, expression), combined))
    }

    /// Parses a lambda definition, associates right.
    pub fn lambda(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        let left_span = left.span.clone();
        let pattern = left.map(ASTPattern::try_from)
            .map_err(|e| Syntax::error(&e, &left_span))?;

        self.consume(Token::Lambda)?;
        let expression = self.expression(Prec::Lambda, false)?;
        let combined   = Span::combine(&pattern.span, &expression.span);
        Ok(Spanned::new(AST::lambda(pattern, expression), combined))
    }

    // TODO: trailing comma must be grouped
    /// Parses a pair operator, i.e. the comma used to build tuples: `a, b, c`.
    pub fn pair(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        let left_span = left.span.clone();
        self.consume(Token::Pair)?;

        let mut tuple = match left.item {
            AST::Tuple(t) => t,
            _ => vec![left],
        };

        let index = self.index;
        let span = if let Ok(item) = self.expression(Prec::Pair.associate_left(), false) {
            let combined = Span::combine(&left_span, &item.span);
            tuple.push(item);
            combined
        } else {
            // restore parser to location right after trailing comma
            self.index = index;
            left_span
        };

        Ok(Spanned::new(AST::Tuple(tuple), span))
    }

    /// Parses a function composition, i.e. `a . b`
    pub fn compose(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.consume(Token::Compose)?;
        let right = self.expression(Prec::Compose.associate_left(), false)?;
        let combined = Span::combine(&left.span, &right.span);
        Ok(Spanned::new(AST::composition(left, right), combined))
    }

    // TODO: names must be full qualified paths.

    /// Parses a left-associative binary operator.
    fn binop(
        &mut self,
        op: Token,
        prec: Prec,
        name: &str,
        left: Spanned<AST>
    ) -> Result<Spanned<AST>, Syntax> {
        self.consume(op)?;
        let right = self.expression(prec, false)?;
        let combined = Span::combine(&left.span, &right.span);

        let arguments = Spanned::new(AST::Tuple(vec![left, right]), combined.clone());
        Ok(Spanned::new(AST::ffi(name, arguments), combined))
    }

    /// Parses an addition, calls out to FFI.
    pub fn add(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Add, Prec::AddSub.associate_left(), "add", left)
    }

    /// Parses a subraction, calls out to FFI.
    pub fn sub(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Sub, Prec::AddSub.associate_left(), "sub", left)
    }

    /// Parses a multiplication, calls out to FFI.
    pub fn mul(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Mul, Prec::MulDiv.associate_left(), "mul", left)
    }

    /// Parses a division, calls out to FFI.
    pub fn div(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Div, Prec::MulDiv.associate_left(), "div", left)
    }

    /// Parses an equality, calls out to FFI.
    pub fn equal(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Equal, Prec::Logic.associate_left(), "equal", left)
    }

    /// Parses an remainder, calls out to FFI.
    pub fn rem(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Rem, Prec::MulDiv.associate_left(), "rem", left)
    }

    /// Parses an power, calls out to FFI.
    pub fn pow(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        self.binop(Token::Pow, Prec::Pow, "pow", left)
    }

    /// Parses a function call.
    /// Function calls are a bit magical,
    /// because they're just a series of expressions.
    /// There's a bit of magic involved -
    /// we interpret anything that isn't an operator as a function call operator.
    /// Then pull a fast one and not parse it like an operator at all.
    pub fn call(&mut self, left: Spanned<AST>) -> Result<Spanned<AST>, Syntax> {
        let argument = self.expression(Prec::Call.associate_left(), false)?;
        let combined = Span::combine(&left.span, &argument.span);

        let mut form = match left.item {
            AST::Form(f) => f,
            _ => vec![left],
        };

        form.push(argument);
        Ok(Spanned::new(AST::Form(form), combined))
    }
}

#[cfg(test)]
mod test {
    use crate::common::{
        data::Data,
        source::Source
    };

    use crate::compiler::lex::lex;
    use super::*;

    #[test]
    pub fn empty() {
        let source = Source::source("");
        let ast = parse(lex(source.clone()).unwrap()).unwrap();
        assert_eq!(ast, Spanned::new(AST::Block(vec![]), Span::empty()));
    }

    #[test]
    pub fn literal() {
        let source = Source::source("x = 55.0");
        let ast = parse(lex(source.clone()).unwrap()).unwrap();
        assert_eq!(
            ast,
            Spanned::new(
                AST::Block(
                    vec![
                        Spanned::new(
                            AST::assign(
                                Spanned::new(ASTPattern::Symbol("x".to_string()), Span::new(&source, 0, 1)),
                                Spanned::new(
                                    AST::Data(Data::Real(55.0)),
                                    Span::new(&source, 4, 4),
                                ),
                            ),
                            Span::new(&source, 0, 8),
                        )
                    ]
                ),
                Span::empty(),
            )
        );
    }

    #[test]
    pub fn lambda() {
        let source = Source::source("x = y -> 3.141592");
        let ast = parse(lex(source.clone()).unwrap()).unwrap();
        // println!("{:#?}", ast);
        assert_eq!(
            ast,
            Spanned::new(
                AST::Block(
                    vec![
                        Spanned::new(
                            AST::assign(
                                Spanned::new(ASTPattern::Symbol("x".to_string()), Span::new(&source, 0, 1)),
                                Spanned::new(
                                    AST::lambda(
                                        Spanned::new(ASTPattern::Symbol("y".to_string()), Span::new(&source, 4, 1)),
                                        Spanned::new(
                                            AST::Data(Data::Real(3.141592)),
                                            Span::new(&source, 9, 8),
                                        ),
                                    ),
                                    Span::new(&source, 4, 13),
                                ),
                            ),
                            Span::new(&source, 0, 17),
                        ),
                    ],
                ),
                Span::empty(),
            )
        );
    }
}
