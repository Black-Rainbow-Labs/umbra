//! Draws TeX math from a reply as terminal text. Inline math becomes one line of Unicode, such as
//! `x² + √(y+1)`. Display math becomes a block of rows, with fractions stacked over a line, roots
//! under a bar, limits above and below big operators, and matrices in tall brackets.

use crate::utils::screen::char_width;

/// How a piece of math is spaced from its neighbors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A letter, digit or symbol with no space around it.
    Ord,
    /// A binary operator such as `+`, with a space on each side unless it is a sign.
    Bin,
    /// A relation such as `=`, with a space on each side.
    Rel,
    /// A big operator such as `∑`, whose scripts go above and below in display math.
    Big,
    /// A function name such as `sin`, followed by a space before its argument.
    Func,
    /// A function name such as `lim`, whose subscript goes below in display math.
    FuncLimits,
}

/// One parsed piece of math.
#[derive(Debug, Clone)]
enum Node {
    Atom(String, Class),
    Group(Vec<Node>),
    Scripts { base: Box<Node>, sup: Option<Box<Node>>, sub: Option<Box<Node>> },
    Frac(Box<Node>, Box<Node>),
    Binom(Box<Node>, Box<Node>),
    Sqrt(Option<Box<Node>>, Box<Node>),
    Fenced(String, Box<Node>, String),
    Matrix { rows: Vec<Vec<Node>>, left: String, right: String, align: bool },
    Accent(char, Box<Node>),
    Space(usize),
}

/// A block of math rows with the row the text line sits on.
#[derive(Debug, Clone)]
struct MathBox {
    lines: Vec<String>,
    base: usize,
    width: usize,
}

/// Every symbol command Umbra knows, with its character and class.
const SYMBOLS: &[(&str, &str, Class)] = &[
    ("alpha", "α", Class::Ord), ("beta", "β", Class::Ord), ("gamma", "γ", Class::Ord), ("delta", "δ", Class::Ord),
    ("epsilon", "ϵ", Class::Ord), ("varepsilon", "ε", Class::Ord), ("zeta", "ζ", Class::Ord), ("eta", "η", Class::Ord),
    ("theta", "θ", Class::Ord), ("vartheta", "ϑ", Class::Ord), ("iota", "ι", Class::Ord), ("kappa", "κ", Class::Ord),
    ("lambda", "λ", Class::Ord), ("mu", "μ", Class::Ord), ("nu", "ν", Class::Ord), ("xi", "ξ", Class::Ord),
    ("pi", "π", Class::Ord), ("varpi", "ϖ", Class::Ord), ("rho", "ρ", Class::Ord), ("varrho", "ϱ", Class::Ord),
    ("sigma", "σ", Class::Ord), ("varsigma", "ς", Class::Ord), ("tau", "τ", Class::Ord), ("upsilon", "υ", Class::Ord),
    ("phi", "ϕ", Class::Ord), ("varphi", "φ", Class::Ord), ("chi", "χ", Class::Ord), ("psi", "ψ", Class::Ord),
    ("omega", "ω", Class::Ord), ("Gamma", "Γ", Class::Ord), ("Delta", "Δ", Class::Ord), ("Theta", "Θ", Class::Ord),
    ("Lambda", "Λ", Class::Ord), ("Xi", "Ξ", Class::Ord), ("Pi", "Π", Class::Ord), ("Sigma", "Σ", Class::Ord),
    ("Upsilon", "Υ", Class::Ord), ("Phi", "Φ", Class::Ord), ("Psi", "Ψ", Class::Ord), ("Omega", "Ω", Class::Ord),
    ("pm", "±", Class::Bin), ("mp", "∓", Class::Bin), ("times", "×", Class::Bin), ("div", "÷", Class::Bin),
    ("cdot", "·", Class::Bin), ("ast", "∗", Class::Bin), ("star", "⋆", Class::Bin), ("circ", "∘", Class::Bin),
    ("bullet", "∙", Class::Bin), ("cap", "∩", Class::Bin), ("cup", "∪", Class::Bin), ("wedge", "∧", Class::Bin),
    ("land", "∧", Class::Bin), ("vee", "∨", Class::Bin), ("lor", "∨", Class::Bin), ("oplus", "⊕", Class::Bin),
    ("ominus", "⊖", Class::Bin), ("otimes", "⊗", Class::Bin), ("odot", "⊙", Class::Bin), ("setminus", "∖", Class::Bin),
    ("sqcup", "⊔", Class::Bin), ("sqcap", "⊓", Class::Bin), ("uplus", "⊎", Class::Bin), ("bmod", "mod", Class::Bin),
    ("leq", "≤", Class::Rel), ("le", "≤", Class::Rel), ("geq", "≥", Class::Rel), ("ge", "≥", Class::Rel),
    ("leqslant", "⩽", Class::Rel), ("geqslant", "⩾", Class::Rel), ("neq", "≠", Class::Rel), ("ne", "≠", Class::Rel),
    ("approx", "≈", Class::Rel), ("equiv", "≡", Class::Rel), ("sim", "∼", Class::Rel), ("simeq", "≃", Class::Rel),
    ("cong", "≅", Class::Rel), ("propto", "∝", Class::Rel), ("ll", "≪", Class::Rel), ("gg", "≫", Class::Rel),
    ("in", "∈", Class::Rel), ("notin", "∉", Class::Rel), ("ni", "∋", Class::Rel), ("subset", "⊂", Class::Rel),
    ("supset", "⊃", Class::Rel), ("subseteq", "⊆", Class::Rel), ("supseteq", "⊇", Class::Rel), ("to", "→", Class::Rel),
    ("rightarrow", "→", Class::Rel), ("leftarrow", "←", Class::Rel), ("gets", "←", Class::Rel), ("Rightarrow", "⇒", Class::Rel),
    ("Leftarrow", "⇐", Class::Rel), ("leftrightarrow", "↔", Class::Rel), ("Leftrightarrow", "⇔", Class::Rel), ("iff", "⟺", Class::Rel),
    ("implies", "⟹", Class::Rel), ("impliedby", "⟸", Class::Rel), ("mapsto", "↦", Class::Rel), ("longrightarrow", "⟶", Class::Rel),
    ("longleftarrow", "⟵", Class::Rel), ("Longrightarrow", "⟹", Class::Rel), ("Longleftarrow", "⟸", Class::Rel), ("longmapsto", "⟼", Class::Rel),
    ("uparrow", "↑", Class::Rel), ("downarrow", "↓", Class::Rel), ("hookrightarrow", "↪", Class::Rel), ("perp", "⊥", Class::Rel),
    ("parallel", "∥", Class::Rel), ("mid", "∣", Class::Rel), ("models", "⊨", Class::Rel), ("vdash", "⊢", Class::Rel),
    ("prec", "≺", Class::Rel), ("succ", "≻", Class::Rel), ("preceq", "⪯", Class::Rel), ("succeq", "⪰", Class::Rel),
    ("doteq", "≐", Class::Rel), ("coloneqq", "≔", Class::Rel), ("triangleq", "≜", Class::Rel), ("asymp", "≍", Class::Rel),
    ("nleq", "≰", Class::Rel), ("ngeq", "≱", Class::Rel), ("sqsubseteq", "⊑", Class::Rel), ("lt", "<", Class::Rel),
    ("gt", ">", Class::Rel), ("nearrow", "↗", Class::Rel), ("searrow", "↘", Class::Rel), ("rightleftharpoons", "⇌", Class::Rel),
    ("infty", "∞", Class::Ord), ("partial", "∂", Class::Ord), ("nabla", "∇", Class::Ord), ("forall", "∀", Class::Ord),
    ("exists", "∃", Class::Ord), ("nexists", "∄", Class::Ord), ("emptyset", "∅", Class::Ord), ("varnothing", "∅", Class::Ord),
    ("neg", "¬", Class::Ord), ("lnot", "¬", Class::Ord), ("angle", "∠", Class::Ord), ("triangle", "△", Class::Ord),
    ("ldots", "…", Class::Ord), ("cdots", "⋯", Class::Ord), ("vdots", "⋮", Class::Ord), ("ddots", "⋱", Class::Ord),
    ("dots", "…", Class::Ord), ("prime", "′", Class::Ord), ("hbar", "ℏ", Class::Ord), ("ell", "ℓ", Class::Ord),
    ("Re", "ℜ", Class::Ord), ("Im", "ℑ", Class::Ord), ("aleph", "ℵ", Class::Ord), ("wp", "℘", Class::Ord),
    ("top", "⊤", Class::Ord), ("bot", "⊥", Class::Ord), ("degree", "°", Class::Ord), ("checkmark", "✓", Class::Ord),
    ("dagger", "†", Class::Ord), ("langle", "⟨", Class::Ord), ("rangle", "⟩", Class::Ord), ("lceil", "⌈", Class::Ord),
    ("rceil", "⌉", Class::Ord), ("lfloor", "⌊", Class::Ord), ("rfloor", "⌋", Class::Ord), ("vert", "|", Class::Ord),
    ("lvert", "|", Class::Ord), ("rvert", "|", Class::Ord), ("Vert", "‖", Class::Ord), ("lVert", "‖", Class::Ord),
    ("rVert", "‖", Class::Ord), ("backslash", "\\", Class::Ord), ("therefore", "∴", Class::Ord), ("because", "∵", Class::Ord),
    ("square", "□", Class::Ord), ("blacksquare", "■", Class::Ord), ("imath", "ı", Class::Ord), ("jmath", "ȷ", Class::Ord),
    ("lbrace", "{", Class::Ord), ("rbrace", "}", Class::Ord), ("colon", ":", Class::Ord), ("surd", "√", Class::Ord),
    ("sum", "∑", Class::Big), ("prod", "∏", Class::Big), ("coprod", "∐", Class::Big), ("int", "∫", Class::Big),
    ("iint", "∬", Class::Big), ("iiint", "∭", Class::Big), ("oint", "∮", Class::Big), ("bigcup", "⋃", Class::Big),
    ("bigcap", "⋂", Class::Big), ("bigoplus", "⨁", Class::Big), ("bigotimes", "⨂", Class::Big), ("bigvee", "⋁", Class::Big),
    ("bigwedge", "⋀", Class::Big), ("bigsqcup", "⨆", Class::Big),
];

