// parser.rs
// -----------------------------------------------------------------------------
// Recursive-descent parser for Quant Query Language (QQL)
// -----------------------------------------------------------------------------
// This version aligns with a struct-style `Token` defined in lexer.rs:
//
// pub struct Token {
//     pub kind: TokenKind,
//     pub line: usize,
//     pub column: usize,
// }
//
// enum TokenKind {
//     Keyword(Keyword),     // e.g. FRAME, PROVIDER, PULL, CALC …
//     Identifier(String),   // field names, ticker symbols (typically upper-cased)
//     Literal(String),      // dates (YYYYMMDD) and intervals (2m, 10d), numeric literals
//     Comma,
//     Newline,
//     EOF,
//     // (optionally) Comment(String), Equals, etc., depending on your lexer
// }
// -----------------------------------------------------------------------------
// This parser builds a minimal AST and returns it via `parse()`.
// It now supports top-level PROVIDER blocks and requires each FRAME to
// reference a provider instance by name. The old ModelSection is removed.
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
    pub provider: String, // reference to a declared ProviderInstance by name
    pub actions: ActionSection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub providers: HashMap<String, ProviderInstance>,
    pub frame: HashMap<String, Frame>,
    pub graph: Option<GraphSection>,
    pub trade: Option<TradeSection>,
}

