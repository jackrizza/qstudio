// lexer.rs
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Live,
    Historical,
    Fundamental,
    Ticker,
    From,
    To,
    Tick,
    For,
    Pull,
    Calc,
    Called,
    ShowTable,
    Difference,
    Sum,
    Multiply,
    Divide,
    Graph,
    Line,
    Candle,
    Bar,
    Comma
}

impl Keyword {
    pub fn from_str(s: &str) -> Option<Self> {
        use Keyword::*;
        match s {
            "LIVE" => Some(Live),
            "HISTORICAL" => Some(Historical),
            "FUNDAMENTAL" => Some(Fundamental),
            "TICKER" => Some(Ticker),
            "FROM" => Some(From),
            "TO" => Some(To),
            "TICK" => Some(Tick),
            "FOR" => Some(For),
            "PULL" => Some(Pull),
            "CALC" => Some(Calc),
            "CALLED" => Some(Called),
            "SHOWTABLE" => Some(ShowTable),
            "GRAPH" => Some(Graph),
            "DIFFERENCE" => Some(Difference),
            "SUM" => Some(Sum),
            "MULTIPLY" => Some(Multiply),
            "DIVIDE" => Some(Divide),
            "LINE" => Some(Line),
            "CANDLE" => Some(Candle),
            "BAR" => Some(Bar),
            _ => None,
        }
    }

    
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Keyword(Keyword),
    Identifier(String),
    Date(String),
    Interval(String),
    Literal(String),
    Comma,
    Newline,
    EOF,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    current_line: usize,
    current_col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Lexer {
            input: source.chars().peekable(),
            current_line: 1,
            current_col: 0,
        }
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.next();
        if let Some(c) = ch {
            if c == '\n' {
                self.current_line += 1;
                self.current_col = 0;
            } else {
                self.current_col += 1;
            }
        }
        ch
    }

    fn _peek(&mut self) -> Option<char> {
        self.input.peek().copied()
    }

    fn lex_word_like(&mut self, first: char) -> TokenKind {
        let mut buf = String::new();
        buf.push(first);
        while let Some(&c) = self.input.peek() {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                buf.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if buf.len() == 8 && buf.chars().all(|c| c.is_ascii_digit()) {
            TokenKind::Literal(buf)
        } else if buf.chars().next_back().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
            && buf[..buf.len() - 1].chars().all(|c| c.is_ascii_digit())
            && matches!(buf.chars().last().unwrap(), 's' | 'm' | 'h' | 'd')
        {
            TokenKind::Interval(buf)
        } else if let Some(kw) = Keyword::from_str(&buf.to_uppercase()) {
            TokenKind::Keyword(kw)
        } else {
            TokenKind::Identifier(buf)
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.input.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        let line = self.current_line;
        let column = self.current_col + 1;

        if let Some(c) = self.advance() {
            match c {
                ',' => Ok(Token {
                    kind: TokenKind::Comma,
                    line,
                    column,
                }),
                '\n' => Ok(Token {
                    kind: TokenKind::Newline,
                    line,
                    column,
                }),
                c if c.is_ascii_alphanumeric() => {
                    let kind = self.lex_word_like(c);
                    Ok(Token { kind, line, column })
                }
                _ => Err(LexError {
                    message: format!("Unexpected character '{}'", c),
                    line,
                    column,
                }),
            }
        } else {
            Ok(Token {
                kind: TokenKind::EOF,
                line,
                column,
            })
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_token() {
            Ok(tok) if matches!(tok.kind, TokenKind::EOF) => None,
            res => Some(res),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_lexer() {
        let input = "LIVE HISTORICAL FUNDAMENTAL TICKER aapl";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Keyword(Keyword::Live));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Keyword(Keyword::Historical));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Keyword(Keyword::Fundamental));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Keyword(Keyword::Ticker));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Identifier("aapl".to_string()));
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::EOF);
    }
}