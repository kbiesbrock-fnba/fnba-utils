// Hand-rolled tokenizer + Pratt/recursive-descent parser + evaluator.
// No eval / Function() — inputs are never executed as code.
//
// Grammar (informally):
//   expr    = additive
//   additive  = multiplicative (('+' | '-') multiplicative)*
//   multiplicative = unary (('*' | '/' | '%') unary | implicit-mul)*
//   unary   = '-' unary | power
//   power   = postfix ('^' unary)?          ← right-associative
//   postfix = primary '!'*
//   primary = NUMBER | IDENT | IDENT '(' args ')' | '(' expr ')'
//   args    = expr (',' expr)*
//
// Implicit multiplication fires between:
//   NUMBER IDENT   — 2pi, 3sin(1)
//   NUMBER '('     — 2(3+4)
//   ')' IDENT      — impossible by grammar, but handled
//   ')' '('        — (2)(3)

export type TrigUnit = "rad" | "deg" | "grad";

// ─── Tokenizer ───────────────────────────────────────────────────────────────

type TokKind =
  | "NUM"
  | "IDENT"
  | "PLUS"
  | "MINUS"
  | "STAR"
  | "SLASH"
  | "PERCENT"
  | "CARET"
  | "BANG"
  | "LPAREN"
  | "RPAREN"
  | "COMMA"
  | "EOF";

interface Token {
  kind: TokKind;
  value: string; // raw text (meaningful for NUM and IDENT)
}

function tokenize(src: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < src.length) {
    const ch = src[i];

    // Skip whitespace
    if (ch === " " || ch === "\t" || ch === "\n" || ch === "\r") {
      i++;
      continue;
    }

    if (ch >= "0" && ch <= "9") {
      // Number literal, including decimals and scientific notation.
      // Rule: consume e/E as exponent ONLY when followed by digits or ± then digits.
      // "2e3" → 2000 but "2e" → NUM(2), IDENT(e).
      let num = "";
      while (i < src.length && src[i] >= "0" && src[i] <= "9") {
        num += src[i++];
      }
      if (i < src.length && src[i] === ".") {
        num += src[i++];
        while (i < src.length && src[i] >= "0" && src[i] <= "9") {
          num += src[i++];
        }
      }
      // Scientific notation: consume only if digit follows (optionally with sign)
      if (i < src.length && (src[i] === "e" || src[i] === "E")) {
        const ePos = i;
        let peek = i + 1;
        if (peek < src.length && (src[peek] === "+" || src[peek] === "-")) peek++;
        if (peek < src.length && src[peek] >= "0" && src[peek] <= "9") {
          // Valid scientific notation — consume it
          num += src[i++]; // e/E
          if (src[i] === "+" || src[i] === "-") num += src[i++];
          while (i < src.length && src[i] >= "0" && src[i] <= "9") {
            num += src[i++];
          }
        }
        // else: leave i at ePos so "e" is tokenised as an IDENT next iteration
        void ePos;
      }
      tokens.push({ kind: "NUM", value: num });
      continue;
    }

    if (ch === "π" || (ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z") || ch === "_") {
      let id = src[i++];
      while (
        i < src.length &&
        (src[i] >= "a" && src[i] <= "z" ||
          src[i] >= "A" && src[i] <= "Z" ||
          src[i] >= "0" && src[i] <= "9" ||
          src[i] === "_")
      ) {
        id += src[i++];
      }
      tokens.push({ kind: "IDENT", value: id });
      continue;
    }

    switch (ch) {
      case "+": tokens.push({ kind: "PLUS",    value: "+" }); i++; break;
      case "-": tokens.push({ kind: "MINUS",   value: "-" }); i++; break;
      case "*": tokens.push({ kind: "STAR",    value: "*" }); i++; break;
      case "/": tokens.push({ kind: "SLASH",   value: "/" }); i++; break;
      case "%": tokens.push({ kind: "PERCENT", value: "%" }); i++; break;
      case "^": tokens.push({ kind: "CARET",   value: "^" }); i++; break;
      case "!": tokens.push({ kind: "BANG",    value: "!" }); i++; break;
      case "(": tokens.push({ kind: "LPAREN",  value: "(" }); i++; break;
      case ")": tokens.push({ kind: "RPAREN",  value: ")" }); i++; break;
      case ",": tokens.push({ kind: "COMMA",   value: "," }); i++; break;
      default:
        // Unknown character — mark as invalid by emitting nothing useful.
        // The parser will fail at EOF or unexpected token.
        throw new ParseError(`Unexpected character: ${ch}`);
    }
  }

  tokens.push({ kind: "EOF", value: "" });
  return tokens;
}

// ─── Parser ──────────────────────────────────────────────────────────────────

class ParseError extends Error {}

// Represents a parsed expression node (as a lazy evaluator closure for simplicity)
type Expr = (ctx: EvalCtx) => number;

interface EvalCtx {
  unit: TrigUnit;
}

class Parser {
  private pos = 0;

  constructor(private readonly tokens: Token[]) {}

  private peek(): Token {
    return this.tokens[this.pos];
  }

  private consume(): Token {
    return this.tokens[this.pos++];
  }