/// Function names drawn upright, and whether their subscript goes below in display math.
const FUNCTIONS: &[(&str, bool)] = &[
    ("sin", false), ("cos", false), ("tan", false), ("cot", false), ("sec", false), ("csc", false), ("arcsin", false),
    ("arccos", false), ("arctan", false), ("sinh", false), ("cosh", false), ("tanh", false), ("coth", false), ("log", false),
    ("ln", false), ("lg", false), ("exp", false), ("dim", false), ("ker", false), ("deg", false), ("hom", false),
    ("arg", false), ("lim", true), ("limsup", true), ("liminf", true), ("max", true), ("min", true), ("sup", true),
    ("inf", true), ("det", true), ("gcd", true), ("lcm", true), ("Pr", true), ("argmax", true), ("argmin", true),
];

/// Commands that change only the look of the text, which a terminal cannot show, so they are dropped.
const IGNORED: &[&str] = &[
    "displaystyle", "textstyle", "scriptstyle", "scriptscriptstyle", "limits", "nolimits", "big", "Big", "bigg", "Bigg",
    "bigl", "bigr", "Bigl", "Bigr", "biggl", "biggr", "left.", "right.", "nonumber", "notag", "label", "tag", "middle",
    "rm", "bf", "it", "cal", "mathstrut", "strut", "phantom", "hphantom", "vphantom",
];

/// Superscript forms of the characters that have one.
const SUPERSCRIPTS: &[(char, char)] = &[
    ('0', '⁰'), ('1', '¹'), ('2', '²'), ('3', '³'), ('4', '⁴'), ('5', '⁵'), ('6', '⁶'), ('7', '⁷'), ('8', '⁸'), ('9', '⁹'),
    ('+', '⁺'), ('−', '⁻'), ('-', '⁻'), ('=', '⁼'), ('(', '⁽'), (')', '⁾'), ('a', 'ᵃ'), ('b', 'ᵇ'), ('c', 'ᶜ'), ('d', 'ᵈ'),
    ('e', 'ᵉ'), ('f', 'ᶠ'), ('g', 'ᵍ'), ('h', 'ʰ'), ('i', 'ⁱ'), ('j', 'ʲ'), ('k', 'ᵏ'), ('l', 'ˡ'), ('m', 'ᵐ'), ('n', 'ⁿ'),
    ('o', 'ᵒ'), ('p', 'ᵖ'), ('r', 'ʳ'), ('s', 'ˢ'), ('t', 'ᵗ'), ('u', 'ᵘ'), ('v', 'ᵛ'), ('w', 'ʷ'), ('x', 'ˣ'), ('y', 'ʸ'),
    ('z', 'ᶻ'), ('A', 'ᴬ'), ('B', 'ᴮ'), ('D', 'ᴰ'), ('E', 'ᴱ'), ('G', 'ᴳ'), ('H', 'ᴴ'), ('I', 'ᴵ'), ('J', 'ᴶ'), ('K', 'ᴷ'),
    ('L', 'ᴸ'), ('M', 'ᴹ'), ('N', 'ᴺ'), ('O', 'ᴼ'), ('P', 'ᴾ'), ('R', 'ᴿ'), ('T', 'ᵀ'), ('U', 'ᵁ'), ('V', 'ⱽ'), ('W', 'ᵂ'),
    ('β', 'ᵝ'), ('γ', 'ᵞ'), ('δ', 'ᵟ'), ('θ', 'ᶿ'), ('φ', 'ᵠ'), ('ϕ', 'ᵠ'), ('χ', 'ᵡ'), ('′', '′'), ('∗', '*'), ('·', '·'),
];

