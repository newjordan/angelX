//! Bounded LaTeX → terminal-math rendering.
//!
//! A terminal cannot reproduce TeX's font metrics, but it can preserve the
//! notation instead of showing source punctuation. Inline formulas use Unicode
//! mathematical glyphs and scripts; display formulas additionally compose
//! fractions vertically around a real rule.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MAX_MATH_BYTES: usize = 16 * 1024;
const MAX_PARSE_DEPTH: usize = 32;

#[derive(Clone, Debug)]
enum Node {
    Row(Vec<Node>),
    Text(String),
    Fraction(Box<Node>, Box<Node>),
    Root(Box<Node>),
    Script {
        base: Box<Node>,
        sub: Option<Box<Node>>,
        sup: Option<Box<Node>>,
    },
    Accent {
        body: Box<Node>,
        mark: char,
    },
}

impl Node {
    fn row(nodes: Vec<Node>) -> Self {
        match nodes.len() {
            0 => Self::Text(String::new()),
            1 => nodes.into_iter().next().expect("one node"),
            _ => Self::Row(nodes),
        }
    }
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            at: 0,
        }
    }

    fn parse(mut self) -> Node {
        self.row(None, 0)
    }

    fn row(&mut self, stop: Option<char>, depth: usize) -> Node {
        let mut nodes = Vec::new();
        while let Some(ch) = self.peek() {
            if Some(ch) == stop {
                self.at += 1;
                break;
            }
            match ch {
                '{' => {
                    self.at += 1;
                    nodes.push(self.row(Some('}'), depth.saturating_add(1)));
                }
                '}' => {
                    self.at += 1;
                    if stop.is_some() {
                        break;
                    }
                    nodes.push(Node::Text("}".into()));
                }
                '^' | '_' => {
                    self.at += 1;
                    let script = self.atom(depth.saturating_add(1));
                    attach_script(&mut nodes, ch, script);
                }
                '\\' => {
                    self.at += 1;
                    if let Some(node) = self.command(depth.saturating_add(1)) {
                        nodes.push(node);
                    }
                }
                '&' => {
                    // Alignment markers are layout instructions, not notation.
                    self.at += 1;
                    nodes.push(Node::Text("  ".into()));
                }
                '~' => {
                    self.at += 1;
                    nodes.push(Node::Text(" ".into()));
                }
                c if c.is_whitespace() => {
                    self.at += 1;
                    if !matches!(nodes.last(), Some(Node::Text(text)) if text.ends_with(' ')) {
                        nodes.push(Node::Text(" ".into()));
                    }
                }
                _ => {
                    self.at += 1;
                    nodes.push(Node::Text(ch.to_string()));
                }
            }
            if depth >= MAX_PARSE_DEPTH {
                let tail: String = self.chars[self.at..].iter().collect();
                if !tail.is_empty() {
                    nodes.push(Node::Text(tail));
                }
                self.at = self.chars.len();
            }
        }
        Node::row(nodes)
    }

    fn atom(&mut self, depth: usize) -> Node {
        match self.peek() {
            Some('{') => {
                self.at += 1;
                self.row(Some('}'), depth)
            }
            Some('\\') => {
                self.at += 1;
                self.command(depth)
                    .unwrap_or_else(|| Node::Text(String::new()))
            }
            Some(ch) => {
                self.at += 1;
                Node::Text(ch.to_string())
            }
            None => Node::Text(String::new()),
        }
    }

    fn required_group(&mut self, depth: usize) -> Node {
        self.skip_spaces();
        if self.peek() == Some('{') {
            self.at += 1;
            self.row(Some('}'), depth)
        } else {
            self.atom(depth)
        }
    }

    fn command(&mut self, depth: usize) -> Option<Node> {
        let Some(first) = self.peek() else {
            return Some(Node::Text("\\".into()));
        };
        if !first.is_ascii_alphabetic() {
            self.at += 1;
            return match first {
                ',' | ':' | ';' | ' ' => Some(Node::Text(" ".into())),
                '!' => None,
                '\\' => Some(Node::Text(" ".into())),
                '{' | '}' | '_' | '%' | '$' | '#' | '&' => Some(Node::Text(first.to_string())),
                _ => Some(Node::Text(first.to_string())),
            };
        }

        let start = self.at;
        while self.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            self.at += 1;
        }
        let name: String = self.chars[start..self.at].iter().collect();
        match name.as_str() {
            "frac" | "dfrac" | "tfrac" => {
                let numerator = self.required_group(depth);
                let denominator = self.required_group(depth);
                Some(Node::Fraction(Box::new(numerator), Box::new(denominator)))
            }
            "sqrt" => {
                // Preserve an optional root index compactly: ³√x, or ⁿ√x.
                self.skip_spaces();
                let index = if self.peek() == Some('[') {
                    self.at += 1;
                    let start = self.at;
                    while self.peek().is_some_and(|ch| ch != ']') {
                        self.at += 1;
                    }
                    let raw: String = self.chars[start..self.at].iter().collect();
                    if self.peek() == Some(']') {
                        self.at += 1;
                    }
                    Some(raw)
                } else {
                    None
                };
                let root = Node::Root(Box::new(self.required_group(depth)));
                index.map_or(Some(root.clone()), |raw| {
                    let prefix =
                        to_script(&raw, ScriptKind::Super).unwrap_or_else(|| format!("^({raw})"));
                    Some(Node::Row(vec![Node::Text(prefix), root]))
                })
            }
            "text" | "textrm" | "mathrm" | "mathit" | "mathbf" | "operatorname" => {
                Some(self.required_group(depth))
            }
            "mathbb" => {
                let body = self.required_group(depth);
                Some(Node::Text(to_math_blackboard(&inline_node(&body))))
            }
            "mathcal" | "mathscr" => {
                let body = self.required_group(depth);
                Some(Node::Text(to_math_script(&inline_node(&body))))
            }
            "hat" | "widehat" | "bar" | "overline" | "vec" | "dot" | "ddot" | "underline" => {
                let mark = match name.as_str() {
                    "hat" | "widehat" => '\u{302}',
                    "bar" | "overline" => '\u{305}',
                    "vec" => '\u{20d7}',
                    "dot" => '\u{307}',
                    "ddot" => '\u{308}',
                    _ => '\u{332}',
                };
                Some(Node::Accent {
                    body: Box::new(self.required_group(depth)),
                    mark,
                })
            }
            "left" | "right" => None,
            "quad" => Some(Node::Text("  ".into())),
            "qquad" => Some(Node::Text("    ".into())),
            "begin" | "end" => {
                // Environment names are TeX control structure. Keep the body
                // readable; row separators and alignment marks are handled.
                let _ = self.required_group(depth);
                None
            }
            _ => Some(Node::Text(
                command_glyph(&name)
                    .map(str::to_owned)
                    .unwrap_or_else(|| name),
            )),
        }
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
}

