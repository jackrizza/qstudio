// parser.rs
// -----------------------------------------------------------------------------
// Recursive-descent parser for Quant Query Language (QQL)
// -----------------------------------------------------------------------------

use crate::lexer::Lexer;
use crate::lexer::{Keyword, Token, TokenKind};
use polars::frame::DataFrame;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::iter::Peekable;

/* ------------------------------- AST types ------------------------------- */

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub provider: String,
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
    pub fn build_provider_queries(&self) -> Result<Vec<(String, String)>, ParseError> {
        let mut out: Vec<(String, String)> = Vec::new();
        for prov in self.providers.values() {
            let backend = match &prov.backend {
                Some(b) if !b.is_empty() => b,
                _ => continue,
            };
            let ticker = match &prov.ticker {
                Some(t) if !t.is_empty() => t,
                _ => continue,
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
    pub name: String,
    pub backend: Option<String>,
    pub ticker: Option<String>,
    pub time_spec: Option<TimeSpec>,
    pub params: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShowType {
    Table,
    Graph(GraphSection),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionSection {
    pub fields: Vec<String>,
    pub calc: Option<Vec<Calc>>, // this is already dependency-ordered when returned by parser
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calc {
    pub inputs: Vec<String>,
    pub operation: Keyword, // Difference, Sum, Multiply, Divide, Sma, Volatility, DoubleVolatility, Constant, LinearRegression
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSection {
    pub xaxis: String,
    pub commands: Vec<DrawCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DrawCommand {
    Line {
        name: String,
        series: Vec<String>,
        frame: String,
    },
    Bar {
        name: String,
        y: String,
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
    Line(String, Vec<f64>),
    Bar(String, Vec<f64>),
    Candlestick(String, Vec<(f64, f64, f64, f64)>),
    RedRect(String, Vec<(f64, f64, f64)>),
    GreenRect(String, Vec<(f64, f64, f64)>),
}

impl DrawType {
    pub fn len(&self) -> usize {
        match self {
            DrawType::Line(_, v) => v.len(),
            DrawType::Bar(_, v) => v.len() / 2,
            DrawType::Candlestick(_, c) => c.len(),
            DrawType::RedRect(_, v) => v.len(),
            DrawType::GreenRect(_, v) => v.len(),
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
            DrawType::Line(_, v) => v.iter().cloned().fold(max, f64::max),
            DrawType::Bar(_, v) => v.iter().cloned().fold(max, f64::max),
            DrawType::Candlestick(_, c) => c.iter().map(|&(_o, h, _l, _c)| h).fold(max, f64::max),
            _ => max,
        })
    }
    pub fn min(&self) -> f64 {
        self.data.iter().fold(f64::INFINITY, |min, dt| match dt {
            DrawType::Line(_, v) => v.iter().cloned().fold(min, f64::min),
            DrawType::Bar(_, v) => v.iter().cloned().fold(min, f64::min),
            DrawType::Candlestick(_, c) => c.iter().map(|&(_o, _h, l, _c)| l).fold(min, f64::min),
            _ => min,
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
    pub fn new<S: Into<String>>(msg: S, line: usize, column: usize) -> Self {
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
    fn expected<S: Into<String>>(found: &Token, expected: S) -> Self {
        Self::new(
            format!("expected {} but found {:?}", expected.into(), found.kind),
            found.line,
            found.column,
        )
    }
}

/* -------------------------------- Parser -------------------------------- */

#[derive(Clone)]
pub struct Parser<'a> {
    iter: Peekable<Lexer<'a>>,
    last_pos: (usize, usize),
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            iter: Lexer::new(src).peekable(),
            last_pos: (0, 0),
        }
    }

    pub fn parse(&mut self) -> Result<Query, ParseError> {
        let providers = self.parse_provider_sections()?;
        let frame = self.parse_frame_section()?;

        let graph = self.parse_graph_section().unwrap_or(None);
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
        loop {
            match self.peek_token() {
                Some(Ok(tok)) => match &tok.kind {
                    TokenKind::Newline => {
                        self.next_token()?;
                    }
                    TokenKind::Comment(_) => {
                        self.next_token()?;
                    }
                    _ => break,
                },
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

    /* --------------------------- PROVIDERS ------------------------------ */

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
                    self.next_token()?;
                }
                _ => break,
            }
        }
        Ok(map)
    }

    fn parse_provider_block(&mut self) -> Result<ProviderInstance, ParseError> {
        self.expect_keyword(Keyword::Provider)?;
        let name = self.expect_identifier()?;
        self.consume_newlines()?;

        let mut backend: Option<String> = None;
        let mut ticker: Option<String> = None;
        let mut time_spec: Option<TimeSpec> = None;
        let mut params: Vec<(String, String)> = Vec::new();

        loop {
            match self.peek_token() {
                Some(Ok(tok)) => match &tok.kind {
                    TokenKind::Keyword(Keyword::Provider) => {
                        if backend.is_some() {
                            break;
                        }
                        self.next_token()?; // inner PROVIDER = backend
                        backend = Some(self.expect_identifier()?);
                        self.consume_newlines()?;
                    }
                    TokenKind::Keyword(Keyword::Using) => {
                        self.next_token()?;
                        backend = Some(self.expect_identifier()?);
                        self.consume_newlines()?;
                    }
                    TokenKind::Keyword(Keyword::Ticker) => {
                        self.next_token()?;
                        ticker = Some(self.expect_identifier()?);
                        self.consume_newlines()?;
                    }
                    TokenKind::Keyword(Keyword::From) => {
                        self.next_token()?;
                        let from = self.expect_literal()?;
                        self.expect_keyword(Keyword::To)?;
                        let to = self.expect_literal()?;
                        time_spec = Some(TimeSpec::DateRange { from, to });
                        self.consume_newlines()?;
                    }
                    TokenKind::Keyword(Keyword::Live) => {
                        self.next_token()?;
                        self.expect_keyword(Keyword::Tick)?;
                        let interval = self.expect_literal()?;
                        self.expect_keyword(Keyword::For)?;
                        let duration = self.expect_literal()?;
                        time_spec = Some(TimeSpec::LiveSpec { interval, duration });
                        self.consume_newlines()?;
                    }
                    TokenKind::Keyword(Keyword::Param) => {
                        self.next_token()?;
                        let key = self.expect_identifier()?;
                        let eq_tok = self.next_token()?;
                        let is_eq = match &eq_tok.kind {
                            TokenKind::Identifier(s) if s == "=" => true,
                            TokenKind::Literal(s) if s == "=" => true,
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
                    TokenKind::Newline => {
                        self.next_token()?;
                    }
                    _ => break,
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
                self.next_token()?;
                let frame_name = self.expect_identifier()?;
                self.consume_newlines()?;

                self.expect_keyword(Keyword::Provider)?;
                let provider_name = self.expect_identifier()?;
                self.consume_newlines()?;

                let actions = self.parse_action_section()?;
                frames.insert(
                    frame_name,
                    Frame {
                        provider: provider_name,
                        actions,
                    },
                );
            } else if tok.kind == TokenKind::Newline {
                self.next_token()?;
            } else {
                break;
            }
        }
        Ok(frames)
    }

    /* ---------------------- Action-section parsing --------------------- */

    fn parse_action_section(&mut self) -> Result<ActionSection, ParseError> {
        self.expect_keyword(Keyword::Pull)?;
        let fields = self.parse_field_list()?;
        self.consume_newlines()?;

        let mut calcs = Vec::new();
        while let Some(Ok(tok)) = self.peek_token() {
            if let TokenKind::Keyword(Keyword::Calc) = tok.kind {
                calcs.push(self.parse_calc()?);
                self.consume_newlines()?;
            } else {
                break;
            }
        }

        // Dependency ordering with relaxed behavior:
        // - numeric literals count as available
        // - if stalled, push unknowns last (no error)
        let calc = if calcs.is_empty() {
            None
        } else {
            let tmp = ActionSection {
                fields: fields.clone(),
                calc: Some(calcs),
            };
            Some(order_calcs_flat(&tmp)?)
        };

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
                    self.next_token()?;
                }
                _ => break,
            }
        }
        Ok(fields)
    }

    fn parse_calc(&mut self) -> Result<Calc, ParseError> {
        self.expect_keyword(Keyword::Calc)?;
        let inputs = self.parse_field_list()?;

        let op_tok = self.next_token()?;
        let operation = match op_tok.kind {
            TokenKind::Keyword(
                k @ (
                    Keyword::Difference
                    | Keyword::Sum
                    | Keyword::Multiply
                    | Keyword::Divide
                    | Keyword::Sma
                    | Keyword::Volatility
                    | Keyword::DoubleVolatility
                    | Keyword::Constant
                    | Keyword::LinearRegression
                )
            ) => k,
            _ => {
                return Err(ParseError::expected(
                    &op_tok,
                    "CALC op (DIFFERENCE|SUM|MULTIPLY|DIVIDE|SMA|VOLATILITY|DOUBLE_VOLATILITY|CONSTANT|LINEAR_REGRESSION)",
                ))
            }
        };

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

        let Some(Ok(tok)) = self.peek_token() else {
            return Ok(None);
        };
        if tok.kind != TokenKind::Keyword(Keyword::Graph) {
            return Ok(None);
        }

        self.next_token()?; // GRAPH
        self.consume_newlines()?;

        self.expect_keyword(Keyword::Xaxis)?;
        let xaxis = self.expect_identifier()?;
        self.consume_newlines()?;

        loop {
            match self.peek_token() {
                Some(Ok(tok)) => match &tok.kind {
                    TokenKind::Keyword(Keyword::Line) => {
                        self.next_token()?;
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
                        self.next_token()?;
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
                        self.next_token()?;
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
                        self.next_token()?;
                    }
                    _ => break,
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
                self.next_token()?; // TRADE
                self.consume_newlines()?;

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
                            "trade type (OPTION CALL|OPTION PUT|STOCK)",
                        ))
                    }
                };
                self.consume_newlines()?;

                self.expect_keyword(Keyword::OverFrame)?;
                let over_frame = self.expect_identifier()?;
                self.consume_newlines()?;

                self.expect_keyword(Keyword::Entry)?;
                let mut entry = Vec::new();
                loop {
                    match self.peek_token() {
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Identifier(_)) => {
                            entry.push(self.expect_identifier()?)
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Comma) => {
                            self.next_token()?;
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Literal(_)) => {
                            entry.push(self.expect_literal()?)
                        }
                        _ => break,
                    }
                }
                let within_entry = entry
                    .pop()
                    .ok_or_else(|| ParseError::eof("missing within_entry"))?
                    .parse::<f64>()
                    .map_err(|_| ParseError::eof("invalid within_entry"))?;
                self.consume_newlines()?;

                self.expect_keyword(Keyword::Exit)?;
                let mut exit = Vec::new();
                loop {
                    match self.peek_token() {
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Identifier(_)) => {
                            exit.push(self.expect_identifier()?)
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Comma) => {
                            self.next_token()?;
                        }
                        Some(Ok(tok)) if matches!(tok.kind, TokenKind::Literal(_)) => {
                            exit.push(self.expect_literal()?)
                        }
                        _ => break,
                    }
                }
                let within_exit = exit
                    .pop()
                    .ok_or_else(|| ParseError::eof("missing within_exit"))?
                    .parse::<f64>()
                    .map_err(|_| ParseError::eof("invalid within_exit"))?;
                self.consume_newlines()?;

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
                        .map_err(|_| ParseError::eof("invalid stop_loss"))?,
                    tok => return Err(ParseError::expected(&tok, "stop_loss value")),
                };
                self.consume_newlines()?;

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
                        .map_err(|_| ParseError::eof("invalid hold"))?,
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

/* ------------------------ TimeSpec helpers -------------------------- */

fn yyyymmdd_to_iso8601_z(s: &str) -> Result<String, &'static str> {
    if s.len() != 8 || !s.chars().all(|c| c.is_ascii_digit()) {
        return Err("invalid date literal (expected YYYYMMDD)");
    }
    let (y, m, d) = (&s[0..4], &s[4..6], &s[6..8]);
    if !(m >= "01" && m <= "12") || !(d >= "01" && d <= "31") {
        return Err("invalid date components");
    }
    Ok(format!("{y}-{m}-{d}T00:00:00Z"))
}

/// Parse a QQL source string and get the AST.
pub fn parse(src: &str) -> Result<Query, ParseError> {
    Parser::new(src).parse()
}

/* ================= Dependency ordering (relaxed) ================= */

fn is_numeric_literal(s: &str) -> bool {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if s.is_empty() {
        return false;
    }
    if s.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return false;
    }
    s.parse::<f64>().is_ok()
}

fn calc_outputs(c: &Calc) -> Vec<String> {
    match c.operation {
        Keyword::Volatility | Keyword::DoubleVolatility => vec![
            c.alias.clone(),
            format!("{}_pos", c.alias),
            format!("{}_neg", c.alias),
        ],
        _ => vec![c.alias.clone()],
    }
}

/// Waves: runnable now → next; stall ⇒ push blocked last (no error).
pub fn order_calcs_by_waves(action: &ActionSection) -> Result<Vec<Vec<Calc>>, ParseError> {
    let mut available: HashSet<String> = action.fields.iter().cloned().collect();
    let mut remaining = action.calc.clone().unwrap_or_default();

    // duplicate alias detection
    {
        let mut seen = HashSet::new();
        for c in &remaining {
            if !seen.insert(&c.alias) {
                return Err(ParseError::new(
                    format!("duplicate CALC alias: '{}'", c.alias),
                    0,
                    0,
                ));
            }
        }
    }

    let mut waves: Vec<Vec<Calc>> = Vec::new();
    while !remaining.is_empty() {
        let (runnable, blocked): (Vec<Calc>, Vec<Calc>) = remaining.into_iter().partition(|c| {
            c.inputs
                .iter()
                .all(|inp| available.contains(inp) || is_numeric_literal(inp))
        });

        if runnable.is_empty() {
            // unknowns last
            if !blocked.is_empty() {
                waves.push(blocked);
            }
            break;
        }

        for c in &runnable {
            for out in calc_outputs(c) {
                available.insert(out);
            }
        }

        waves.push(runnable);
        remaining = blocked;
    }
    Ok(waves)
}

pub fn order_calcs_flat(action: &ActionSection) -> Result<Vec<Calc>, ParseError> {
    Ok(order_calcs_by_waves(action)?
        .into_iter()
        .flatten()
        .collect())
}

/* ======================= UNIT TESTS (abridged) ======================= */

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn test_query_build_provider_queries_pairs() {
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
        let map: HashMap<String, String> = pairs.into_iter().collect();

        assert_eq!(
            map.get("sec_aapl").unwrap(),
            "provider sec_edgar search ticker=AAPL date=2025-01-04T00:00:00Z..2025-10-05T00:00:00Z"
        );
        assert_eq!(
            map.get("yf_nvda").unwrap(),
            "provider yahoo_finance search ticker=NVDA date=2025-09-05T00:00:00Z..2025-10-05T00:00:00Z"
        );
    }
}