/// Subscript forms of the characters that have one.
const SUBSCRIPTS: &[(char, char)] = &[
    ('0', '₀'), ('1', '₁'), ('2', '₂'), ('3', '₃'), ('4', '₄'), ('5', '₅'), ('6', '₆'), ('7', '₇'), ('8', '₈'), ('9', '₉'),
    ('+', '₊'), ('−', '₋'), ('-', '₋'), ('=', '₌'), ('(', '₍'), (')', '₎'), ('a', 'ₐ'), ('e', 'ₑ'), ('h', 'ₕ'), ('i', 'ᵢ'),
    ('j', 'ⱼ'), ('k', 'ₖ'), ('l', 'ₗ'), ('m', 'ₘ'), ('n', 'ₙ'), ('o', 'ₒ'), ('p', 'ₚ'), ('r', 'ᵣ'), ('s', 'ₛ'), ('t', 'ₜ'),
    ('u', 'ᵤ'), ('v', 'ᵥ'), ('x', 'ₓ'), ('β', 'ᵦ'), ('γ', 'ᵧ'), ('ρ', 'ᵨ'), ('φ', 'ᵩ'), ('ϕ', 'ᵩ'), ('χ', 'ᵪ'),
];

/// Draws math on one line, for inline math and for display math that does not fit.
/// @param source - the TeX source between the dollar signs
pub fn inline(source: &str) -> String {
    linear_seq(&parse(source), false).trim().to_string()
}

/// Draws display math as rows no wider than `width`, centered, or None when the drawing is wider.
/// @param source - the TeX source between the dollar signs
/// @param width - the columns available
pub fn display(source: &str, width: usize) -> Option<Vec<String>> {
    let laid = seq_box(&parse(source));
    if laid.width > width || laid.lines.is_empty() {
        return None;
    }
    let left = (width - laid.width) / 2;
    Some(laid.lines.iter().map(|line| format!("{}{}", " ".repeat(left), line).trim_end().to_string()).collect())
}

/// Rewrites `\(...\)` as `$...$` and `\[...\]` as `$$...$$` outside code, so the markdown parser
/// reads both forms models write as math.
/// @param markdown - the reply source
pub fn convert_delimiters(markdown: &str) -> String {
    if !markdown.contains("\\(") && !markdown.contains("\\[") {
        return markdown.to_string();
    }
    let display = regex::Regex::new(r"(?s)\\\[(.+?)\\\]").expect("valid regex");
    let inline = regex::Regex::new(r"\\\((.+?)\\\)").expect("valid regex");
    let mut out = String::with_capacity(markdown.len());
    for (text, code) in code_segments(markdown) {
        if code {
            out.push_str(text);
            continue;
        }
        let step = display.replace_all(text, |c: &regex::Captures| format!("$${}$$", &c[1]));
        out.push_str(&inline.replace_all(&step, |c: &regex::Captures| format!("${}$", c[1].trim())));
    }
    out
}

/// Splits markdown into pieces that are code (fenced blocks and code spans) and pieces that are not.
/// @param markdown - the reply source
fn code_segments(markdown: &str) -> Vec<(&str, bool)> {
    let mut pieces = Vec::new();
    let (mut start, mut at, mut fence): (usize, usize, Option<String>) = (0, 0, None);
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let marker: String = trimmed.chars().take_while(|c| *c == '`' || *c == '~').collect();
        let is_fence = marker.len() >= 3 && marker.chars().all(|c| c == marker.chars().next().unwrap_or('`'));
        match (&fence, is_fence) {
            (None, true) => {
                split_spans(&markdown[start..at], &mut pieces);
                start = at;
                fence = Some(marker);
            }
            (Some(open), true) if marker.starts_with(open.as_str()) => {
                pieces.push((&markdown[start..at + line.len()], true));
                start = at + line.len();
                fence = None;
            }
            _ => {}
        }
        at += line.len();
    }
    if fence.is_some() {
        pieces.push((&markdown[start..], true));
    } else {
        split_spans(&markdown[start..], &mut pieces);
    }
    pieces
}

/// Splits text outside fenced blocks at its code spans.
/// @param text - the text
/// @param pieces - where the pieces go
fn split_spans<'a>(text: &'a str, pieces: &mut Vec<(&'a str, bool)>) {
    let bytes = text.as_bytes();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
        let closer = "`".repeat(run);
        match text[i + run..].find(&closer) {
            Some(offset) => {
                pieces.push((&text[start..i], false));
                let end = i + run + offset + run;
                pieces.push((&text[i..end], true));
                start = end;
                i = end;
            }
            None => i += run,
        }
    }
    pieces.push((&text[start..], false));
}

/// The parser's position in the source.
struct Parser {
    chars: Vec<char>,
    at: usize,
}

/// Parses TeX math into nodes.
/// @param source - the TeX source
fn parse(source: &str) -> Vec<Node> {
    let mut p = Parser { chars: source.chars().collect(), at: 0 };
    let mut nodes = Vec::new();
    while p.at < p.chars.len() {
        nodes.extend(parse_seq(&mut p, &[]));
        if p.at < p.chars.len() {
            p.at += 1;
        }
    }
    nodes
}

/// Parses nodes until the end, a `}`, or a command in `stops` such as `right`, `end`, `&` or `\\`.
/// The stop is left unread.
/// @param p - the parser
/// @param stops - what ends the sequence besides `}`
fn parse_seq(p: &mut Parser, stops: &[&str]) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    while let Some(&c) = p.chars.get(p.at) {
        if c == '}' || at_stop(p, stops) {
            break;
        }
        match c {
            '^' | '_' => {
                p.at += 1;
                let arg = parse_arg(p);
                attach_script(&mut nodes, c == '^', arg);
            }
            '\'' => {
                p.at += 1;
                attach_script(&mut nodes, true, Node::Atom("′".into(), Class::Ord));
            }
            _ => {
                if let Some(node) = parse_atom(p) {
                    nodes.push(node);
                }
            }
        }
    }
    nodes
}