  private expect(kind: TokKind): Token {
    const t = this.consume();
    if (t.kind !== kind) throw new ParseError(`Expected ${kind}, got ${t.kind}`);
    return t;
  }

  parse(): Expr {
    const e = this.parseAdditive();
    if (this.peek().kind !== "EOF") {
      throw new ParseError(`Unexpected token: ${this.peek().value}`);
    }
    return e;
  }

  private parseAdditive(): Expr {
    let left = this.parseMultiplicative();
    while (this.peek().kind === "PLUS" || this.peek().kind === "MINUS") {
      const op = this.consume().kind;
      const right = this.parseMultiplicative();
      if (op === "PLUS") {
        const l = left, r = right;
        left = (ctx) => l(ctx) + r(ctx);
      } else {
        const l = left, r = right;
        left = (ctx) => l(ctx) - r(ctx);
      }
    }
    return left;
  }

  private parseMultiplicative(): Expr {
    let left = this.parseUnary();
    for (;;) {
      const k = this.peek().kind;
      if (k === "STAR" || k === "SLASH" || k === "PERCENT") {
        const op = this.consume().kind;
        const right = this.parseUnary();
        if (op === "STAR") {
          const l = left, r = right;
          left = (ctx) => l(ctx) * r(ctx);
        } else if (op === "SLASH") {
          const l = left, r = right;
          left = (ctx) => l(ctx) / r(ctx);
        } else {
          const l = left, r = right;
          left = (ctx) => l(ctx) % r(ctx);
        }
      } else if (this.isImplicitMulStart(left)) {
        // Implicit multiplication: consume next primary without an operator
        const right = this.parseUnary();
        const l = left, r = right;
        left = (ctx) => l(ctx) * r(ctx);
      } else {
        break;
      }
    }
    return left;
  }

  // Implicit multiplication applies when the *next* token starts a new primary
  // and the preceding factor was a NUM/RPAREN (but not after operators or LPAREN).
  // We determine this by inspecting the next token, not the left expr itself.
  private isImplicitMulStart(_left: Expr): boolean {
    const k = this.peek().kind;
    return k === "IDENT" || k === "LPAREN";
  }

  private parseUnary(): Expr {
    if (this.peek().kind === "MINUS") {
      this.consume();
      const operand = this.parseUnary();
      return (ctx) => -operand(ctx);
    }
    // Unary plus (tolerated, e.g. "+3")
    if (this.peek().kind === "PLUS") {
      this.consume();
      return this.parseUnary();
    }
    return this.parsePower();
  }

  private parsePower(): Expr {
    const base = this.parsePostfix();
    if (this.peek().kind === "CARET") {
      this.consume();
      // Right-associative: recurse into parseUnary so "-3^2" = -(3^2) = -9
      // (the unary minus binds LESS tightly than ^, Mages-compatible)
      const exp = this.parseUnary();
      return (ctx) => Math.pow(base(ctx), exp(ctx));
    }
    return base;
  }

  private parsePostfix(): Expr {
    let e = this.parsePrimary();
    while (this.peek().kind === "BANG") {
      this.consume();
      const inner = e;
      e = (ctx) => {
        const n = inner(ctx);
        return factorial(n);
      };
    }
    return e;
  }

  private parsePrimary(): Expr {
    const t = this.peek();

    if (t.kind === "NUM") {
      this.consume();
      const v = Number(t.value);
      return () => v;
    }

    if (t.kind === "IDENT") {
      this.consume();
      const name = t.value.toLowerCase();

      // Look-ahead: is this a function call?
      if (this.peek().kind === "LPAREN") {
        this.consume(); // eat '('
        const args: Expr[] = [];
        if (this.peek().kind !== "RPAREN") {
          args.push(this.parseAdditive());
          while (this.peek().kind === "COMMA") {
            this.consume();
            args.push(this.parseAdditive());
          }
        }
        this.expect("RPAREN");
        return buildFunctionCall(name, args);
      }

      // Constant or bare name
      return buildConstantOrIdent(name);
    }

    if (t.kind === "LPAREN") {
      this.consume();
      const inner = this.parseAdditive();
      this.expect("RPAREN");
      return inner;
    }

    throw new ParseError(`Unexpected token: ${t.kind} ("${t.value}")`);
  }
}

// ─── Trig helpers ─────────────────────────────────────────────────────────────

function toRad(v: number, unit: TrigUnit): number {
  if (unit === "deg") return (v * Math.PI) / 180;
  if (unit === "grad") return (v * Math.PI) / 200;
  return v;
}

function fromRad(v: number, unit: TrigUnit): number {
  if (unit === "deg") return (v * 180) / Math.PI;
  if (unit === "grad") return (v * 200) / Math.PI;
  return v;
}

// ─── Constants & functions ────────────────────────────────────────────────────

const CONSTANTS: Record<string, number> = {
  pi: Math.PI,
  π: Math.PI,
  e: Math.E,
};

function buildConstantOrIdent(name: string): Expr {
  if (name in CONSTANTS) {
    const v = CONSTANTS[name];
    return () => v;
  }
  throw new ParseError(`Unknown identifier: ${name}`);
}

