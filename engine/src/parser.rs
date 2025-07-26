// parser.rs
// -----------------------------------------------------------------------------
// Recursive‑descent parser for Quant Query Language (QQL)
// -----------------------------------------------------------------------------
// This version is aligned with the *struct*‑style `Token` defined in lexer.rs:
//
// pub struct Token {
//     pub kind: TokenKind,
//     pub line: usize,
//     pub column: usize,
// }
//
// enum TokenKind {
//     Keyword(Keyword),     // e.g. LIVE, HISTORICAL, PULL …
//     Identifier(String),   // field names, ticker symbols (already upper‑cased)
//     Literal(String),      // dates (YYYYMMDD) and intervals (2m, 10d)
//     Comma,
//     Newline,
//     EOF,
// }
// -----------------------------------------------------------------------------
// The parser builds a minimal AST and returns it via `parse()`.
// Focus is on showing how to consume tokens correctly; you can extend the AST
// or enforcement logic as needed.
// -----------------------------------------------------------------------------

use std::iter::Peekable;

use crate::lexer::Lexer;
use crate::lexer::{Keyword, Token, TokenKind}; // assuming you expose Lexer in lexer.rs

/* ------------------------------- AST types ------------------------------- */

#[derive(Debug, Clone)]
pub struct Query {
    pub model: ModelSection,
    pub actions: ActionSection,
    pub graph: Option<GraphSection>, // optional graph section
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelType {
    Live,
    Historical,
    Fundamental,
}

#[derive(Debug, Clone)]
pub struct ModelSection {
    pub model_type: ModelType,
    pub ticker: String,
    pub time_spec: TimeSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeSpec {
    DateRange { from: String, to: String },
    LiveSpec { interval: String, duration: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShowType {
    Table,
    Graph,
}

#[derive(Debug, Clone)]
pub struct ActionSection {
    pub fields: Vec<String>,
    pub calc: Option<Calc>,
    pub show: ShowType, // always true if present
}

#[derive(Debug, Clone)]
pub struct Calc {
    pub inputs: Vec<String>,
    pub operation: Keyword, // Difference, Sum, Multiply, Divide
    pub alias: String,
}

#[derive(Debug, Clone)]
pub struct GraphSection {
    pub commands: Vec<DrawCommand>,
}

#[derive(Debug, Clone)]
pub enum DrawCommand {
    Line(Vec<String>),
    Bar(String),
    Candle {
        open: String,
        high: String,
        low: String,
        close: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawType {
    Line(Vec<f64>),                         // single series for line
    Bar(Vec<f64>),                          // (open, close) pairs for bar
    Candlestick(Vec<(f64, f64, f64, f64)>), // (open, high, low, close) for candlestick
}

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub data: Vec<DrawType>,
    pub axis_labels: Vec<String>,
    pub title: String,
}

/* ------------------------------- ParseError ------------------------------ */

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl ParseError {
    fn new<S: Into<String>>(msg: S, line: usize, column: usize) -> Self {
        Self {
            message: msg.into(),
            line,
            column,
        }
    }

    fn eof<S: Into<String>>(msg: S) -> Self {
        Self {
            message: msg.into(),
            line: 0,
            column: 0,
        }
    }

    fn expected<S: Into<String>>(found_tok: &Token, expected: S) -> Self {
        Self::new(
            format!(
                "expected {} but found {:?}",
                expected.into(),
                found_tok.kind
            ),
            found_tok.line,
            found_tok.column,
        )
    }
}

/* -------------------------------- Parser -------------------------------- */

pub struct Parser<'a> {
    iter: Peekable<Lexer<'a>>, // Lexer implements Iterator<Item = Result<Token, LexError>>
    last_pos: (usize, usize),  // (line, col) used for EOF errors
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            iter: Lexer::new(src).peekable(),
            last_pos: (0, 0),
        }
    }

    // Entry point
    pub fn parse(&mut self) -> Result<Query, ParseError> {
        let model = self.parse_model_section()?;
        let actions = self.parse_action_section()?;
        let graph = self.parse_graph_section().ok(); // graph section is optional
        Ok(Query {
            model,
            actions,
            graph,
        })
    }

    /* --------------------------- token helpers -------------------------- */

    fn next_token(&mut self) -> Result<Token, ParseError> {
        match self.iter.next() {
            Some(Ok(tok)) => {
                self.last_pos = (tok.line, tok.column);
                Ok(tok)
            }
            Some(Err(e)) => Err(ParseError::new(e.message, e.line, e.column)),
            None => Err(ParseError::eof("unexpected EOF")),
        }
    }

    fn peek_token(&mut self) -> Option<Result<&Token, ParseError>> {
        match self.iter.peek() {
            Some(Ok(tok)) => Some(Ok(tok)),
            Some(Err(e)) => Some(Err(ParseError::new(e.message.clone(), e.line, e.column))),
            None => None,
        }
    }

    fn consume_newlines(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_token() {
                Some(Ok(tok)) if matches!(tok.kind, TokenKind::Newline) => {
                    self.next_token()?; // consume
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn expect_keyword(&mut self, kw: Keyword) -> Result<Token, ParseError> {
        let tok = self.next_token()?;
        match &tok.kind {
            TokenKind::Keyword(k) if *k == kw => Ok(tok),
            _ => Err(ParseError::expected(&tok, format!("keyword {:?}", kw))),
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let tok = self.next_token()?;
        match tok.kind {
            TokenKind::Identifier(id) => Ok(id),
            _ => Err(ParseError::expected(&tok, "identifier")),
        }
    }

    fn expect_literal(&mut self) -> Result<String, ParseError> {
        let tok = self.next_token()?;
        match tok.kind {
            TokenKind::Literal(lit) => Ok(lit),
            _ => Err(ParseError::expected(&tok, "literal")),
        }
    }

    fn _expect_comma_or_newline(&mut self) -> Result<(), ParseError> {
        let tok = self.next_token()?;
        match tok.kind {
            TokenKind::Comma | TokenKind::Newline => Ok(()),
            _ => Err(ParseError::expected(&tok, ", or newline")),
        }
    }

    fn expect_eof(&mut self) -> Result<(), ParseError> {
        match self.next_token() {
            Err(ParseError { line: 0, .. }) => Ok(()), // reached iterator end
            Ok(tok) => Err(ParseError::expected(&tok, "EOF")),
            Err(e) => Err(e),
        }
    }

    /* ---------------------- Model‑section parsing ---------------------- */

    fn parse_model_section(&mut self) -> Result<ModelSection, ParseError> {
        self.consume_newlines()?;
        // model_type
        let (model_type, _model_kw) = match self.next_token()? {
            tok @ Token {
                kind: TokenKind::Keyword(Keyword::Live),
                ..
            } => (ModelType::Live, tok),
            tok @ Token {
                kind: TokenKind::Keyword(Keyword::Historical),
                ..
            } => (ModelType::Historical, tok),
            tok @ Token {
                kind: TokenKind::Keyword(Keyword::Fundamental),
                ..
            } => (ModelType::Fundamental, tok),
            tok => {
                return Err(ParseError::expected(
                    &tok,
                    "model type (LIVE | HISTORICAL | FUNDAMENTAL)",
                ))
            }
        };

        self.consume_newlines()?;
        // TICKER symbol
        self.expect_keyword(Keyword::Ticker)?;
        let ticker = self.expect_identifier()?;
        self.consume_newlines()?;

        // time spec
        let time_spec = match model_type {
            ModelType::Live => self.parse_live_spec()?,
            _ => self.parse_date_range()?,
        };
        self.consume_newlines()?;

        Ok(ModelSection {
            model_type,
            ticker,
            time_spec,
        })
    }

    fn parse_live_spec(&mut self) -> Result<TimeSpec, ParseError> {
        self.expect_keyword(Keyword::Tick)?;
        let interval = self.expect_literal()?; // e.g., "2m"
        self.expect_keyword(Keyword::For)?;
        let duration = self.expect_literal()?; // e.g., "10d"
        Ok(TimeSpec::LiveSpec { interval, duration })
    }

    fn parse_date_range(&mut self) -> Result<TimeSpec, ParseError> {
        self.expect_keyword(Keyword::From)?;
        let from = self.expect_literal()?; // date literal
        self.expect_keyword(Keyword::To)?;
        let to = self.expect_literal()?;
        Ok(TimeSpec::DateRange { from, to })
    }

    /* ---------------------- Action‑section parsing --------------------- */

    fn parse_action_section(&mut self) -> Result<ActionSection, ParseError> {
        // PULL
        self.expect_keyword(Keyword::Pull)?;
        let fields = self.parse_field_list()?;
        self.consume_newlines()?;

        // optional CALC
        let calc = match self.peek_token() {
            Some(Ok(tok)) if matches!(tok.kind, TokenKind::Keyword(Keyword::Calc)) => {
                Some(self.parse_calc()?)
            }
            _ => None,
        };
        self.consume_newlines()?;

        // SHOW (required)
        let show = match self.next_token()? {
            Token {
                kind: TokenKind::Keyword(Keyword::ShowTable),
                ..
            } => ShowType::Table,
            Token {
                kind: TokenKind::Keyword(Keyword::Graph),
                ..
            } => ShowType::Graph,
            tok => return Err(ParseError::expected(&tok, "SHOWTABLE or GRAPH")),
        };

        Ok(ActionSection { fields, calc, show })
    }

    fn parse_field_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut fields = Vec::new();
        loop {
            let id = self.expect_identifier()?;
            fields.push(id);
            match self.peek_token() {
                Some(Ok(tok)) if matches!(tok.kind, TokenKind::Comma) => {
                    self.next_token()?; // consume comma and keep looping
                }
                _ => break,
            }
        }
        Ok(fields)
    }

    fn parse_calc(&mut self) -> Result<Calc, ParseError> {
        self.expect_keyword(Keyword::Calc)?;
        let inputs = self.parse_field_list()?;

        // operation keyword
        let op_tok = self.next_token()?;
        let operation = match op_tok.kind {
            TokenKind::Keyword(
                k @ (Keyword::Difference
                | Keyword::Sum
                | Keyword::Multiply
                | Keyword::Divide
                | Keyword::Sma
                | Keyword::Volatility),
            ) => k,
            _ => {
                return Err(ParseError::expected(
                    &op_tok,
                    "CALC operation (DIFFERENCE | SUM | MULTIPLY | DIVIDE | SMA | VOLATILITY)",
                ))
            }
        };

        // CALLED alias
        self.expect_keyword(Keyword::Called)?;
        let alias = self.expect_identifier()?;
        Ok(Calc {
            inputs,
            operation,
            alias,
        })
    }

    fn parse_graph_section(&mut self) -> Result<GraphSection, ParseError> {
        let mut commands = Vec::new();
        self.consume_newlines()?;

        loop {
            match self.peek_token() {
                Some(Ok(tok)) => match &tok.kind {
                    TokenKind::Keyword(Keyword::Line) => {
                        self.next_token()?; // consume LINE
                        let fields = self.parse_field_list()?;
                        commands.push(DrawCommand::Line(fields));
                    }
                    TokenKind::Keyword(Keyword::Bar) => {
                        self.next_token()?; // consume BAR
                        let y = self.expect_identifier()?;
                        commands.push(DrawCommand::Bar(y));
                    }
                    TokenKind::Keyword(Keyword::Candle) => {
                        self.next_token()?; // consume CANDLE
                        let open = self.expect_identifier()?;
                        self._expect_comma_or_newline()?;
                        let high = self.expect_identifier()?;
                        self._expect_comma_or_newline()?;
                        let low = self.expect_identifier()?;
                        self._expect_comma_or_newline()?;
                        let close = self.expect_identifier()?;
                        commands.push(DrawCommand::Candle {
                            open,
                            high,
                            low,
                            close,
                        });
                    }
                    TokenKind::Newline => {
                        self.next_token()?; // skip newlines
                    }
                    _ => break, // end of graph block
                },
                _ => break,
            }
        }

        Ok(GraphSection { commands })
    }
}

/// ----------------------------- Convenience ----------------------------- //  /

/// Parse a QQL source string and get the AST.
pub fn parse(src: &str) -> Result<Query, ParseError> {
    Parser::new(src).parse()
}

/* ======================= UNIT TESTS ======================= */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_historical_query() {
        let src = r#"
            HISTORICAL 
            TICKER aapl
            FROM 20220101 TO 20221231
            PULL field1, field2, field3
            CALC field1, field2 DIFFERENCE CALLED diff_field
            SHOW
        "#;

        let query = parse(&src.replace("\n", " ")).unwrap();
        println!("{:#?}", query);
        assert_eq!(query.model.model_type, ModelType::Historical);
        assert_eq!(query.model.ticker, "aapl");
        assert_eq!(
            query.model.time_spec,
            TimeSpec::DateRange {
                from: "20220101".to_string(),
                to: "20221231".to_string()
            }
        );
        assert_eq!(query.actions.fields, vec!["field1", "field2", "field3"]);
        assert!(query.actions.calc.is_some());
        assert_eq!(query.actions.calc.unwrap().operation, Keyword::Difference);
    }
}