impl Query {
    /// Build outbound provider queries from this AST.
    ///
    /// Returns Vec<(name, query)> where `name` is the provider instance name.
    /// Skips providers that are missing backend/ticker or time window.
    pub fn build_provider_queries(&self) -> Result<Vec<(String, String)>, ParseError> {
        let mut out: Vec<(String, String)> = Vec::new();

        for prov in self.providers.values() {
            let backend = match &prov.backend {
                Some(b) if !b.is_empty() => b,
                _ => continue, // incomplete; skip
            };
            let ticker = match &prov.ticker {
                Some(t) if !t.is_empty() => t,
                _ => continue, // incomplete; skip
            };

            match &prov.time_spec {
                Some(TimeSpec::DateRange { from, to }) => {
                    let from_iso = yyyymmdd_to_iso8601_z(from).map_err(|msg| {
                        ParseError::new(format!("provider \"{}\": {}", prov.name, msg), 0, 0)
                    })?;
                    let to_iso = yyyymmdd_to_iso8601_z(to).map_err(|msg| {
                        ParseError::new(format!("provider \"{}\": {}", prov.name, msg), 0, 0)
                    })?;

                    let query = format!(
                        "provider {} search ticker={} date={}..{}",
                        backend, ticker, from_iso, to_iso
                    );
                    out.push((prov.name.clone(), query));
                }
                // Add LIVE support here if desired.
                _ => continue,
            }
        }

        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeSpec {
    DateRange { from: String, to: String },
    LiveSpec { interval: String, duration: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderInstance {
    pub name: String,                  // instance name, e.g. "aapl_data"
    pub backend: Option<String>,       // e.g. "yahoo_finance"
    pub ticker: Option<String>,        // e.g. "AAPL"
    pub time_spec: Option<TimeSpec>,   // FROM/TO or LIVE TICK/FOR
    pub params: Vec<(String, String)>, // PARAM key = value
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calc {
    pub inputs: Vec<String>,
    pub operation: Keyword, // Difference, Sum, Multiply, Divide, Sma, etc.
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSection {
    pub xaxis: String, // x-axis label / frame
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
                candles.iter().map(|&(_o, h, _l, _c)| h).fold(max, f64::max)
            }
            _ => max, // Rectangles don't have a max value
        })
    }

    pub fn min(&self) -> f64 {
        self.data.iter().fold(f64::INFINITY, |min, dt| match dt {
            DrawType::Line(_, values) => values.iter().cloned().fold(min, f64::min),
            DrawType::Bar(_, values) => values.iter().cloned().fold(min, f64::min),
            DrawType::Candlestick(_, candles) => {
                candles.iter().map(|&(_o, _h, l, _c)| l).fold(min, f64::min)
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
        let providers = self.parse_provider_sections()?;
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
            providers,
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
        // Eat *all* non-semantic trivia: newlines and comments
        loop {
            match self.peek_token() {
                Some(Ok(tok)) => {
                    match &tok.kind {
                        TokenKind::Newline => {
                            self.next_token()?;
                        } // consume newline
                        // If your lexer has Comment(String)
                        TokenKind::Comment(_) => {
                            self.next_token()?;
                        } // consume comment
                        _ => break,
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

    /* --------------------------- PROVIDERS (NEW) ------------------------ */

    fn parse_provider_sections(&mut self) -> Result<HashMap<String, ProviderInstance>, ParseError> {
        self.consume_newlines()?;

        let mut map = HashMap::new();

        loop {
            match self.peek_token() {
                Some(Ok(tok)) if tok.kind == TokenKind::Keyword(Keyword::Provider) => {
                    let inst = self.parse_provider_block()?;
                    if map.contains_key(&inst.name) {
                        return Err(ParseError::new(
                            format!("provider \"{}\" is already defined", inst.name),
                            self.last_pos.0,
                            self.last_pos.1,
                        ));
                    }
                    map.insert(inst.name.clone(), inst);
                }
                Some(Ok(tok)) if tok.kind == TokenKind::Newline => {
                    self.next_token()?; // skip stray newline
                }
                _ => break,
            }
        }

        Ok(map)
    }

    fn parse_provider_block(&mut self) -> Result<ProviderInstance, ParseError> {
        // PROVIDER <instance_name>
        self.expect_keyword(Keyword::Provider)?;
        let name = self.expect_identifier()?;
        self.consume_newlines()?;

        let mut backend: Option<String> = None;
        let mut ticker: Option<String> = None;
        let mut time_spec: Option<TimeSpec> = None;
        let mut params: Vec<(String, String)> = Vec::new();

        // Accept lines until we hit something that's not part of a provider body
        loop {
            match self.peek_token() {
                Some(Ok(tok)) => match &tok.kind {
                    // inner backend: "PROVIDER <backend>" or "USING <backend>"
                    TokenKind::Keyword(Keyword::Provider) => {
                        // Disambiguate: if we've already set a backend, this is a NEW provider block.
                        if backend.is_some() {
                            break; // let outer loop handle the next PROVIDER <instance_name>
                        }
                        // Otherwise, treat as "backend" line: PROVIDER <backend_name>
                        self.next_token()?; // consume PROVIDER
                        backend = Some(self.expect_identifier()?);
                        self.consume_newlines()?;
                    }
                    TokenKind::Keyword(Keyword::Using) => {
                        self.next_token()?;
                        backend = Some(self.expect_identifier()?);
                        self.consume_newlines()?;
                    }

                    // TICKER <IDENT>
                    TokenKind::Keyword(Keyword::Ticker) => {
                        self.next_token()?;
                        ticker = Some(self.expect_identifier()?);
                        self.consume_newlines()?;
                    }

                    // FROM <YYYYMMDD> TO <YYYYMMDD>
                    TokenKind::Keyword(Keyword::From) => {
                        self.next_token()?;
                        let from = self.expect_literal()?;
                        self.expect_keyword(Keyword::To)?;
                        let to = self.expect_literal()?;
                        time_spec = Some(TimeSpec::DateRange { from, to });
                        self.consume_newlines()?;
                    }

                    // LIVE TICK <interval> FOR <duration>
                    TokenKind::Keyword(Keyword::Live) => {
                        self.next_token()?;
                        self.expect_keyword(Keyword::Tick)?;
                        let interval = self.expect_literal()?;
                        self.expect_keyword(Keyword::For)?;
                        let duration = self.expect_literal()?;
                        time_spec = Some(TimeSpec::LiveSpec { interval, duration });
                        self.consume_newlines()?;
                    }

                    // PARAM key = value
                    TokenKind::Keyword(Keyword::Param) => {
                        self.next_token()?;
                        let key = self.expect_identifier()?;

                        // accept '=' as Identifier("=") or Literal("="); replace with TokenKind::Equals if you have it
                        let eq_tok = self.next_token()?;
                        let is_eq = match &eq_tok.kind {
                            TokenKind::Identifier(s) if s == "=" => true,
                            TokenKind::Literal(s) if s == "=" => true,
                            // If you have TokenKind::Equals in your lexer, add:
                            // TokenKind::Equals => true,
                            _ => false,
                        };
                        if !is_eq {
                            return Err(ParseError::expected(&eq_tok, "'='"));
                        }

                        let val_tok = self.next_token()?;
                        let val = match val_tok.kind {
                            TokenKind::Identifier(s) | TokenKind::Literal(s) => s,
                            _ => return Err(ParseError::expected(&val_tok, "value")),
                        };
                        params.push((key, val));
                        self.consume_newlines()?;
                    }

                    // skip blank lines
                    TokenKind::Newline => {
                        self.next_token()?;
                    }

                    _ => break, // end of provider block
                },
                _ => break,
            }
        }

        Ok(ProviderInstance {
            name,
            backend,
            ticker,
            time_spec,
            params,
        })
    }

    /* ----------------------------- FRAMES ------------------------------- */

    fn parse_frame_section(&mut self) -> Result<HashMap<String, Frame>, ParseError> {
        self.consume_newlines()?;

        let mut frames = HashMap::new();

        while let Some(Ok(tok)) = self.peek_token() {
            if tok.kind == TokenKind::Keyword(Keyword::Frame) {
                self.next_token()?; // consume FRAME
                let frame_name = self.expect_identifier()?;
                self.consume_newlines()?;

                // Require: PROVIDER <instance_name>
                self.expect_keyword(Keyword::Provider)?;
                let provider_name = self.expect_identifier()?;
                self.consume_newlines()?;

                // Actions (PULL + optional CALC*)
                let actions = self.parse_action_section()?;

                frames.insert(
                    frame_name,
                    Frame {
                        provider: provider_name,
                        actions,
                    },
                );
            } else if tok.kind == TokenKind::Newline {
                self.next_token()?; // skip stray newline
            } else {
                break; // no more FRAME sections
            }
        }

        Ok(frames)
    }

    /* ---------------------- Action-section parsing --------------------- */

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

    /* ----------------------------- GRAPH -------------------------------- */

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

    /* ------------------------------ TRADE ------------------------------- */

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

    /* ------------------------ TimeSpec helpers -------------------------- */

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
}

/// ----------------------------- Convenience ----------------------------- ///

// Private helper for the method above.
fn yyyymmdd_to_iso8601_z(s: &str) -> Result<String, &'static str> {
    if s.len() != 8 || !s.chars().all(|c| c.is_ascii_digit()) {
        return Err("invalid date literal (expected YYYYMMDD)");
    }
    let (y, m, d) = (&s[0..4], &s[4..6], &s[6..8]);
    if !(m >= "01" && m <= "12") || !(d >= "01" && d <= "31") {
        return Err("invalid date components in YYYYMMDD");
    }
    Ok(format!("{y}-{m}-{d}T00:00:00Z"))
}

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
    fn test_provider_and_frame() {
        let src = indoc! {r#"
            -- Provider instance
            PROVIDER aapl_data
                USING yahoo_finance
                TICKER AAPL
                FROM 20220101 TO 20221231

            -- Frame using that provider
            FRAME aapl
                PROVIDER aapl_data
                PULL open, high, low, close
                CALC open, close DIFFERENCE CALLED oc_diff
        "#};

        let query = parse(src).unwrap();

        // provider
        let p = query.providers.get("aapl_data").expect("provider exists");
        assert_eq!(p.backend.as_deref(), Some("yahoo_finance"));
        assert_eq!(p.ticker.as_deref(), Some("AAPL"));
        assert_eq!(
            p.time_spec,
            Some(TimeSpec::DateRange {
                from: "20220101".into(),
                to: "20221231".into()
            })
        );

        // frame
        let f = query.frame.get("aapl").expect("frame exists");
        assert_eq!(f.provider, "aapl_data");
        assert_eq!(f.actions.fields, vec!["open", "high", "low", "close"]);
        assert!(f
            .actions
            .calc
            .as_ref()
            .unwrap()
            .iter()
            .any(|c| c.alias == "oc_diff"));
    }

    #[test]
    fn test_multiple_calcs() {
        let src = r#"
            PROVIDER p
                USING yahoo_finance
                TICKER AAPL
                FROM 20220101 TO 20221231

            FRAME test
                PROVIDER p
                PULL field1, field2
                CALC field1, field2 DIFFERENCE CALLED diff_field
                CALC field1, field2 SUM CALLED sum_field
        "#;

        let query = parse(src).unwrap();
        let calcs = query
            .frame
            .get("test")
            .unwrap()
            .actions
            .calc
            .as_ref()
            .unwrap();
        assert_eq!(calcs.len(), 2);
    }

    #[test]
    fn test_comment_handling_minimal() {
        let src = r#"
            -- comment before
            PROVIDER P1
                USING yahoo_finance
                TICKER AAPL
                FROM 20220101 TO 20221231
            -- frame begins
            FRAME test
                PROVIDER P1
                PULL field1, field2
        "#;

        let query = match parse(src) {
            Ok(query) => query,
            Err(err) => {
                println!("Failed to parse query: {:?}", err);
                panic!("Failed to parse query")
            }
        };

        assert_eq!(query.frame.get("test").unwrap().provider, "P1");
        assert_eq!(
            query.frame.get("test").unwrap().actions.fields,
            vec!["field1", "field2"]
        );
    }
    #[test]
    fn test_query_build_provider_queries_pairs() {
        use indoc::indoc;
        use std::collections::HashMap;

        let src = indoc! {r#"
            PROVIDER sec_aapl
                PROVIDER sec_edgar
                TICKER AAPL
                FROM 20250104 TO 20251005

            PROVIDER yf_nvda
                PROVIDER yahoo_finance
                TICKER NVDA
                FROM 20250905 TO 20251005
        "#};

        let q = parse(src).unwrap();
        let pairs = q.build_provider_queries().unwrap();

        // Convert to map for order-independent assertions
        let map: HashMap<String, String> = pairs.into_iter().collect();

        assert_eq!(
            map.get("sec_aapl").unwrap(),
            "provider sec_edgar search ticker=AAPL date=2025-01-04T00:00:00Z..2025-10-05T00:00:00Z"
        );

        assert_eq!(
            map.get("yf_nvda").unwrap(),
            "provider yahoo_finance search ticker=NVDA date=2025-09-05T00:00:00Z..2025-10-05T00:00:00Z"
        );

        assert_eq!(map.len(), 2);
    }
}
