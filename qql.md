## Quant Query Language (.qql)

### Abstract

In diving into the world of algorithmic trading, I found that many projects required excessive boilerplate code just to view a simple chart. There had to be a more streamlined and intuitive way to interact with financial data. That’s where **QQL** comes in—a lightweight, expressive domain-specific language designed to simplify data querying and analysis for traders and developers alike.

------

### Query Examples

#### Historical Data Query

This example demonstrates how to pull historical data and display it as a table:

```
HISTORICAL
TICKER aapl
FROM 20200101 TO 20210101
PULL open, close
CALC open, close DIFFERENCE CALLED oc_diff
SHOW
```

#### Live Data Query

This example shows how to retrieve live market data in intervals:

```
LIVE
TICKER aapl
TICK 2m FOR 10d
PULL open, close
CALC open, close DIFFERENCE CALLED oc_diff
SHOW
```

#### Fundamental Data Query

This example pulls company fundamentals from SEC filings:

```
FUNDAMENTAL
TICKER aapl
FROM 20200101 TO 20200101
PULL eps, outstanding_shares
CALC eps, outstanding_shares MULTIPLY CALLED revenue
SHOW
```

------

### Concepts

A QQL query is composed of two primary components:

- **Model** – defines what data is being pulled
- **Action** – defines what is done with the data

------

#### Model

The **Model** section specifies the data source and context. This begins by declaring the source type using one of the following:

- `LIVE` – Pulls real-time data via a live market data API (TBD) and opens a socket for streaming results to the client.
- `HISTORICAL` – Fetches past market data (e.g., from Yahoo Finance) and returns the dataset to the client.
- `FUNDAMENTAL` – Retrieves financial statements and fundamentals (e.g., from SEC filings) for a specific ticker.

The model also requires defining the target asset using:

- `TICKER` – Specifies the asset symbol (e.g., AAPL)

Depending on the data source, timeframes are declared as:

- `FROM ... TO ...` – For `HISTORICAL` and `FUNDAMENTAL` data
- `TICK ... FOR ...` – For `LIVE` data (e.g., every 2 minutes for 10 days)

------

#### Action

Once the model is defined, QQL supports a concise set of **Actions** to manipulate and visualize the data:

- `PULL` – Specifies which fields to extract from the data source (e.g., `open`, `close`, `eps`).

- `CALC` – Defines a calculation or transformation between two or more fields. The format is:

```
CALC <field1>, <field2>, ... <OPERATION> CALLED <new_field_name>
```

  Supported operations: `DIFFERENCE`, `SUM`, `MULTIPLY`, `DIVIDE`

- `SHOW` – Outputs the processed dataset in a tabular format (console or frontend viewer).

------

### Syntax Specification

QQL has a line-based syntax. Each line is a command, optionally followed by arguments. The structure is deterministic and easy to parse.

**Grammar (EBNF Style):**

```
query        ::= model_section action_section
model_section ::= model_type ticker time_spec
model_type   ::= "LIVE" | "HISTORICAL" | "FUNDAMENTAL"
ticker       ::= "TICKER" symbol
symbol       ::= /[a-zA-Z0-9\.]+/
time_spec    ::= live_spec | date_range
live_spec    ::= "TICK" interval "FOR" duration
interval     ::= /[0-9]+[smhd]/
duration     ::= /[0-9]+[smhd]/
date_range   ::= "FROM" date "TO" date
date         ::= /[0-9]{8}/

action_section ::= pull calc? show
pull         ::= "PULL" field_list
field_list   ::= field ("," field)*
field        ::= /[a-zA-Z_]+/
calc         ::= "CALC" field_list operation "CALLED" field
operation    ::= "DIFFERENCE" | "SUM" | "MULTIPLY" | "DIVIDE"
show         ::= "SHOW"
```

------

### Server-Side Execution

When a QQL query is submitted, the server processes it through the following pipeline:

1. **Lexical Analysis**
    The raw query is broken into tokens using a lexer. Each keyword, symbol, or value is identified and categorized.

2. **Parsing**
    The token stream is passed to a parser that validates its structure against the defined grammar. If it conforms, it generates an abstract syntax tree (AST).

3. **Syntax Analysis** (via Analyzer)
    The analyzer inspects the AST to detect syntax and semantic errors. Examples include:

   - Missing required clauses (e.g., `TICKER` without a symbol)
   - Unsupported operations (e.g., invalid `CALC` op)
   - Field mismatch or duplicate aliases

   If any issue is found, a detailed error message is returned to the client with the line number and nature of the error.

4. **Model Resolution**
    The server loads the data source as indicated by `LIVE`, `HISTORICAL`, or `FUNDAMENTAL`, fetching and caching the requested dataset.

5. **Execution of Actions**
    The server performs the specified `PULL`, then applies transformations defined in `CALC`. Results are organized and returned for `SHOW`.

6. **Output Formatting**
    The result set is formatted into a standardized table or streaming structure suitable for frontend rendering or CLI output.

This modular pipeline ensures each query is efficiently processed and errors are caught early before data is fetched or transformed.

------

### Planned Language Extensions

To expand the power of QQL, future versions may support the following features:

#### Conditionals

Allow conditional filtering based on column values:

```
FILTER close > 150
```

Could be used before `SHOW` to limit rows displayed.

#### Graphing
Allows data export for graphs
```
GRAPH
  LINE open
  CANDLE 0pen high, low, close
  ...
```

#### Functions

Allow reusable computations:

```
DEF MA5 AS MOVING_AVERAGE(close, 5)
USE MA5
```

Support for built-in and user-defined functions improves composability and abstraction.

#### Loops

Though rare in DSLs, loops might help iterate over tickers or time ranges:

```
FOR $ticker IN [aapl, msft, goog]
  HISTORICAL
  TICKER $ticker
  ...
```

#### Macros

Reusable blocks of QQL code:

```
MACRO basic_metrics
  PULL open, close, volume
  CALC open, close DIFFERENCE CALLED change
  SHOW

USE basic_metrics
```

#### Trading

Only avaliable with a Historical or Live function

```
TRADE
  OPTION CALL
  ENTRY low, low_sma, 0.05 
  EXIT high, high_sma, 0.05
  LIMIT 0.1
  HOLD 14
```