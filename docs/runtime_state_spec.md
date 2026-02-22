# Runtime State Specification for scnr2

## 0. Background / Motivation

### Problem Statement

Some lexical constructs require **matching delimiters** where the closing delimiter must correspond to the opening delimiter. Examples:

1. **Rust raw strings**: `r##"..."##` - The number of `#` in the closer must equal the opener
2. **Heredocs**: `<<EOF...EOF` - The closing marker must match the opening marker exactly
3. **Asymmetric delimiters**: `<<<...>>>` - Count of `<` must match count of `>`

These constructs cannot be expressed with static regex patterns alone because the relationship between opener and closer is **dynamic** - determined at runtime based on what was captured.

### The Partial Close Problem

Consider the input: `r##"hello "# world"##`

- Opener: `r##"` (2 hashes)
- Content: `hello "# world` (includes a quote followed by 1 hash)
- Closer: `"##` (2 hashes)

The `"#` in the middle is **not** a closer - it's content. A static pattern cannot distinguish this; we need runtime state to remember that n=2 and validate that the closer has exactly 2 hashes.

### Solution: Runtime State

`scnr2` provides **runtime state** that allows:
- **Capture**: Store a value (count or string) when matching the opener
- **Validate**: Check captured content against stored value when matching the closer

## 1. Goals

- Define the runtime state handling model (capture/validate) for `scnr2`.
- Keep the scanner DSL simple and intuitive.
- Specify the runtime mechanism to **capture** and **validate** dynamic state values during scanning.

## 2. Non-Goals

- No support for multiple `capture()` or `validate()` segments in a single pattern.
- No guarantee of zero-cost for runtime validation (regex re-match is required).

## 3. Terminology

- **State**: Named value tracked during scanning. Two types: `count` (usize) and `str` (String).
- **StateOp**: Runtime action attached to an accept state: capture or validate.
- **Runtime Pattern**: A pattern that includes a dynamic state segment and produces a `StateOp`.
- **Constraint**: Validation rule derived from `validate(...)` (e.g., equals, range, less-than).

## 4. DSL (User-Facing Syntax)

### State Declarations

```
state n: count(0..=16);
state marker: str(r"[A-Z]{1,6}");
```

Notes:
- `count` ranges must be literal integer ranges (no variables).
- `count(min..max)` is **exclusive** of `max`; `count(min..=max)` is **inclusive**.
- `str` patterns are regex literals defining valid capture patterns.
- **Count patterns are literal strings** (regex metacharacters are not special); they are escaped before being inserted into generated regexes.
- Regex literals used in **`str` state declarations** must not contain capturing groups (no `(...)`). Use non-capturing groups `(?:...)` if grouping is needed.

### Pattern Segments

```
capture("#", n)          // Capture count of '#' into state n
validate("#", n)         // Validate: captured count == n
validate("#", n, 0..n)   // Validate: captured count in range [0, n)
capture(marker)          // Capture string into state marker
validate(marker)         // Validate: captured string == marker
```

Rules:
- At most one `capture()` or `validate()` per pattern.
- `capture()` and `validate()` cannot both appear in the same pattern.

## 5. Complete Usage Example

### Rust Raw String Scanner

```rust
scanner! {
    RawStringScanner {
        state n: count(0..=16);

        mode INITIAL {
            // Capture opener: r followed by 0-16 hashes and a quote
            token r"r" + capture("#", n) + r#"""# => 1;
            token r"." => 99;  // Other characters

            on 1 enter RAW;
        }

        mode RAW {
            // Content: non-quote characters
            token r#"[^"]*"# => 10;

            // Partial close: quote followed by fewer hashes than n (this is content)
            token r#"""# + validate("#", n, 0..n) => 10;

            // Exact close: quote followed by exactly n hashes
            token r#"""# + validate("#", n) => 20;

            on 20 enter INITIAL;
        }
    }
}
```

### Example Input/Output

**Input:** `r##"hello "# world"##`

**Token sequence:**

| Match | Token | Description |
|-------|-------|-------------|
| `r##"` | 1 | Opener, captures n=2 |
| `hello ` | 10 | Content |
| `"#` | 10 | Partial close (1 < 2), treated as content |
| ` world` | 10 | Content |
| `"##` | 20 | Exact close (2 == 2) |

**Input:** `r"simple"`

| Match | Token | Description |
|-------|-------|-------------|
| `r"` | 1 | Opener, captures n=0 |
| `simple` | 10 | Content |
| `"` | 20 | Exact close (0 == 0) |

## 6. Runtime Data Model

### State Storage

Per scanner instance, shared across all modes:

```rust
enum StateValue {
    Count(usize),
    Str(String),
}

// Stored in ScannerImpl
state_storage: HashMap<&'static str, StateValue>
```

Lifecycle:
- **Capture** overwrites any existing value for that state name.
- **Validate** reads the value; missing state causes validation failure.
- State **persists** across mode transitions and tokens until overwritten.

### AcceptData Extension

```rust
pub struct AcceptData {
    pub token_type: usize,
    pub priority: usize,
    pub lookahead: Lookahead,
    pub state_op: Option<StateOp>,
}
```

### StateOp Enum

```rust
pub enum StateOp {
    CaptureCount {
        state_name: &'static str,
        capture_pattern: &'static str, // literal unit to count (e.g. "#", "##")
        group: usize,  // 1-based capture group index
    },
    ValidateCount {
        state_name: &'static str,
        capture_pattern: &'static str,
        group: usize,
        constraint: ValidationConstraint,
    },
    CaptureStr {
        state_name: &'static str,
        group: usize,
    },
    ValidateStr {
        state_name: &'static str,
        group: usize,
    },
}
```