/// True when the parser is at one of the stops: `&`, `\\`, or a command name.
/// @param p - the parser
/// @param stops - the stops
fn at_stop(p: &Parser, stops: &[&str]) -> bool {
    let rest: String = p.chars[p.at..].iter().take(8).collect();
    stops.iter().any(|stop| match *stop {
        "&" => rest.starts_with('&'),
        "\\\\" => rest.starts_with("\\\\"),
        name => rest.starts_with(&format!("\\{name}")) && !p.chars.get(p.at + 1 + name.len()).is_some_and(|c| c.is_ascii_alphabetic()),
    })
}

/// Adds a superscript or subscript to the last node.
/// @param nodes - the nodes so far
/// @param sup - true for a superscript
/// @param arg - the script
fn attach_script(nodes: &mut Vec<Node>, sup: bool, arg: Node) {
    let last = nodes.pop().unwrap_or(Node::Atom(String::new(), Class::Ord));
    let node = match last {
        Node::Scripts { base, sup: s, sub } => {
            if sup { Node::Scripts { base, sup: Some(Box::new(arg)), sub } } else { Node::Scripts { base, sup: s, sub: Some(Box::new(arg)) } }
        }
        base => {
            let arg = Some(Box::new(arg));
            if sup { Node::Scripts { base: Box::new(base), sup: arg, sub: None } } else { Node::Scripts { base: Box::new(base), sup: None, sub: arg } }
        }
    };
    nodes.push(node);
}

/// Parses one argument: a group in braces, a command, or a single character.
/// @param p - the parser
fn parse_arg(p: &mut Parser) -> Node {
    skip_spaces(p);
    match p.chars.get(p.at) {
        Some('{') => parse_group(p),
        Some(_) => parse_atom(p).unwrap_or(Node::Group(Vec::new())),
        None => Node::Group(Vec::new()),
    }
}

/// Parses a group in braces.
/// @param p - the parser, at the `{`
fn parse_group(p: &mut Parser) -> Node {
    p.at += 1;
    let nodes = parse_seq(p, &[]);
    if p.chars.get(p.at) == Some(&'}') {
        p.at += 1;
    }
    Node::Group(nodes)
}

/// Skips whitespace.
/// @param p - the parser
fn skip_spaces(p: &mut Parser) {
    while p.chars.get(p.at).is_some_and(|c| c.is_whitespace()) {
        p.at += 1;
    }
}

/// Parses one atom: a group, a command, or a character. Returns None for whitespace and dropped commands.
/// @param p - the parser
fn parse_atom(p: &mut Parser) -> Option<Node> {
    let c = *p.chars.get(p.at)?;
    if c == '{' {
        return Some(parse_group(p));
    }
    p.at += 1;
    if c == '\\' {
        return parse_command(p);
    }
    if c.is_whitespace() {
        return None;
    }
    Some(char_node(c))
}

/// The node for a plain character.
/// @param c - the character
fn char_node(c: char) -> Node {
    let (text, class) = match c {
        '+' => ("+".to_string(), Class::Bin),
        '-' => ("−".to_string(), Class::Bin),
        '*' => ("∗".to_string(), Class::Bin),
        '=' | '<' | '>' => (c.to_string(), Class::Rel),
        '~' => (" ".to_string(), Class::Ord),
        _ => (c.to_string(), Class::Ord),
    };
    Node::Atom(text, class)
}

/// Reads a command's name: letters, or one other character.
/// @param p - the parser, after the backslash
fn command_name(p: &mut Parser) -> String {
    let mut name = String::new();
    while let Some(&c) = p.chars.get(p.at) {
        if !c.is_ascii_alphabetic() {
            break;
        }
        name.push(c);
        p.at += 1;
    }
    if name.is_empty() {
        if let Some(&c) = p.chars.get(p.at) {
            name.push(c);
            p.at += 1;
        }
    }
    name
}

/// Parses a command after its backslash.
/// @param p - the parser, after the backslash
fn parse_command(p: &mut Parser) -> Option<Node> {
    let name = command_name(p);
    if let Some(node) = structure_command(p, &name) {
        return Some(node);
    }
    if let Some((_, text, class)) = SYMBOLS.iter().find(|(n, _, _)| *n == name) {
        return Some(Node::Atom(text.to_string(), *class));
    }
    if let Some((_, limits)) = FUNCTIONS.iter().find(|(n, _)| *n == name) {
        return Some(Node::Atom(name.clone(), if *limits { Class::FuncLimits } else { Class::Func }));
    }
    match name.as_str() {
        "," | ":" | ">" | ";" | " " => Some(Node::Space(1)),
        "!" => None,
        "quad" => Some(Node::Space(2)),
        "qquad" => Some(Node::Space(4)),
        "{" | "}" | "%" | "$" | "#" | "&" | "_" | "|" => Some(Node::Atom(if name == "|" { "‖".into() } else { name }, Class::Ord)),
        "\\" => Some(Node::Space(1)),
        n if IGNORED.contains(&n) => None,
        _ => Some(Node::Atom(name, Class::Ord)),
    }
}

/// Parses the commands that take arguments: fractions, roots, fences, environments, text and fonts.
/// @param p - the parser, after the command name
/// @param name - the command name
fn structure_command(p: &mut Parser, name: &str) -> Option<Node> {
    match name {
        "frac" | "dfrac" | "tfrac" | "cfrac" => Some(Node::Frac(Box::new(parse_arg(p)), Box::new(parse_arg(p)))),
        "binom" | "dbinom" | "tbinom" | "choose" => Some(Node::Binom(Box::new(parse_arg(p)), Box::new(parse_arg(p)))),
        "sqrt" => {
            let index = parse_optional(p);
            Some(Node::Sqrt(index.map(Box::new), Box::new(parse_arg(p))))
        }
        "left" => Some(parse_fenced(p)),
        "begin" => Some(parse_environment(p)),
        "text" | "textrm" | "textit" | "textbf" | "mbox" | "operatorname" | "mathrm" | "textsf" | "texttt" | "hbox" => {
            Some(Node::Atom(raw_group(p), if name == "operatorname" { Class::Func } else { Class::Ord }))
        }
        "mathbb" | "mathcal" | "mathfrak" | "mathscr" => Some(font_node(&parse_arg(p), name)),
        "mathbf" | "mathit" | "mathsf" | "mathtt" | "boldsymbol" | "bm" | "pmb" => Some(parse_arg(p)),
        "overline" | "bar" => Some(Node::Accent('\u{0305}', Box::new(parse_arg(p)))),
        "hat" | "widehat" => Some(Node::Accent('\u{0302}', Box::new(parse_arg(p)))),
        "tilde" | "widetilde" => Some(Node::Accent('\u{0303}', Box::new(parse_arg(p)))),
        "vec" | "overrightarrow" => Some(Node::Accent('\u{20d7}', Box::new(parse_arg(p)))),
        "dot" => Some(Node::Accent('\u{0307}', Box::new(parse_arg(p)))),
        "ddot" => Some(Node::Accent('\u{0308}', Box::new(parse_arg(p)))),
        "underline" => Some(Node::Accent('\u{0332}', Box::new(parse_arg(p)))),
        _ => None,
    }
}