fn attach_script(nodes: &mut Vec<Node>, kind: char, script: Node) {
    let base = nodes.pop().unwrap_or_else(|| Node::Text(String::new()));
    let (base, mut sub, mut sup) = match base {
        Node::Script { base, sub, sup } => (base, sub, sup),
        other => (Box::new(other), None, None),
    };
    if kind == '^' {
        sup = Some(Box::new(script));
    } else {
        sub = Some(Box::new(script));
    }
    nodes.push(Node::Script { base, sub, sup });
}

/// Render inline LaTeX as compact Unicode mathematical notation.
pub fn inline(source: &str) -> String {
    if source.len() > MAX_MATH_BYTES {
        return source.to_string();
    }
    inline_node(&Parser::new(source).parse())
}

/// Render display LaTeX as centered terminal rows. Fractions retain a stacked
/// numerator/rule/denominator when the expression fits; very wide expressions
/// fall back to safe cell-bounded inline rows.
pub fn display(source: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if source.len() > MAX_MATH_BYTES {
        return wrap_cells(source, width);
    }
    let node = Parser::new(source).parse();
    let layout = layout_node(&node);
    if layout.width <= width {
        let left = (width - layout.width) / 2;
        return layout
            .rows
            .into_iter()
            .map(|row| format!("{}{}", " ".repeat(left), row))
            .collect();
    }
    wrap_cells(&inline_node(&node), width)
}

fn inline_node(node: &Node) -> String {
    match node {
        Node::Text(text) => text.clone(),
        Node::Row(nodes) => nodes.iter().map(inline_node).collect(),
        Node::Fraction(top, bottom) => {
            let top = inline_node(top);
            let bottom = inline_node(bottom);
            format!("{}/{}", parenthesize(&top), parenthesize(&bottom))
        }
        Node::Root(body) => {
            let body = inline_node(body);
            if is_simple(&body) {
                format!("√{body}")
            } else {
                format!("√({body})")
            }
        }
        Node::Script { base, sub, sup } => {
            let mut out = inline_node(base);
            if let Some(sub) = sub {
                let raw = inline_node(sub);
                out.push_str(
                    &to_script(&raw, ScriptKind::Sub).unwrap_or_else(|| format!("_({raw})")),
                );
            }
            if let Some(sup) = sup {
                let raw = inline_node(sup);
                out.push_str(
                    &to_script(&raw, ScriptKind::Super).unwrap_or_else(|| format!("^({raw})")),
                );
            }
            out
        }
        Node::Accent { body, mark } => inline_node(body)
            .chars()
            .flat_map(|ch| [ch, *mark])
            .collect(),
    }
}

fn parenthesize(text: &str) -> String {
    if is_simple(text) {
        text.to_string()
    } else {
        format!("({text})")
    }
}

fn is_simple(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_alphanumeric() || "∞πθφψαβγδεζηλμνξρστχω".contains(ch))
}

#[derive(Debug)]
struct Layout {
    rows: Vec<String>,
    width: usize,
    baseline: usize,
}