// Factorial: integers 0–170 only (171! overflows to Infinity).
function factorial(n: number): number {
  if (!Number.isInteger(n) || n < 0 || n > 170) return NaN;
  let r = 1;
  for (let i = 2; i <= n; i++) r *= i;
  return r;
}

// The trig functions that convert input unit → rad and vice versa for inverse.
const TRIG_INPUT  = new Set(["sin", "cos", "tan", "sinh", "cosh", "tanh"]);
const TRIG_OUTPUT = new Set(["asin", "acos", "atan", "asinh", "acosh", "atanh"]);

function buildFunctionCall(name: string, args: Expr[]): Expr {
  const arity1 = (fn: (x: number) => number): Expr => {
    if (args.length !== 1) throw new ParseError(`${name}() expects 1 argument`);
    const a = args[0];
    return (ctx) => fn(a(ctx));
  };

  const trigIn = (fn: (x: number) => number): Expr => {
    if (args.length !== 1) throw new ParseError(`${name}() expects 1 argument`);
    const a = args[0];
    return (ctx) => fn(toRad(a(ctx), ctx.unit));
  };

  const trigOut = (fn: (x: number) => number): Expr => {
    if (args.length !== 1) throw new ParseError(`${name}() expects 1 argument`);
    const a = args[0];
    return (ctx) => fromRad(fn(a(ctx)), ctx.unit);
  };

  switch (name) {
    // Trig (input converted)
    case "sin":   return trigIn(Math.sin);
    case "cos":   return trigIn(Math.cos);
    case "tan":   return trigIn(Math.tan);
    case "sinh":  return arity1(Math.sinh);
    case "cosh":  return arity1(Math.cosh);
    case "tanh":  return arity1(Math.tanh);
    // Inverse trig (output converted)
    case "asin":  return trigOut(Math.asin);
    case "acos":  return trigOut(Math.acos);
    case "atan":  return trigOut(Math.atan);
    case "asinh": return arity1(Math.asinh);
    case "acosh": return arity1(Math.acosh);
    case "atanh": return arity1(Math.atanh);
    // Logarithms
    case "log":   return arity1((x) => Math.log10(x));
    case "ln":    return arity1(Math.log);
    case "log2":  return arity1(Math.log2);
    case "exp":   return arity1(Math.exp);
    // Other math
    case "sqrt":  return arity1(Math.sqrt);
    case "abs":   return arity1(Math.abs);
    case "ceil":  return arity1(Math.ceil);
    case "floor": return arity1(Math.floor);
    case "round": return arity1(Math.round);
    case "sign":  return arity1(Math.sign);
    case "factorial": {
      if (args.length !== 1) throw new ParseError("factorial() expects 1 argument");
      const a = args[0];
      return (ctx) => factorial(a(ctx));
    }
    case "pow": {
      if (args.length !== 2) throw new ParseError("pow() expects 2 arguments");
      const [a, b] = args;
      return (ctx) => Math.pow(a(ctx), b(ctx));
    }
    case "min": {
      if (args.length === 0) throw new ParseError("min() expects at least 1 argument");
      return (ctx) => Math.min(...args.map((a) => a(ctx)));
    }
    case "max": {
      if (args.length === 0) throw new ParseError("max() expects at least 1 argument");
      return (ctx) => Math.max(...args.map((a) => a(ctx)));
    }
    case "rand": {
      if (args.length !== 0) throw new ParseError("rand() expects no arguments");
      return () => Math.random();
    }
    case "randi": {
      if (args.length !== 1) throw new ParseError("randi() expects 1 argument");
      const a = args[0];
      return (ctx) => Math.floor(Math.random() * (Math.floor(a(ctx)) + 1));
    }
    default:
      // Check if it looks like a trig name that was missed (defensive)
      if (TRIG_INPUT.has(name) || TRIG_OUTPUT.has(name)) {
        throw new ParseError(`Trig function ${name} not wired`);
      }
      throw new ParseError(`Unknown function: ${name}`);
  }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Evaluate an arithmetic/scientific expression and return the numeric result,
 * or `null` for any parse error, eval error, or non-finite result.
 */
export function evaluate(expr: string, unit: TrigUnit): number | null {
  const input = expr.trim();
  if (!input) return null;
  try {
    const tokens = tokenize(input);
    const parser = new Parser(tokens);
    const fn = parser.parse();
    const result = fn({ unit });
    return Number.isFinite(result) ? result : null;
  } catch {
    return null;
  }
}

/**
 * Format a number the same way CmdPal does: round to 10 decimal places,
 * strip trailing zeros. For very large values (|v| >= 1e15), use String().
 */
export function formatResult(v: number): string {
  if (Math.abs(v) >= 1e15) return String(v);
  return String(parseFloat(v.toFixed(10)));
}

/**
 * Return true if the expression string contains any trig function name.
 * Used to decide whether to append the trig-unit hint in the description.
 */
export function usesTrig(expr: string): boolean {
  return /\b(sin|cos|tan|asin|acos|atan|sinh|cosh|tanh|asinh|acosh|atanh)\s*\(/i.test(expr);
}