/// Parses an optional argument in square brackets, such as the index of `\sqrt[3]{x}`.
/// @param p - the parser
fn parse_optional(p: &mut Parser) -> Option<Node> {
    skip_spaces(p);
    if p.chars.get(p.at) != Some(&'[') {
        return None;
    }
    p.at += 1;
    let mut nodes = Vec::new();
    while let Some(&c) = p.chars.get(p.at) {
        if c == ']' {
            p.at += 1;
            break;
        }
        if let Some(node) = parse_atom(p) {
            nodes.push(node);
        }
    }
    Some(Node::Group(nodes))
}

/// Reads a group's text as it is written, for `\text{...}`.
/// @param p - the parser
fn raw_group(p: &mut Parser) -> String {
    skip_spaces(p);
    if p.chars.get(p.at) != Some(&'{') {
        return p.chars.get(p.at).map(|c| { p.at += 1; c.to_string() }).unwrap_or_default();
    }
    let (mut depth, mut text) = (0, String::new());
    while let Some(&c) = p.chars.get(p.at) {
        p.at += 1;
        match c {
            '{' if depth == 0 => depth = 1,
            '{' => { depth += 1; text.push(c) }
            '}' if depth == 1 => break,
            '}' => { depth -= 1; text.push(c) }
            _ => text.push(c),
        }
    }
    text
}

/// A blackboard bold, script or fraktur letter for `\mathbb` and its kin, or the plain text.
/// @param arg - the argument
/// @param font - the command name
fn font_node(arg: &Node, font: &str) -> Node {
    let text = linear(arg, true);
    let mapped: String = text.chars().map(|c| match (font, c) {
        ("mathbb", 'R') => 'ℝ', ("mathbb", 'N') => 'ℕ', ("mathbb", 'Z') => 'ℤ', ("mathbb", 'Q') => 'ℚ',
        ("mathbb", 'C') => 'ℂ', ("mathbb", 'P') => 'ℙ', ("mathbb", 'H') => 'ℍ', ("mathbb", 'E') => '𝔼',
        ("mathbb", '1') => '𝟙', ("mathcal" | "mathscr", 'L') => 'ℒ', ("mathcal" | "mathscr", 'H') => 'ℋ',
        ("mathcal" | "mathscr", 'F') => 'ℱ', ("mathcal" | "mathscr", 'O') => '𝒪', ("mathcal" | "mathscr", 'N') => '𝒩',
        _ => c,
    }).collect();
    Node::Atom(mapped, Class::Ord)
}

/// Parses `\left( ... \right)`.
/// @param p - the parser, after `\left`
fn parse_fenced(p: &mut Parser) -> Node {
    let left = delimiter(p);
    let body = parse_seq(p, &["right"]);
    let right = if at_stop(p, &["right"]) {
        p.at += 6;
        delimiter(p)
    } else {
        String::new()
    };
    Node::Fenced(left, Box::new(Node::Group(body)), right)
}

/// Reads the delimiter after `\left` or `\right`. A dot means none.
/// @param p - the parser
fn delimiter(p: &mut Parser) -> String {
    skip_spaces(p);
    let Some(&c) = p.chars.get(p.at) else { return String::new() };
    p.at += 1;
    if c == '.' {
        return String::new();
    }
    if c != '\\' {
        return c.to_string();
    }
    let name = command_name(p);
    match name.as_str() {
        "{" | "lbrace" => "{".into(),
        "}" | "rbrace" => "}".into(),
        "|" | "Vert" => "‖".into(),
        "langle" => "⟨".into(),
        "rangle" => "⟩".into(),
        "lvert" | "rvert" | "vert" => "|".into(),
        "lfloor" => "⌊".into(),
        "rfloor" => "⌋".into(),
        "lceil" => "⌈".into(),
        "rceil" => "⌉".into(),
        _ => String::new(),
    }
}

/// Parses `\begin{env} ... \end{env}`: rows split at `\\`, cells split at `&`.
/// @param p - the parser, after `\begin`
fn parse_environment(p: &mut Parser) -> Node {
    let env = raw_group(p);
    let mut rows: Vec<Vec<Node>> = Vec::new();
    let mut row: Vec<Node> = Vec::new();
    loop {
        let cell = parse_seq(p, &["&", "\\\\", "end"]);
        row.push(Node::Group(cell));
        if at_stop(p, &["&"]) {
            p.at += 1;
        } else if at_stop(p, &["\\\\"]) {
            p.at += 2;
            rows.push(std::mem::take(&mut row));
        } else {
            if at_stop(p, &["end"]) {
                p.at += 4;
                raw_group(p);
            } else if p.at < p.chars.len() {
                p.at += 1;
                continue;
            }
            break;
        }
    }
    if row.iter().any(|cell| !matches!(cell, Node::Group(g) if g.is_empty())) || rows.is_empty() {
        rows.push(row);
    }
    let base = env.trim_end_matches('*');
    let (left, right) = match base {
        "pmatrix" => ("(", ")"),
        "bmatrix" => ("[", "]"),
        "Bmatrix" => ("{", "}"),
        "vmatrix" => ("|", "|"),
        "Vmatrix" => ("‖", "‖"),
        "cases" => ("{", ""),
        _ => ("", ""),
    };
    let align = matches!(base, "aligned" | "align" | "alignat" | "split" | "eqnarray" | "gathered" | "cases");
    Node::Matrix { rows, left: left.into(), right: right.into(), align }
}

/// The class a node is spaced by.
/// @param node - the node
fn class_of(node: &Node) -> Class {
    match node {
        Node::Atom(_, class) => *class,
        Node::Scripts { base, .. } => match class_of(base) {
            Class::Big | Class::FuncLimits | Class::Func => Class::Ord,
            other => other,
        },
        _ => Class::Ord,
    }
}