### ValidationConstraint

```rust
pub enum ValidationConstraint {
    /// captured_len == n
    Equals,

    /// captured_len < n
    LessThan,

    /// min <= captured_len < max_exclusive
    /// where min/max can be expressions involving n
    Range {
        min: ConstraintExpr,
        max_exclusive: ConstraintExpr,
    },
}

pub enum ConstraintExpr {
    Lit(usize),
    N,
    Add(Box<ConstraintExpr>, Box<ConstraintExpr>),
    Sub(Box<ConstraintExpr>, Box<ConstraintExpr>),
}
```

## 7. Generated Regex for Runtime Patterns

For patterns with `capture()` or `validate()`, the generator produces a regex with capture groups.

### Example: `r"r" + capture("#", n) + r#"""#`

With state declaration `state n: count(0..=16)`:

```
Generated regex: r(#{0,16})"
                  ^^^^^^^^^
                  Capture group 1
```

### Example: `r#"""# + validate("#", n, 0..n)`

```
Generated regex: "(#{0,16})
                   ^^^^^^^
                   Capture group 1
```

The generated regex:
- Uses the state's declared range for repetition bounds
- Escapes count patterns as **literal** units before inserting them
- Wraps the dynamic segment in a capture group
- Is anchored (`\A...\z`) when used for re-matching

## 8. Runtime Matching Algorithm

### 8.1 Normal DFA Matching

Find the best accept by the following total ordering:
1. Longest match (by length)
2. Lower priority number (higher priority)
3. More specific constraint (Equals > Range > LessThan)

Lookahead is **zero-width** and does not advance the match end.

### 8.2 StateOp Evaluation

When a DFA state accepts:

1. If `state_op` is `None` → accept as normal.
2. If `state_op` is `Some`:
   a. Extract matched substring from input.
   b. Re-run the stored regex to extract capture group.
   c. Apply capture or validate operation.
   d. If validation **fails** → **reject this match entirely**.

### 8.3 Validation Failure Behavior

When StateOp validation fails, that accept is rejected and the scanner continues ranking other accepts at the **same input position**, just as if the failed accept had never matched.

### 8.4 Capture Group Extraction

The DFA does not track capture group boundaries, so runtime extraction requires regex re-matching:

1. Store the capture-capable regex pattern in `AcceptData` (or lazily compile).
2. On accept, re-run this regex against the matched substring.
3. Extract the specified capture group.
4. If regex fails or group is missing → validation fails.

Implementation options:
- Store `&'static regex::Regex` via `once_cell::sync::Lazy`
- Store pattern string and compile lazily on first use

## 9. Count State Semantics

For `count` states:

**Capture:**
```rust
state_value = count_occurrences(captured_substring, capture_pattern)
```

**Validate:**
- `Equals`: `captured_len == state_value`
- `LessThan`: `captured_len < state_value`
- `Range { min, max }`: `eval(min, n) <= captured_len < eval(max, n)`

Note:
- Count patterns are **literal** strings, not regexes.
- If the pattern is a single character, this is a character count (not bytes).
- If the pattern has multiple characters, count **non-overlapping** occurrences of the literal pattern.

## 10. String State Semantics

For `str` states:

**Capture:** Store captured substring as-is.

**Validate:** Captured substring must be exactly equal to stored value.

## 11. Specificity Ordering

When multiple accepts have equal length and priority, prefer more specific constraints:

1. `Equals` (most specific)
2. `Range`
3. `LessThan` (least specific)

This ensures exact closers are preferred over partial closers.

## 12. Compile-Time Validation

All invalid syntax must be caught at compile time with clear error messages:

| Error Condition | Error Message |
|-----------------|---------------|
| Undeclared state reference | `state 'x' is not declared` |
| State type mismatch (count vs str) | `state 'n' is declared as count, but used as str` |
| Invalid range in state declaration | `state range must use literal integers only` |
| Multiple capture/validate in pattern | `only one capture() or validate() allowed per pattern` |
| Both capture and validate in pattern | `capture() and validate() cannot appear in the same pattern` |
| Invalid constraint expression | `constraint expression must be 'n', a literal, or simple arithmetic` |
| Open-ended range in constraint | `open-ended ranges (n..) are not supported` |
| Capturing groups in `str` state regex literal | `capturing groups are not allowed; use (?:...)` |

## 13. Design Decisions

**Q: What happens when StateOp validation fails?**
A: The match is **rejected**, and the scanner continues ranking other accepts at the **same input position** (as if the failed accept never existed).

**Q: Should StateOp validation run before or after lookahead evaluation?**
A: After lookahead, before final accept. Order: DFA match → lookahead → StateOp → accept.

**Q: What is the lifecycle of state values?**
A: State **persists** across mode transitions and tokens until explicitly overwritten by another capture.

**Q: Multi-char patterns in count - Should `capture("##", n)` count occurrences or total characters?**
A: Count **occurrences** of the pattern. Example: `"####"` with `capture("##", n)` → n=2 (two occurrences), not n=4 (four characters).

**Q: Multiple accepts with same regex - How should they be ordered?**
A: Use the total ordering: length → priority → specificity (Equals > Range > LessThan).

**Q: State scoping - Should states be clearable?**
A: No. States are **global** and persist until overwritten. No `clear(state)` syntax.
