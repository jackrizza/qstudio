#  Quant Query Language (.qql)

##  Abstract

In the world of algorithmic trading, accessing and visualizing market data shouldn’t require excessive boilerplate. **QQL** is a lightweight, expressive DSL for traders and developers. It abstracts away the complexity of data access, transformation, and visualization into a clean, declarative format.

---

##  Example Queries

###  Historical Data

```qql
HISTORICAL
  TICKER aapl
  RANGE 2020-01-01 TO 2021-01-01
  PULL open, close
  CALC open, close => DIFFERENCE AS oc_diff
SHOW
```

###  Live Data

```qql
LIVE
  TICKER aapl
  TICK 2m FOR 10d
  PULL open, close
  CALC open, close => DIFFERENCE AS oc_diff
SHOW
```

###  Fundamental Data

```qql
FUNDAMENTAL
  TICKER aapl
  RANGE 2020-01-01 TO 2020-12-31
  PULL eps, outstanding_shares
  CALC eps, outstanding_shares => MULTIPLY AS revenue
SHOW
```

---

##  Core Concepts

Each QQL script consists of:

- **Model** – Defines the source and scope of data.
- **Action** – Describes transformations and output.

---

##  Model Section

Defines **what** data to pull. Syntax varies by source type:

### Source Types

```qql
HISTORICAL         -- Fetches past market data
LIVE               -- Streams real-time data
FUNDAMENTAL        -- Gets financial filings (e.g. EPS, revenue)
```

### Required Parameters

- `TICKER <symbol>` – Defines the target asset
- `RANGE <start> TO <end>` – For HISTORICAL or FUNDAMENTAL
- `TICK <interval> FOR <duration>` – For LIVE

---

##  Action Section

Describes what to do with the data:

###  PULL

```qql
PULL open, close, volume
```

###  CALC

```qql
CALC open, close => DIFFERENCE AS oc_diff
CALC high => SMA AS high_sma
```

Supported operations:

- `DIFFERENCE`
- `SUM`
- `MULTIPLY`
- `DIVIDE`
- `SMA` (simple moving average)

###  SHOW

```qql
SHOW
```

---

##  GRAPH Section

```qql
GRAPH
  XAXIS aapl

  CANDLE open, high, low, close FOR aapl
  CANDLE open, high, low, close FOR nvda

  LINE oc_diff FOR aapl
  LINE hl_diff FOR nvda
```

---

##  TRADE Section

```qql
TRADE
  STOCK
  ENTRY aapl.low, aapl.l_sma, threshold=0.05
  EXIT  aapl.high, aapl.h_sma, threshold=0.05
  LIMIT 0.1
  HOLD  14
```

---

##  Grammar Specification (EBNF)

```ebnf
query        ::= section+
section      ::= frame | graph_block | trade_block

frame        ::= "FRAME" symbol model_block
model_block  ::= model_type "TICKER" symbol (range | tick_range) pull calc* show?
model_type   ::= "HISTORICAL" | "LIVE" | "FUNDAMENTAL"
range        ::= "RANGE" date "TO" date
tick_range   ::= "TICK" interval "FOR" duration
pull         ::= "PULL" field_list
calc         ::= "CALC" field_list "=>" operation "AS" field
show         ::= "SHOW"

graph_block  ::= "GRAPH" "XAXIS" symbol graph_command+
graph_command ::= ("LINE" symbol ":" field) | ("CANDLE" symbol ":" field_list)

trade_block  ::= "TRADE" "STOCK" entry exit limit hold
entry        ::= "ENTRY" field "," field "," "threshold=" float
exit         ::= "EXIT"  field "," field "," "threshold=" float
limit        ::= "LIMIT" float
hold         ::= "HOLD" int

field_list   ::= field ("," field)*
field        ::= /[a-zA-Z_][a-zA-Z0-9_]*/
symbol       ::= /[a-zA-Z0-9\._]+/
date         ::= /\d{4}-\d{2}-\d{2}/
interval     ::= /\d+[smhd]/
duration     ::= /\d+[smhd]/
operation    ::= "DIFFERENCE" | "SUM" | "MULTIPLY" | "DIVIDE" | "SMA"
```

---

## Server Pipeline

1. **Lexical Analysis** – Tokenizes raw QQL source.
2. **Parsing** – Converts tokens into an AST.
3. **Semantic Analysis** – Validates grammar and logical consistency.
4. **Model Resolution** – Downloads or streams the data.
5. **Action Execution** – Applies `PULL`, `CALC`, etc.
6. **Rendering** – Sends result for `SHOW`, `GRAPH`, or strategy evaluation.

---

## Future Language Extensions

### Loops

```qql
FOR $ticker IN [aapl, msft, goog]
  FRAME $ticker
    HISTORICAL
    TICKER $ticker
    RANGE 2020-01-01 TO 2022-01-01
    PULL close
    CALC close => SMA AS ma
SHOW
```

### Macros

```qql
MACRO basic_metrics
  PULL open, close, volume
  CALC open, close => DIFFERENCE AS change
  SHOW

USE basic_metrics
```

### Functions

```qql
DEF MA5 AS MOVING_AVERAGE(close, 5)
USE MA5
```