/// The spaces before and after each node of a sequence: around operators and relations, after a
/// function name, and none around a sign.
/// @param nodes - the sequence
/// @param tight - true inside a script, where nothing is spaced
fn spacing(nodes: &[Node], tight: bool) -> Vec<(bool, bool)> {
    let mut out = vec![(false, false); nodes.len()];
    if tight {
        return out;
    }
    for (i, node) in nodes.iter().enumerate() {
        let before = if i == 0 { None } else { Some(class_of(&nodes[i - 1])) };
        let opening = i > 0 && matches!(&nodes[i - 1], Node::Atom(t, _) if t == "(" || t == "[" || t == "{" || t == ",");
        match class_of(node) {
            Class::Rel => out[i] = (i > 0, i + 1 < nodes.len()),
            Class::Bin if before.is_some_and(|b| !matches!(b, Class::Bin | Class::Rel | Class::Big)) && !opening => out[i] = (true, i + 1 < nodes.len()),
            Class::Func | Class::FuncLimits if i + 1 < nodes.len() && !starts_with_paren(&nodes[i + 1]) => out[i].1 = true,
            Class::Big if i + 1 < nodes.len() => out[i].1 = true,
            _ => {}
        }
        if matches!(node, Node::Atom(t, Class::Ord) if t == ",") && i + 1 < nodes.len() {
            out[i].1 = true;
        }
        if matches!(node, Node::Scripts { base, .. } if matches!(**base, Node::Atom(_, Class::Big) | Node::Atom(_, Class::FuncLimits) | Node::Atom(_, Class::Func))) && i + 1 < nodes.len() {
            out[i].1 = true;
        }
    }
    out
}

/// True when a node starts with an opening parenthesis or bracket.
/// @param node - the node
fn starts_with_paren(node: &Node) -> bool {
    linear(node, true).starts_with(['(', '[', '|', '⟨'])
}

/// Draws a sequence on one line.
/// @param nodes - the sequence
/// @param tight - true inside a script
fn linear_seq(nodes: &[Node], tight: bool) -> String {
    let gaps = spacing(nodes, tight);
    let mut out = String::new();
    for (node, (before, after)) in nodes.iter().zip(gaps) {
        if before && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(&linear(node, tight));
        if after {
            out.push(' ');
        }
    }
    out
}

/// Draws a node on one line.
/// @param node - the node
/// @param tight - true inside a script
fn linear(node: &Node, tight: bool) -> String {
    match node {
        Node::Atom(text, _) => text.clone(),
        Node::Space(n) => " ".repeat(*n),
        Node::Group(nodes) => linear_seq(nodes, tight),
        Node::Scripts { base, sup, sub } => {
            let mut out = linear(base, tight);
            if let Some(sub) = sub {
                out.push_str(&script_text(&linear(sub, true), SUBSCRIPTS, '_'));
            }
            if let Some(sup) = sup {
                out.push_str(&script_text(&linear(sup, true), SUPERSCRIPTS, '^'));
            }
            out
        }
        Node::Frac(num, den) => format!("{}/{}", wrap_parens(&linear(num, tight)), wrap_parens(&linear(den, tight))),
        Node::Binom(n, k) => format!("C({}, {})", linear(n, tight), linear(k, tight)),
        Node::Sqrt(index, body) => format!("{}{}", root_sign(index.as_deref()), wrap_parens(&linear(body, tight))),
        Node::Fenced(left, body, right) => format!("{left}{}{right}", linear(body, tight)),
        Node::Matrix { rows, left, right, .. } => {
            let rows: Vec<String> = rows.iter().map(|r| r.iter().map(|c| linear(c, tight).trim().to_string()).collect::<Vec<_>>().join(" ")).collect();
            let close = if right.is_empty() && left == "{" { "" } else { right.as_str() };
            format!("{}{}{}", if left.is_empty() { "[" } else { left }, rows.join("; "), if left.is_empty() { "]" } else { close })
        }
        Node::Accent(mark, body) => accented(&linear(body, tight), *mark),
    }
}

/// A script as Unicode small characters when every character has one, else `^x` or `^(...)`.
/// @param text - the script text
/// @param table - the superscript or subscript table
/// @param marker - `^` or `_`
fn script_text(text: &str, table: &[(char, char)], marker: char) -> String {
    let small: Option<String> = text.chars().map(|c| table.iter().find(|(from, _)| *from == c).map(|(_, to)| *to)).collect();
    match small {
        Some(small) if !small.is_empty() => small,
        _ if text.chars().count() == 1 => format!("{marker}{text}"),
        _ => format!("{marker}({text})"),
    }
}

/// Puts parentheses around text that is more than one character and not a plain number.
/// @param text - the text
fn wrap_parens(text: &str) -> String {
    let text = text.trim();
    let simple = text.chars().count() <= 1 || text.chars().all(|c| c.is_ascii_digit() || c == '.') || (text.starts_with('(') && text.ends_with(')'));
    if simple { text.to_string() } else { format!("({text})") }
}

/// The root sign for an index: `√`, `∛`, `∜`, or the index as a superscript before `√`.
/// @param index - the root's index, if any
fn root_sign(index: Option<&Node>) -> String {
    match index.map(|i| linear(i, true)) {
        None => "√".into(),
        Some(i) if i == "3" => "∛".into(),
        Some(i) if i == "4" => "∜".into(),
        Some(i) => format!("{}√", script_text(&i, SUPERSCRIPTS, '^')),
    }
}

/// Adds a combining mark after every character of the text, such as a bar over each letter.
/// @param text - the text
/// @param mark - the combining mark
fn accented(text: &str, mark: char) -> String {
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if !c.is_whitespace() {
            out.push(mark);
        }
    }
    out
}