fn layout_node(node: &Node) -> Layout {
    match node {
        Node::Row(nodes) => compose_row(nodes.iter().map(layout_node).collect()),
        Node::Fraction(top, bottom) => {
            let top = layout_node(top);
            let bottom = layout_node(bottom);
            let width = top.width.max(bottom.width).saturating_add(2);
            let mut rows = pad_layout(top, width);
            let baseline = rows.len();
            rows.push("─".repeat(width));
            rows.extend(pad_layout(bottom, width));
            Layout {
                rows,
                width,
                baseline,
            }
        }
        _ => {
            let text = inline_node(node);
            Layout {
                width: UnicodeWidthStr::width(text.as_str()),
                rows: vec![text],
                baseline: 0,
            }
        }
    }
}

fn compose_row(parts: Vec<Layout>) -> Layout {
    if parts.is_empty() {
        return Layout {
            rows: vec![String::new()],
            width: 0,
            baseline: 0,
        };
    }
    let baseline = parts.iter().map(|part| part.baseline).max().unwrap_or(0);
    let below = parts
        .iter()
        .map(|part| part.rows.len().saturating_sub(part.baseline + 1))
        .max()
        .unwrap_or(0);
    let height = baseline + 1 + below;
    let width = parts.iter().map(|part| part.width).sum();
    let mut rows = vec![String::new(); height];
    for part in parts {
        let top = baseline.saturating_sub(part.baseline);
        for (row_index, row) in rows.iter_mut().enumerate() {
            if let Some(content) = row_index.checked_sub(top).and_then(|i| part.rows.get(i)) {
                row.push_str(content);
                row.push_str(
                    &" ".repeat(
                        part.width
                            .saturating_sub(UnicodeWidthStr::width(content.as_str())),
                    ),
                );
            } else {
                row.push_str(&" ".repeat(part.width));
            }
        }
    }
    Layout {
        rows,
        width,
        baseline,
    }
}

fn pad_layout(layout: Layout, width: usize) -> Vec<String> {
    layout
        .rows
        .into_iter()
        .map(|row| {
            let row_width = UnicodeWidthStr::width(row.as_str());
            let left = width.saturating_sub(row_width) / 2;
            let right = width.saturating_sub(row_width + left);
            format!("{}{}{}", " ".repeat(left), row, " ".repeat(right))
        })
        .collect()
}

fn wrap_cells(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let cells = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cells > 0 && used.saturating_add(cells) > width {
            rows.push(std::mem::take(&mut row));
            used = 0;
        }
        if cells <= width {
            row.push(ch);
            used += cells;
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

#[derive(Clone, Copy)]
enum ScriptKind {
    Super,
    Sub,
}

fn to_script(text: &str, kind: ScriptKind) -> Option<String> {
    text.chars()
        .map(|ch| match kind {
            ScriptKind::Super => superscript(ch),
            ScriptKind::Sub => subscript(ch),
        })
        .collect()
}

fn superscript(ch: char) -> Option<char> {
    Some(match ch {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        _ => return None,
    })
}

fn subscript(ch: char) -> Option<char> {
    Some(match ch {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        _ => return None,
    })
}

fn command_glyph(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" | "vartheta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" | "varpi" => "π",
        "rho" | "varrho" => "ρ",
        "sigma" | "varsigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" | "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "div" => "÷",
        "cdot" => "·",
        "ast" => "∗",
        "circ" => "∘",
        "bullet" => "•",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "sim" => "∼",
        "propto" => "∝",
        "in" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "subset" => "⊂",
        "supset" => "⊃",
        "subseteq" => "⊆",
        "supseteq" => "⊇",
        "cup" => "∪",
        "cap" => "∩",
        "setminus" => "∖",
        "emptyset" | "varnothing" => "∅",
        "forall" => "∀",
        "exists" => "∃",
        "neg" | "lnot" => "¬",
        "land" | "wedge" => "∧",
        "lor" | "vee" => "∨",
        "oplus" => "⊕",
        "otimes" => "⊗",
        "to" | "rightarrow" => "→",
        "leftarrow" => "←",
        "leftrightarrow" => "↔",
        "Rightarrow" | "implies" => "⇒",
        "Leftarrow" => "⇐",
        "Leftrightarrow" | "iff" => "⇔",
        "mapsto" => "↦",
        "ldots" | "dots" => "…",
        "cdots" => "⋯",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "angle" => "∠",
        "perp" => "⊥",
        "parallel" => "∥",
        "ell" => "ℓ",
        "hbar" => "ℏ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        _ => return None,
    })
}

fn to_math_blackboard(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'C' => 'ℂ',
            'H' => 'ℍ',
            'N' => 'ℕ',
            'P' => 'ℙ',
            'Q' => 'ℚ',
            'R' => 'ℝ',
            'Z' => 'ℤ',
            _ => ch,
        })
        .collect()
}

fn to_math_script(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'B' => 'ℬ',
            'E' => 'ℰ',
            'F' => 'ℱ',
            'H' => 'ℋ',
            'I' => 'ℐ',
            'L' => 'ℒ',
            'M' => 'ℳ',
            'R' => 'ℛ',
            _ => ch,
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/math__tests.rs"]
mod tests;
