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

use crate::lexer::Lexer;
use crate::lexer::{Keyword, Token, TokenKind}; // assuming you expose Lexer in lexer.rs
use polars::frame::DataFrame;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::iter::Peekable;

/* ------------------------------- AST types ------------------------------- */

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub model: ModelSection,
    pub actions: ActionSection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub frame: HashMap<String, Frame>,
    pub graph: Option<GraphSection>,
    pub trade: Option<TradeSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelType {
    Live,
    Historical,
    Fundamental,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSection {
    pub model_type: ModelType,
    pub ticker: String,
    pub time_spec: TimeSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeSpec {
    DateRange { from: String, to: String },
    LiveSpec { interval: String, duration: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShowType {
    Table,
    Graph(GraphSection),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionSection {
    pub fields: Vec<String>,
    pub calc: Option<Vec<Calc>>,
    // pub show: ShowType, // always true if present
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calc {
    pub inputs: Vec<String>,
    pub operation: Keyword, // Difference, Sum, Multiply, Divide
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSection {
    pub xaxis: String, // optional x-axis label
    pub commands: Vec<DrawCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DrawCommand {
    Line {
        name: String,
        series: Vec<String>, // fields to draw
        frame: String,
    },
    Bar {
        name: String,
        y: String, // single field for bar
        frame: String,
    },
    Candle {
        name: String,
        open: String,
        high: String,
        low: String,
        close: String,
        frame: String,
    },
}

impl DrawCommand {
    pub fn get_frame(&self) -> String {
        match self {
            DrawCommand::Line { frame, .. } => frame.clone(),
            DrawCommand::Bar { frame, .. } => frame.clone(),
            DrawCommand::Candle { frame, .. } => frame.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DrawType {
    Line(String, Vec<f64>),                         // single series for line
    Bar(String, Vec<f64>),                          // (open, close) pairs for bar
    Candlestick(String, Vec<(f64, f64, f64, f64)>), // (open, high, low, close) for candlestick
    RedRect(String, Vec<(f64, f64, f64)>),          // (x, y1, y2) for red rectangle
    GreenRect(String, Vec<(f64, f64, f64)>),        // (x, y1, y2) for green rectangle
}

impl DrawType {
    pub fn len(&self) -> usize {
        match self {
            DrawType::Line(_, values) => values.len(),
            DrawType::Bar(_, values) => values.len() / 2,
            DrawType::Candlestick(_, candles) => candles.len(),
            DrawType::RedRect(_, values) => values.len(),
            DrawType::GreenRect(_, values) => values.len(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Graph {
    pub data: Vec<DrawType>,
    pub axis_labels: Vec<String>,
    pub title: String,
}

impl Graph {
    pub fn max(&self) -> f64 {
        self.data.iter().fold(0.0, |max, dt| match dt {
            DrawType::Line(_, values) => values.iter().cloned().fold(max, f64::max),
            DrawType::Bar(_, values) => values.iter().cloned().fold(max, f64::max),
            DrawType::Candlestick(_, candles) => {
                candles.iter().map(|&(o, h, l, c)| h).fold(max, f64::max)
            }
            _ => max, // Rectangles don't have a max value
        })
    }

    pub fn min(&self) -> f64 {
        self.data.iter().fold(f64::INFINITY, |min, dt| match dt {
            DrawType::Line(_, values) => values.iter().cloned().fold(min, f64::min),
            DrawType::Bar(_, values) => values.iter().cloned().fold(min, f64::min),
            DrawType::Candlestick(_, candles) => {
                candles.iter().map(|&(o, h, l, c)| l).fold(min, f64::min)
            }
            _ => min, // Rectangles don't have a min value
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TradeType {
    OptionCall,
    OptionPut,
    Stock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TradeSection {
    pub trade_type: TradeType,
    pub over_frame: String,
    pub entry: Vec<String>,
    pub within_entry: f64,
    pub exit: Vec<String>,
    pub within_exit: f64,
    pub stop_loss: f64,
    pub hold: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Trades {
    pub trades_table: DataFrame,
    pub trades_graph: Vec<([[f64; 2]; 4], [[f64; 2]; 4])>,
    pub trade_summary: crate::utils::trade::TradeSummary,
    pub over_frame: String,
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

#[derive(Clone)]
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
        let frame = self.parse_frame_section()?;

        let graph = match self.parse_graph_section() {
            Ok(g) => g,
            Err(_e) => None,
        };
        let trade = match self.parse_trade_section() {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to parse trade section: {}", e.message);
                None
            }
        };
        Ok(Query {
            frame,
            trade,
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
                Some(Ok(tok)) => {
                    if matches!(tok.kind, TokenKind::Newline) {
                        self.next_token()?; // consume
                    } else if matches!(tok.kind, TokenKind::Comment(_)) {
                        self.next_token()?; // consume comment
                    } else {
                        break; // stop on non-newline/non-comment
                    }
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

    // fn expect_eof(&mut self) -> Result<(), ParseError> {
    //     match self.next_token() {
    //         Err(ParseError { line: 0, .. }) => Ok(()), // reached iterator end
    //         Ok(tok) => Err(ParseError::expected(&tok, "EOF")),
    //         Err(e) => Err(e),
    //     }
    // }

    /* ---------------------- Model‑section parsing ---------------------- */

    fn parse_frame_section(&mut self) -> Result<HashMap<String, Frame>, ParseError> {
        self.consume_newlines()?;

        let mut frames = HashMap::new();

        while let Some(Ok(tok)) = self.peek_token() {
            if tok.kind == TokenKind::Keyword(Keyword::Frame) {
                self.next_token()?; // consume FRAME
                let frame_name = self.expect_identifier()?;
                self.consume_newlines()?;

                let model = self.parse_model_section()?;
                let actions = self.parse_action_section()?;

                frames.insert(frame_name, Frame { model, actions });
            } else {
                break; // no more FRAME sections
            }
        }

        Ok(frames)
    }

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

        // Parse zero or more CALC blocks
        let mut calcs = Vec::new();
        while let Some(Ok(tok)) = self.peek_token() {
            if let TokenKind::Keyword(Keyword::Calc) = tok.kind {
                calcs.push(self.parse_calc()?);
                self.consume_newlines()?;
            } else {
                break;
            }
        }

        let calc = if calcs.is_empty() { None } else { Some(calcs) };
        self.consume_newlines()?;

        // // SHOW (required)
        // let show = match self.next_token()? {
        //     Token {
        //         kind: TokenKind::Keyword(Keyword::ShowTable),
        //         ..
        //     } => ShowType::Table,
        //     Token {
        //         kind: TokenKind::Keyword(Keyword::Graph),
        //         ..
        //     } => {
        //         let graph = self.parse_graph_section()?; // graph section is optional
        //         ShowType::Graph(graph)
        //     }
        //     tok => return Err(ParseError::expected(&tok, "SHOWTABLE or GRAPH")),
        // };

        Ok(ActionSection { fields, calc })
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
                | Keyword::Volatility
                | Keyword::DoubleVolatility
                | Keyword::Constant
                | Keyword::LinearRegression),
            ) => k,
            _ => {
                return Err(ParseError::expected(
                    &op_tok,
                    "CALC operation (DIFFERENCE | SUM | MULTIPLY | DIVIDE | SMA | VOLATILITY | LINEAR_REGRESSION | DOUBLE_VOLATILITY)",
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

    fn parse_graph_section(&mut self) -> Result<Option<GraphSection>, ParseError> {
        let mut commands = Vec::new();
        self.consume_newlines()?;

        self.expect_keyword(Keyword::Graph)?;
        self.consume_newlines()?;

        self.expect_keyword(Keyword::Xaxis)?;
        let xaxis = self.expect_identifier()?;
        self.consume_newlines()?;

        loop {
            match self.peek_token() {
                Some(Ok(tok)) => match &tok.kind {
                    TokenKind::Keyword(Keyword::Line) => {
                        self.next_token()?; // consume LINE
                        let fields = self.parse_field_list()?;

                        self.expect_keyword(Keyword::For)?;
                        let frame = self.expect_identifier()?;

                        for field in &fields {
                            commands.push(DrawCommand::Line {
                                name: field.clone(),
                                series: fields.clone(),
                                frame: frame.clone(),
                            });
                        }
                    }
                    TokenKind::Keyword(Keyword::Bar) => {
                        self.next_token()?; // consume BAR
                        let y = self.expect_identifier()?;

                        self.expect_keyword(Keyword::For)?;
                        let frame = self.expect_identifier()?;
                        commands.push(DrawCommand::Bar {
                            name: y.clone(),
                            y,
                            frame,
                        });
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

                        self.expect_keyword(Keyword::For)?;
                        let frame = self.expect_identifier()?;

                        commands.push(DrawCommand::Candle {
                            name: "Candle".to_string(),
                            open,
                            high,
                            low,
                            close,
                            frame,
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

        Ok(Some(GraphSection { xaxis, commands }))
    }

    fn parse_trade_section(&mut self) -> Result<Option<TradeSection>, ParseError> {
        self.consume_newlines()?;
        match self.peek_token() {
            Some(Ok(tok)) if matches!(tok.kind, TokenKind::Keyword(Keyword::Trade)) => {
                self.next_token()?; // consume TRADE
                self.consume_newlines()?;

                // Trade type
                let trade_type = match self.next_token()? {
                    Token {
                        kind: TokenKind::Keyword(Keyword::OptionCall),
                        ..
                    } => TradeType::OptionCall,
                    Token {
                        kind: TokenKind::Keyword(Keyword::OptionPut),
                        ..
                    } => TradeType::OptionPut,
                    Token {
                        kind: TokenKind::Keyword(Keyword::Stock),
                        ..
                    } => TradeType::Stock,
                    tok => {
                        return Err(ParseError::expected(
                            &tok,
                            "trade type (OPTION CALL | OPTION PUT | STOCK)",
                        ))
                    }
                };
                self.consume_newlines()?;

                // OVER FRAME
                self.expect_keyword(Keyword::OverFrame)?;
                let over_frame = self.expect_identifier()?;

                self.consume_newlines()?;

                // ENTRY
                self.expect_keyword(Keyword::Entry)?;
                let mut entry = Vec::new();
                loop {
                    match self.peek_token() {
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Identifier(_)) => {
                            entry.push(self.expect_identifier()?);
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Comma) => {
                            self.next_token()?; // consume comma
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Literal(_)) => {
                            entry.push(self.expect_literal()?);
                        }
                        _ => break,
                    }
                }
                // Last entry is within_entry (f64)
                let within_entry = entry
                    .pop()
                    .ok_or_else(|| ParseError::eof("missing within_entry value"))?
                    .parse::<f64>()
                    .map_err(|_| ParseError::eof("invalid within_entry value"))?;

                self.consume_newlines()?;

                // EXIT
                self.expect_keyword(Keyword::Exit)?;
                let mut exit = Vec::new();
                loop {
                    match self.peek_token() {
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Identifier(_)) => {
                            exit.push(self.expect_identifier()?);
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Comma) => {
                            self.next_token()?; // consume comma
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Literal(_)) => {
                            exit.push(self.expect_literal()?);
                        }
                        _ => break,
                    }
                }
                // Last exit is within_exit (f64)
                let within_exit = exit
                    .pop()
                    .ok_or_else(|| ParseError::eof("missing within_exit value"))?
                    .parse::<f64>()
                    .map_err(|_| ParseError::eof("invalid within_exit value"))?;

                self.consume_newlines()?;

                // LIMIT (stop_loss)
                self.expect_keyword(Keyword::Limit)?;
                let stop_loss = match self.next_token()? {
                    Token {
                        kind: TokenKind::Literal(lit),
                        ..
                    }
                    | Token {
                        kind: TokenKind::Identifier(lit),
                        ..
                    } => lit
                        .parse::<f64>()
                        .map_err(|_| ParseError::eof("invalid stop_loss value"))?,
                    tok => return Err(ParseError::expected(&tok, "stop_loss value")),
                };
                self.consume_newlines()?;

                // HOLD
                self.expect_keyword(Keyword::Hold)?;
                let hold = match self.next_token()? {
                    Token {
                        kind: TokenKind::Literal(lit),
                        ..
                    }
                    | Token {
                        kind: TokenKind::Identifier(lit),
                        ..
                    } => lit
                        .parse::<i32>()
                        .map_err(|_| ParseError::eof("invalid hold value"))?,
                    tok => return Err(ParseError::expected(&tok, "hold value")),
                };
                self.consume_newlines()?;

                Ok(Some(TradeSection {
                    trade_type,
                    over_frame,
                    entry,
                    within_entry,
                    exit,
                    within_exit,
                    stop_loss,
                    hold,
                }))
            }
            _ => Ok(None),
        }
    }
}

/// ----------------------------- Convenience ----------------------------- ///

/// Parse a QQL source string and get the AST.
pub fn parse(src: &str) -> Result<Query, ParseError> {
    Parser::new(src).parse()
}

/* ======================= UNIT TESTS ======================= */

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_historical_query() {
        let src = indoc! {r#"
            FRAME test
                HISTORICAL 
                TICKER aapl
                FROM 20220101 TO 20221231
                PULL field1, field2, field3
                CALC field1, field2 DIFFERENCE CALLED diff_field
        "#};

        // let query = parse(&src.replace("\n", " ")).unwrap();
        let query = parse(src).unwrap();
        println!("{:#?}", query);
        assert_eq!(
            query.frame.get("test").unwrap().model.model_type,
            ModelType::Historical
        );
        assert_eq!(query.frame.get("test").unwrap().model.ticker, "aapl");
        assert_eq!(
            query.frame.get("test").unwrap().model.time_spec,
            TimeSpec::DateRange {
                from: "20220101".to_string(),
                to: "20221231".to_string()
            }
        );
        assert_eq!(
            query.frame.get("test").unwrap().actions.fields,
            vec!["field1", "field2", "field3"]
        );
        assert!(query.frame.get("test").unwrap().actions.calc.is_some());
        assert_eq!(
            query
                .frame
                .get("test")
                .unwrap()
                .actions
                .calc
                .as_ref()
                .unwrap()[0]
                .operation,
            Keyword::Difference
        );
    }

    #[test]
    fn test_multiple_calcs() {
        let src = r#"
            FRAME test
                HISTORICAL 
                TICKER aapl
                FROM 20220101 TO 20221231
                PULL field1, field2
                CALC field1, field2 DIFFERENCE CALLED diff_field
                CALC field1, field2 SUM CALLED sum_field
        "#;

        // let query = parse(&src.replace("\n", " ")).unwrap();
        let query = parse(src).unwrap();
        assert_eq!(
            query
                .frame
                .get("test")
                .unwrap()
                .actions
                .calc
                .as_ref()
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn test_comment_handling() {
        let src = indoc! {r#"
        -- This is a comment
        FRAME test
            HISTORICAL
            -- Another comment
            TICKER AAPL
            FROM 20220101 TO 20221231
            -- PULL starts here
            PULL field1, field2
        "#};

        let query = parse(src).unwrap();
        assert_eq!(query.frame.get("test").unwrap().model.ticker, "AAPL");
        assert_eq!(
            query.frame.get("test").unwrap().actions.fields,
            vec!["field1", "field2"]
        );
    }
}