/// The columns text takes.
/// @param text - the text
fn width_of(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// A one row box.
/// @param text - the text
fn text_box(text: &str) -> MathBox {
    MathBox { lines: vec![text.to_string()], base: 0, width: width_of(text) }
}

/// Pads a box to a width, with the content placed by `left` columns.
/// @param b - the box
/// @param width - the new width
/// @param left - the columns in front of the content
fn pad_box(b: &MathBox, width: usize, left: usize) -> Vec<String> {
    b.lines.iter().map(|line| {
        let right = width.saturating_sub(left + width_of(line));
        format!("{}{line}{}", " ".repeat(left), " ".repeat(right))
    }).collect()
}

/// Joins boxes side by side, lined up on their base rows.
/// @param boxes - the boxes, left to right
fn hcat(boxes: &[MathBox]) -> MathBox {
    let above = boxes.iter().map(|b| b.base).max().unwrap_or(0);
    let below = boxes.iter().map(|b| b.lines.len() - b.base - 1).max().unwrap_or(0);
    let mut lines = vec![String::new(); above + below + 1];
    for b in boxes {
        let top = above - b.base;
        let padded = pad_box(b, b.width, 0);
        for (row, line) in lines.iter_mut().enumerate() {
            match row.checked_sub(top).and_then(|i| padded.get(i)) {
                Some(text) => line.push_str(text),
                None => line.push_str(&" ".repeat(b.width)),
            }
        }
    }
    MathBox { lines, base: above, width: boxes.iter().map(|b| b.width).sum() }
}

/// Stacks boxes, each centered, with the base on the row `base_part` box's base.
/// @param parts - the boxes, top to bottom
/// @param base_part - the index of the box whose base is the result's base
fn vstack(parts: &[MathBox], base_part: usize) -> MathBox {
    let width = parts.iter().map(|b| b.width).max().unwrap_or(0);
    let mut lines = Vec::new();
    let mut base = 0;
    for (index, part) in parts.iter().enumerate() {
        if index == base_part {
            base = lines.len() + part.base;
        }
        lines.extend(pad_box(part, width, (width - part.width) / 2));
    }
    MathBox { lines, base, width }
}

/// Lays out a sequence as a box, with the same spacing as the one line form.
/// @param nodes - the sequence
fn seq_box(nodes: &[Node]) -> MathBox {
    let gaps = spacing(nodes, false);
    let mut boxes = Vec::new();
    for (node, (before, after)) in nodes.iter().zip(gaps) {
        if before {
            boxes.push(text_box(" "));
        }
        boxes.push(node_box(node));
        if after {
            boxes.push(text_box(" "));
        }
    }
    if boxes.is_empty() {
        return text_box("");
    }
    hcat(&boxes)
}

/// Lays out a node as a box.
/// @param node - the node
fn node_box(node: &Node) -> MathBox {
    match node {
        Node::Group(nodes) => seq_box(nodes),
        Node::Scripts { base, sup, sub } => scripts_box(base, sup.as_deref(), sub.as_deref()),
        Node::Frac(num, den) => frac_box(&node_box(num), &node_box(den)),
        Node::Binom(n, k) => fence_box("(", &vstack(&[node_box(n), node_box(k)], 0), ")"),
        Node::Sqrt(index, body) => sqrt_box(index.as_deref(), &node_box(body)),
        Node::Fenced(left, body, right) => fence_box(left, &node_box(body), right),
        Node::Matrix { rows, left, right, align } => fence_box(left, &matrix_box(rows, *align), right),
        Node::Accent(..) | Node::Atom(..) | Node::Space(_) => text_box(&linear(node, false)),
    }
}

/// A fraction: the numerator over a line over the denominator.
/// @param num - the numerator
/// @param den - the denominator
fn frac_box(num: &MathBox, den: &MathBox) -> MathBox {
    let width = num.width.max(den.width) + 2;
    let bar = text_box(&"─".repeat(width));
    let mut stacked = vstack(&[num.clone(), bar, den.clone()], 1);
    stacked.base = num.lines.len();
    stacked
}

/// A root: a line over the content and the root sign on the bottom row.
/// @param index - the root's index, if any
/// @param body - the content
fn sqrt_box(index: Option<&Node>, body: &MathBox) -> MathBox {
    let sign = root_sign(index);
    let sign_width = width_of(&sign);
    let mut lines = vec![format!("{}{}", " ".repeat(sign_width), "─".repeat(body.width))];
    let last = body.lines.len() - 1;
    for (row, line) in pad_box(body, body.width, 0).into_iter().enumerate() {
        let lead = if row == last { sign.clone() } else { format!("{}│", " ".repeat(sign_width - 1)) };
        lines.push(format!("{lead}{line}"));
    }
    MathBox { lines, base: body.base + 1, width: sign_width + body.width }
}

/// A base with scripts: on one line when they have Unicode forms, limits above and below a big
/// operator, otherwise raised and lowered beside the base.
/// @param base - the base node
/// @param sup - the superscript, if any
/// @param sub - the subscript, if any
fn scripts_box(base: &Node, sup: Option<&Node>, sub: Option<&Node>) -> MathBox {
    let base_box = node_box(base);
    if matches!(base, Node::Atom(_, Class::Big) | Node::Atom(_, Class::FuncLimits)) {
        let mut parts = Vec::new();
        if let Some(sup) = sup {
            parts.push(script_box(sup));
        }
        let at = parts.len();
        parts.push(base_box);
        if let Some(sub) = sub {
            parts.push(script_box(sub));
        }
        return vstack(&parts, at);
    }
    let fits = |n: Option<&Node>, table: &[(char, char)]| n.is_none_or(|n| simple_script(n, table));
    if base_box.lines.len() == 1 && fits(sup, SUPERSCRIPTS) && fits(sub, SUBSCRIPTS) {
        return text_box(&linear(&Node::Scripts { base: Box::new(base.clone()), sup: sup.map(|s| Box::new(s.clone())), sub: sub.map(|s| Box::new(s.clone())) }, false));
    }
    raised_scripts(base_box, sup.map(script_box), sub.map(script_box))
}

/// A script's box: one tight line with no spaces around operators when it fits on one line.
/// @param node - the script
fn script_box(node: &Node) -> MathBox {
    let laid = node_box(node);
    if laid.lines.len() == 1 { text_box(&linear(node, true)) } else { laid }
}

/// True when a script is one line and every character has a Unicode small form.
/// @param node - the script
/// @param table - the superscript or subscript table
fn simple_script(node: &Node, table: &[(char, char)]) -> bool {
    let text = linear(node, true);
    !matches!(node_box(node).lines.len(), 2..) && text.chars().all(|c| table.iter().any(|(from, _)| *from == c))
}

/// Scripts set beside the base: the superscript above its top row, the subscript below its bottom row.
/// @param base - the base box
/// @param sup - the superscript box, if any
/// @param sub - the subscript box, if any
fn raised_scripts(base: MathBox, sup: Option<MathBox>, sub: Option<MathBox>) -> MathBox {
    let empty = MathBox { lines: Vec::new(), base: 0, width: 0 };
    let (sup, sub) = (sup.unwrap_or(empty.clone()), sub.unwrap_or(empty));
    let width = sup.width.max(sub.width);
    let mut column: Vec<String> = sup.lines.iter().map(|l| format!("{l}{}", " ".repeat(width - width_of(l)))).collect();
    column.extend(std::iter::repeat_n(" ".repeat(width), base.lines.len()));
    column.extend(sub.lines.iter().map(|l| format!("{l}{}", " ".repeat(width - width_of(l)))));
    let side = MathBox { base: sup.lines.len() + base.base, lines: column, width };
    hcat(&[base, side])
}

/// The rows of a tall delimiter, or the delimiter itself when the content is one row.
/// @param delim - the delimiter
/// @param height - the rows it spans
fn tall_delimiter(delim: &str, height: usize) -> Vec<String> {
    if height <= 1 || delim.is_empty() {
        return vec![delim.to_string(); height.max(1)];
    }
    let (top, mid, bottom, center) = match delim {
        "(" => ('⎛', '⎜', '⎝', '⎜'),
        ")" => ('⎞', '⎟', '⎠', '⎟'),
        "[" => ('⎡', '⎢', '⎣', '⎢'),
        "]" => ('⎤', '⎥', '⎦', '⎥'),
        "{" => ('⎧', '⎪', '⎩', '⎨'),
        "}" => ('⎫', '⎪', '⎭', '⎬'),
        "|" => ('│', '│', '│', '│'),
        "‖" => ('‖', '‖', '‖', '‖'),
        other => return (0..height).map(|i| if i == height / 2 { other.to_string() } else { " ".repeat(width_of(other)) }).collect(),
    };
    (0..height).map(|i| {
        let c = if i == 0 { top } else if i + 1 == height { bottom } else if i == height / 2 { center } else { mid };
        c.to_string()
    }).collect()
}

/// Content between two delimiters as tall as the content.
/// @param left - the left delimiter, or empty
/// @param body - the content
/// @param right - the right delimiter, or empty
fn fence_box(left: &str, body: &MathBox, right: &str) -> MathBox {
    let height = body.lines.len();
    let side = |delim: &str| {
        let lines = tall_delimiter(delim, height);
        let width = lines.iter().map(|l| width_of(l)).max().unwrap_or(0);
        MathBox { lines, base: body.base, width }
    };
    let mut parts = Vec::new();
    if !left.is_empty() {
        parts.push(side(left));
    }
    parts.push(body.clone());
    if !right.is_empty() {
        parts.push(side(right));
    }
    hcat(&parts)
}

/// A matrix or aligned rows: the cells in columns two spaces apart, centered, or for aligned rows
/// right aligned then left aligned with no gap. A blank row goes between rows taller than one.
/// @param rows - the cells
/// @param align - true for aligned equations and cases
fn matrix_box(rows: &[Vec<Node>], align: bool) -> MathBox {
    let cells: Vec<Vec<MathBox>> = rows.iter().map(|r| r.iter().map(node_box).collect()).collect();
    let cols = cells.iter().map(|r| r.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols).map(|c| cells.iter().filter_map(|r| r.get(c)).map(|b| b.width).max().unwrap_or(0)).collect();
    let tall = cells.iter().any(|r| r.iter().any(|b| b.lines.len() > 1));
    let mut lines = Vec::new();
    for (index, row) in cells.iter().enumerate() {
        if index > 0 && tall {
            lines.push(String::new());
        }
        let mut parts = Vec::new();
        for (c, width) in widths.iter().enumerate() {
            let cell = row.get(c).cloned().unwrap_or_else(|| text_box(""));
            let left = match (align, c % 2) { (true, 0) if cols > 1 && !is_cases(rows) => width - cell.width, (true, _) => 0, _ => (width - cell.width) / 2 };
            let gap = if c + 1 < cols && (!align || is_cases(rows)) { 2 } else { 0 };
            parts.push(MathBox { lines: pad_box(&cell, width + gap, left), base: cell.base, width: width + gap });
        }
        lines.extend(hcat(&parts).lines);
    }
    let width = widths.iter().sum::<usize>() + if align && !is_cases(rows) { 0 } else { 2 * cols.saturating_sub(1) };
    let lines: Vec<String> = lines.into_iter().map(|l| format!("{l}{}", " ".repeat(width.saturating_sub(width_of(&l))))).collect();
    MathBox { base: lines.len().saturating_sub(1) / 2, lines, width }
}

/// True for a `cases` environment, guessed from rows of two cells.
/// @param rows - the cells
fn is_cases(rows: &[Vec<Node>]) -> bool {
    rows.iter().all(|r| r.len() <= 2) && rows.iter().any(|r| r.len() == 2 && matches!(&r[1], Node::Group(g) if g.first().is_some_and(|n| matches!(n, Node::Atom(t, _) if t.chars().all(|c| c.is_alphabetic()) && t.len() > 1))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_math_uses_unicode_scripts_and_symbols() {
        assert_eq!(inline("x^2 + y^2 = z^2"), "x² + y² = z²");
        assert_eq!(inline("a_i \\cdot b_{ij}"), "aᵢ · bᵢⱼ");
        assert_eq!(inline("\\alpha \\to \\infty"), "α → ∞");
        assert_eq!(inline("e^{i\\pi} + 1 = 0"), "e^(iπ) + 1 = 0");
        assert_eq!(inline("\\frac{a+b}{2}"), "(a + b)/2");
        assert_eq!(inline("\\sqrt{x+1}"), "√(x + 1)");
        assert_eq!(inline("\\sqrt[3]{8}"), "∛8");
        assert_eq!(inline("-x"), "−x");
        assert_eq!(inline("\\sin x"), "sin x");
        assert_eq!(inline("\\mathbb{R}^n"), "ℝⁿ");
        assert_eq!(inline("f'(x)"), "f′(x)");
        assert_eq!(inline("\\text{if } x > 0"), "if x > 0");
    }

    #[test]
    fn display_math_stacks_fractions_and_limits() {
        let rows = display("\\frac{a+b}{c}", 40).unwrap();
        let trimmed: Vec<&str> = rows.iter().map(|r| r.trim()).collect();
        assert_eq!(trimmed, vec!["a + b", "───────", "c"]);
        let sum = display("\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}", 60).unwrap();
        assert_eq!(sum.len(), 3, "{sum:#?}");
        assert!(sum[0].contains('n') && sum[1].contains('∑') && sum[2].contains("i=1"), "{sum:#?}");
        assert!(sum[1].contains("────"), "{sum:#?}");
    }

    #[test]
    fn display_math_draws_matrices_in_tall_brackets() {
        let rows = display("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}", 40).unwrap();
        let trimmed: Vec<&str> = rows.iter().map(|r| r.trim()).collect();
        assert_eq!(trimmed, vec!["⎛1  2⎞", "⎝3  4⎠"]);
        let cases = display("f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases}", 60).unwrap();
        assert!(cases.iter().any(|r| r.contains('⎧')) || cases.iter().any(|r| r.contains('{')), "{cases:#?}");
    }

    #[test]
    fn a_root_gets_a_bar_and_a_wide_formula_does_not_fit() {
        let rows = display("\\sqrt{\\frac{1}{2}}", 40).unwrap();
        assert!(rows[0].contains('─') && rows.last().unwrap().contains('√'), "{rows:#?}");
        assert!(display(&"x + ".repeat(30), 20).is_none());
    }

    #[test]
    fn delimiters_outside_code_become_dollars() {
        let text = "Inline \\(x^2\\) and\n\\[\ny = 1\n\\]\n`\\(code\\)`\n```\n\\[not\\]\n```\n";
        let out = convert_delimiters(text);
        assert!(out.contains("Inline $x^2$ and"), "{out}");
        assert!(out.contains("$$\ny = 1\n$$"), "{out}");
        assert!(out.contains("`\\(code\\)`") && out.contains("\\[not\\]"), "{out}");
    }
}
