//! Flowcharts (`graph` and `flowchart`): parses the nodes and links, lays them out in layers, and
//! draws boxes joined by lines with arrows. Layers run down for TD and TB, up for BT, right for LR
//! and left for RL. The layout is the layered method: break cycles, give each node the layer of its
//! longest path from a source, put a placeholder in each layer a long link crosses, order each layer
//! by the average position of its neighbors, then draw each link in the channel between two layers.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use super::canvas::{draw_box, Canvas, Corners, Stroke};
use super::label_lines;
use crate::utils::screen::char_width;

/// The way the layers run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Down,
    Up,
    Right,
    Left,
}

/// The outline of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Rect,
    Round,
    Decision,
}

/// What a link has at one end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Head {
    None,
    Arrow,
    Circle,
    Cross,
}

/// A node as written.
#[derive(Debug, Clone)]
struct Node {
    label: Vec<String>,
    shape: Shape,
}

/// A link between two nodes as written.
#[derive(Debug, Clone)]
struct Edge {
    from: usize,
    to: usize,
    label: String,
    head_from: Head,
    head_to: Head,
    stroke: Stroke,
}

/// The parsed chart.
#[derive(Debug, Default)]
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    index: HashMap<String, usize>,
}

/// A link's operator, before the nodes on its two sides are known.
struct Link {
    label: String,
    head_from: Head,
    head_to: Head,
    stroke: Stroke,
    hidden: bool,
}

/// A node in the layout: a real node, or a placeholder where a long link crosses a layer.
#[derive(Debug, Clone)]
struct Slot {
    real: Option<usize>,
    layer: usize,
    /// The size along the way the layers run.
    main: usize,
    /// The size across it.
    cross: usize,
    /// The position across, from the left or the top.
    pos: usize,
    stroke: Stroke,
    /// The node's label lines, empty for a placeholder.
    label: Vec<String>,
    shape: Shape,
}

/// The part of a link between two neighboring layers.
#[derive(Debug, Clone)]
struct Segment {
    top: usize,
    bottom: usize,
    head_top: Head,
    head_bottom: Head,
    label: String,
    stroke: Stroke,
    /// Columns or rows to move the ends off the node centers, so a second link between the same
    /// two nodes does not draw over the first.
    shift: usize,
}

/// The finished layout.
struct Layout {
    slots: Vec<Slot>,
    layers: Vec<Vec<usize>>,
    segments: Vec<Segment>,
}

/// Draws a flowchart, trying top down when a left to right chart is wider than `width`.
/// @param header - the first line, such as `graph LR`
/// @param body - the other lines
/// @param width - the columns available
pub fn render(header: &str, body: &[String], width: usize) -> Option<Vec<String>> {
    let dir = direction(header);
    let graph = parse(body)?;
    let rows = draw(&graph, dir);
    let wide = rows.iter().any(|r| r.chars().map(char_width).sum::<usize>() > width);
    if wide && matches!(dir, Dir::Right | Dir::Left) {
        return Some(draw(&graph, Dir::Down));
    }
    Some(rows)
}

/// The direction named on the first line. Top down when none is named.
/// @param header - the first line
fn direction(header: &str) -> Dir {
    match header.split_whitespace().nth(1).map(|d| d.to_ascii_uppercase()).as_deref() {
        Some("BT") => Dir::Up,
        Some("LR") => Dir::Right,
        Some("RL") => Dir::Left,
        _ => Dir::Down,
    }
}

/// Parses the statements of a chart. Returns None when no node was found.
/// @param body - the lines after the first
fn parse(body: &[String]) -> Option<Graph> {
    let mut graph = Graph::default();
    for line in body {
        for statement in split_statements(line) {
            let statement = statement.trim();
            if !statement.is_empty() && !skipped(statement) {
                parse_statement(statement, &mut graph);
            }
        }
    }
    (!graph.nodes.is_empty()).then_some(graph)
}

/// Splits a line at `;` outside quotes.
/// @param line - the line
fn split_statements(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut start, mut quoted) = (0, false);
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parts.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&line[start..]);
    parts
}

/// True for statements that only style or group nodes, which are not drawn.
/// @param statement - the statement
fn skipped(statement: &str) -> bool {
    let word = statement.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    matches!(word.as_str(), "subgraph" | "end" | "style" | "classdef" | "class" | "linkstyle" | "click" | "direction" | "accTitle" | "accdescr")
}

/// Parses one statement: a chain of node groups joined by links. A statement that does not parse
/// keeps what was read before the error.
/// @param statement - the statement
/// @param graph - the chart, changed in place
fn parse_statement(statement: &str, graph: &mut Graph) -> Option<()> {
    let mut rest = statement;
    let mut left = parse_group(&mut rest, graph)?;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return Some(());
        }
        let link = parse_link(&mut rest)?;
        let right = parse_group(&mut rest, graph)?;
        if !link.hidden {
            for &from in &left {
                for &to in &right {
                    graph.edges.push(Edge { from, to, label: link.label.clone(), head_from: link.head_from, head_to: link.head_to, stroke: link.stroke });
                }
            }
        }
        left = right;
    }
}

/// Parses nodes joined by `&`.
/// @param rest - the text left, moved past what was read
/// @param graph - the chart, changed in place
fn parse_group(rest: &mut &str, graph: &mut Graph) -> Option<Vec<usize>> {
    let mut ids = vec![parse_node(rest, graph)?];
    while let Some(after) = rest.trim_start().strip_prefix('&') {
        *rest = after;
        ids.push(parse_node(rest, graph)?);
    }
    Some(ids)
}

/// Parses a node reference: an id with an optional shape and label, and an optional `:::class`.
/// @param rest - the text left, moved past what was read
/// @param graph - the chart, changed in place
fn parse_node(rest: &mut &str, graph: &mut Graph) -> Option<usize> {
    let text = rest.trim_start();
    let end = text.char_indices().find(|(_, c)| !(c.is_alphanumeric() || *c == '_')).map_or(text.len(), |(i, _)| i);
    if end == 0 {
        return None;
    }
    let id = &text[..end];
    let mut after = &text[end..];
    let shaped = parse_shape(&mut after);
    if let Some(class) = after.strip_prefix(":::") {
        let n = class.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).unwrap_or(class.len());
        after = &class[n..];
    }
    *rest = after;
    Some(add_node(graph, id, shaped))
}

/// Adds a node, or updates the label and shape of a node already added.
/// @param graph - the chart
/// @param id - the node id
/// @param shaped - the label and shape when this reference gave them
fn add_node(graph: &mut Graph, id: &str, shaped: Option<(String, Shape)>) -> usize {
    if let Some(&index) = graph.index.get(id) {
        if let Some((label, shape)) = shaped {
            graph.nodes[index] = Node { label: label_lines(&label), shape };
        }
        return index;
    }
    let (label, shape) = shaped.unwrap_or_else(|| (id.to_string(), Shape::Rect));
    graph.nodes.push(Node { label: label_lines(&label), shape });
    graph.index.insert(id.to_string(), graph.nodes.len() - 1);
    graph.nodes.len() - 1
}

/// Reads a shape and its label after a node id, such as `[Text]`, `(Text)` or `{Choice}`.
/// @param rest - the text after the id, moved past the shape
fn parse_shape(rest: &mut &str) -> Option<(String, Shape)> {
    const SHAPES: [(&str, &str, Shape); 14] = [
        ("(((", ")))", Shape::Round), ("((", "))", Shape::Round), ("([", "])", Shape::Round), ("[[", "]]", Shape::Rect),
        ("[(", ")]", Shape::Round), ("[/", "/]", Shape::Rect), ("[/", "\\]", Shape::Rect), ("[\\", "\\]", Shape::Rect),
        ("[\\", "/]", Shape::Rect), ("{{", "}}", Shape::Decision), ("{", "}", Shape::Decision), ("[", "]", Shape::Rect),
        ("(", ")", Shape::Round), (">", "]", Shape::Rect),
    ];
    for (open, close, shape) in SHAPES {
        let Some(inner) = rest.strip_prefix(open) else { continue };
        let (label, after) = match inner.trim_start().strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.find('"')?;
                (&quoted[..end], quoted[end + 1..].trim_start())
            }
            None => (inner, inner),
        };
        let Some(at) = after.find(close) else { continue };
        let label = if label.as_ptr() == inner.as_ptr() { &inner[..at] } else { label };
        *rest = &after[at + close.len()..];
        return Some((label.trim().to_string(), shape));
    }
    None
}

/// The link operator without a label in the middle, such as `-->`, `-.->`, `==>` or `---`.
fn plain_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?:<|o|x)?(?:-{2,}>|-{3,}|={2,}>|={3,}|-\.+->|-\.+-|-{2,}[ox]\b|={2,}[ox]\b|~{3,})").expect("valid regex"))
}

/// The link operator with a label in the middle, such as `-- yes -->`.
fn labeled_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(<|o|x)?(--|==|-\.)\s*([^|>]+?)\s*(-{2,}>|-{3,}|={2,}>|={3,}|\.-+>|\.-+|-{2,}[ox]\b|={2,}[ox]\b)").expect("valid regex"))
}

/// Parses a link operator and its label.
/// @param rest - the text left, moved past the link
fn parse_link(rest: &mut &str) -> Option<Link> {
    let text = rest.trim_start();
    if let Some(found) = plain_link().find(text) {
        let op = found.as_str();
        let mut after = &text[found.end()..];
        let mut label = String::new();
        if let Some(inner) = after.trim_start().strip_prefix('|') {
            if let Some(end) = inner.find('|') {
                label = inner[..end].to_string();
                after = &inner[end + 1..];
            }
        }
        *rest = after;
        return Some(link_from(op, &label));
    }
    let caps = labeled_link().captures(text)?;
    let op = format!("{}{}{}", caps.get(1).map_or("", |m| m.as_str()), &caps[2], &caps[4]);
    *rest = &text[caps.get(0)?.end()..];
    Some(link_from(&op, &caps[3]))
}

/// The heads, stroke and label of a link from its operator.
/// @param op - the operator text
/// @param label - the label text
fn link_from(op: &str, label: &str) -> Link {
    let head = |c: Option<char>| match c {
        Some('>') | Some('<') => Head::Arrow,
        Some('o') => Head::Circle,
        Some('x') => Head::Cross,
        _ => Head::None,
    };
    let stroke = if op.contains('=') { Stroke::Thick } else if op.contains('.') { Stroke::Dotted } else { Stroke::Solid };
    Link { label: label_lines(label).join(" "), head_from: head(op.chars().next()), head_to: head(op.chars().last()), stroke, hidden: op.contains('~') }
}

/// Draws the chart in a direction.
/// @param graph - the chart
/// @param dir - the direction the layers run
fn draw(graph: &Graph, dir: Dir) -> Vec<String> {
    let mut layout = build_layout(graph, dir);
    order_layers(&mut layout);
    let horizontal = matches!(dir, Dir::Right | Dir::Left);
    place_across(&mut layout, if horizontal { 1 } else { 3 });
    paint(&layout, horizontal).rows()
}

/// Finds the links that point back along a depth first search, which are reversed to break cycles.
/// @param count - the number of nodes
/// @param edges - the links
fn back_edges(count: usize, edges: &[Edge]) -> Vec<bool> {
    let mut adjacent = vec![Vec::new(); count];
    for (index, edge) in edges.iter().enumerate() {
        adjacent[edge.from].push(index);
    }
    let (mut state, mut back) = (vec![0u8; count], vec![false; edges.len()]);
    for start in 0..count {
        if state[start] != 0 {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        state[start] = 1;
        while let Some((node, next)) = stack.pop() {
            let Some(&edge) = adjacent[node].get(next) else {
                state[node] = 2;
                continue;
            };
            stack.push((node, next + 1));
            let to = edges[edge].to;
            match state[to] {
                0 => {
                    state[to] = 1;
                    stack.push((to, 0));
                }
                1 => back[edge] = true,
                _ => {}
            }
        }
    }
    back
}

/// The layer of each node: the length of its longest path from a source, reversed for BT and RL.
/// @param graph - the chart
/// @param dir - the direction
fn layers_of(graph: &Graph, dir: Dir) -> Vec<usize> {
    let back = back_edges(graph.nodes.len(), &graph.edges);
    let links: Vec<(usize, usize)> = graph.edges.iter().zip(&back).filter(|(e, _)| e.from != e.to).map(|(e, b)| if *b { (e.to, e.from) } else { (e.from, e.to) }).collect();
    let mut incoming = vec![0usize; graph.nodes.len()];
    for &(_, to) in &links {
        incoming[to] += 1;
    }
    let mut layer = vec![0usize; graph.nodes.len()];
    let mut ready: Vec<usize> = (0..graph.nodes.len()).filter(|&n| incoming[n] == 0).rev().collect();
    while let Some(node) = ready.pop() {
        for &(from, to) in links.iter().filter(|(f, _)| *f == node) {
            layer[to] = layer[to].max(layer[from] + 1);
            incoming[to] -= 1;
            if incoming[to] == 0 {
                ready.push(to);
            }
        }
    }
    if matches!(dir, Dir::Up | Dir::Left) {
        let top = layer.iter().copied().max().unwrap_or(0);
        layer.iter_mut().for_each(|l| *l = top - *l);
    }
    layer
}

/// Builds the slots and segments: one slot per node, a placeholder slot in each layer a long link
/// crosses, and one segment per pair of neighboring layers a link spans.
/// @param graph - the chart
/// @param dir - the direction
fn build_layout(graph: &Graph, dir: Dir) -> Layout {
    let layer = layers_of(graph, dir);
    let horizontal = matches!(dir, Dir::Right | Dir::Left);
    let mut slots: Vec<Slot> = graph.nodes.iter().enumerate().map(|(i, node)| {
        let w = node.label.iter().map(|l| l.chars().map(char_width).sum::<usize>()).max().unwrap_or(0) + 4;
        let h = node.label.len() + 2;
        let (main, cross) = if horizontal { (w, h) } else { (h, w) };
        Slot { real: Some(i), layer: layer[i], main, cross, pos: 0, stroke: Stroke::Solid, label: node.label.clone(), shape: node.shape }
    }).collect();
    let mut segments = Vec::new();
    for edge in graph.edges.iter().filter(|e| e.from != e.to) {
        let down = layer[edge.from] < layer[edge.to];
        let (top, bottom, head_top, head_bottom) = if down { (edge.from, edge.to, edge.head_from, edge.head_to) } else { (edge.to, edge.from, edge.head_to, edge.head_from) };
        let mut previous = top;
        for crossing in layer[top] + 1..layer[bottom] {
            slots.push(Slot { real: None, layer: crossing, main: 1, cross: 1, pos: 0, stroke: edge.stroke, label: Vec::new(), shape: Shape::Rect });
            let first = previous == top;
            segments.push(Segment { top: previous, bottom: slots.len() - 1, head_top: if first { head_top } else { Head::None }, head_bottom: Head::None, label: String::new(), stroke: edge.stroke, shift: 0 });
            previous = slots.len() - 1;
        }
        let first = previous == top;
        let shift = 2 * segments.iter().filter(|s: &&Segment| s.top == previous && s.bottom == bottom).count();
        segments.push(Segment { top: previous, bottom, head_top: if first { head_top } else { Head::None }, head_bottom, label: edge.label.clone(), stroke: edge.stroke, shift });
    }
    let count = slots.iter().map(|s| s.layer + 1).max().unwrap_or(0);
    let mut layers = vec![Vec::new(); count];
    for (index, slot) in slots.iter().enumerate() {
        layers[slot.layer].push(index);
    }
    Layout { slots, layers, segments }
}

/// The slots joined to a slot in the layer above (`up`) or below.
/// @param layout - the layout
/// @param slot - the slot
/// @param up - true for the layer above
fn neighbors(layout: &Layout, slot: usize, up: bool) -> Vec<usize> {
    layout.segments.iter().filter_map(|s| match up {
        true if s.bottom == slot => Some(s.top),
        false if s.top == slot => Some(s.bottom),
        _ => None,
    }).collect()
}

/// Orders each layer by the average position of each slot's neighbors, four times down and up.
/// @param layout - the layout, changed in place
fn order_layers(layout: &mut Layout) {
    for _ in 0..4 {
        for index in 1..layout.layers.len() {
            sort_layer(layout, index, true);
        }
        for index in (0..layout.layers.len().saturating_sub(1)).rev() {
            sort_layer(layout, index, false);
        }
    }
}

/// Sorts one layer by the average position of each slot's neighbors in the layer above or below.
/// A slot with no neighbors there keeps its place.
/// @param layout - the layout, changed in place
/// @param index - the layer
/// @param up - true to look at the layer above
fn sort_layer(layout: &mut Layout, index: usize, up: bool) {
    let other = if up { index.checked_sub(1) } else { Some(index + 1) };
    let Some(other) = other.filter(|o| *o < layout.layers.len()) else { return };
    let position: HashMap<usize, usize> = layout.layers[other].iter().enumerate().map(|(p, s)| (*s, p)).collect();
    let mut keyed: Vec<(f64, usize)> = layout.layers[index].iter().enumerate().map(|(p, &slot)| {
        let near: Vec<usize> = neighbors(layout, slot, up).into_iter().filter_map(|n| position.get(&n).copied()).collect();
        let key = if near.is_empty() { p as f64 } else { near.iter().sum::<usize>() as f64 / near.len() as f64 };
        (key, slot)
    }).collect();
    keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    layout.layers[index] = keyed.into_iter().map(|(_, s)| s).collect();
}

/// Gives each slot its position across the layers: each layer is placed under the centers of its
/// neighbors above, then each layer is moved toward the centers of its neighbors below.
/// @param layout - the layout, changed in place
/// @param gap - the space between two slots of a layer
fn place_across(layout: &mut Layout, gap: usize) {
    for index in 0..layout.layers.len() {
        place_layer(layout, index, gap, true);
    }
    for index in (0..layout.layers.len().saturating_sub(1)).rev() {
        place_layer(layout, index, gap, false);
    }
    let least = layout.slots.iter().map(|s| s.pos).min().unwrap_or(0);
    layout.slots.iter_mut().for_each(|s| s.pos -= least);
}

/// Places one layer's slots left to right, each as close to the center of its neighbors as the
/// slots before it allow. On the way up a slot is never moved left of where it is.
/// @param layout - the layout, changed in place
/// @param index - the layer
/// @param gap - the space between two slots
/// @param up - true to follow the layer above, false the layer below
fn place_layer(layout: &mut Layout, index: usize, gap: usize, up: bool) {
    let mut cursor = 0usize;
    for slot in layout.layers[index].clone() {
        let near = neighbors(layout, slot, up);
        let cross = layout.slots[slot].cross;
        let wanted = if near.is_empty() {
            if up { cursor } else { layout.slots[slot].pos }
        } else {
            let center = near.iter().map(|&n| layout.slots[n].pos + layout.slots[n].cross / 2).sum::<usize>() / near.len();
            center.saturating_sub(cross / 2)
        };
        let floor = if up { cursor } else { cursor.max(layout.slots[slot].pos) };
        let pos = wanted.max(floor);
        layout.slots[slot].pos = pos;
        cursor = pos + cross + gap;
    }
}

/// The center of a slot across the layers.
/// @param slot - the slot
fn center(slot: &Slot) -> usize {
    slot.pos + slot.cross / 2
}

/// Where a segment meets its upper and lower slots across the layers: the centers, moved by the
/// segment's shift while it stays inside the box.
/// @param layout - the layout
/// @param seg - the segment
fn ends(layout: &Layout, seg: &Segment) -> (usize, usize) {
    let at = |slot: &Slot| {
        let moved = center(slot) + seg.shift;
        if slot.real.is_some() && moved + 1 < slot.pos + slot.cross { moved } else { center(slot) }
    };
    (at(&layout.slots[seg.top]), at(&layout.slots[seg.bottom]))
}

/// The rows of tracks, the rows of labels and the run of the last part of a channel's links.
struct Channel {
    tracks: Vec<Option<usize>>,
    label_rows: Vec<usize>,
    rows: usize,
    run: usize,
    size: usize,
}

/// Works out a channel: each bent segment gets a track, shared with segments from the same slot or
/// to the same slot, and in a top down chart each label gets a row where it does not overlap another.
/// @param layout - the layout
/// @param segments - the indexes of the segments in this channel
/// @param horizontal - true for LR and RL
fn plan_channel(layout: &Layout, segments: &[usize], horizontal: bool) -> Channel {
    let mut tracks = vec![None; layout.segments.len()];
    let mut used: Vec<Vec<(usize, usize, usize, usize)>> = Vec::new();
    for &index in segments {
        let seg = &layout.segments[index];
        let (a, b) = ends(layout, seg);
        if a == b {
            continue;
        }
        let (lo, hi) = (a.min(b), a.max(b));
        let free = |t: &Vec<(usize, usize, usize, usize)>| t.iter().all(|&(l, h, ta, tb)| hi + 1 < l || h + 1 < lo || ta == a || tb == b);
        let track = match used.iter().position(free) {
            Some(t) => t,
            None => {
                used.push(Vec::new());
                used.len() - 1
            }
        };
        used[track].push((lo, hi, a, b));
        tracks[index] = Some(track);
    }
    let (label_rows, rows, longest) = plan_labels(layout, segments, horizontal);
    let run = if horizontal { if longest > 0 { longest + 3 } else { 3 } } else { 0 };
    let size = if horizontal { 1 + used.len() + run } else { 2 + used.len() + rows };
    Channel { tracks, label_rows, rows: used.len(), run, size }
}

/// The label row of each labeled segment in a top down chart, the number of label rows, and the
/// longest label.
/// @param layout - the layout
/// @param segments - the indexes of the segments in this channel
/// @param horizontal - true for LR and RL, where labels sit on the line
fn plan_labels(layout: &Layout, segments: &[usize], horizontal: bool) -> (Vec<usize>, usize, usize) {
    let mut rows = vec![0usize; layout.segments.len()];
    let mut taken: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut longest = 0;
    for &index in segments {
        let seg = &layout.segments[index];
        let width: usize = seg.label.chars().map(char_width).sum();
        if width == 0 {
            continue;
        }
        longest = longest.max(width);
        if horizontal {
            continue;
        }
        let start = ends(layout, seg).1.saturating_sub(width / 2);
        let row = match taken.iter().position(|t| t.iter().all(|&(s, e)| start + width < s || e + 1 < start)) {
            Some(r) => r,
            None => {
                taken.push(Vec::new());
                taken.len() - 1
            }
        };
        taken[row].push((start, start + width));
        rows[index] = row;
    }
    (rows, taken.len(), longest)
}

/// Where each layer and each channel starts along the way the layers run, and each layer's size.
/// @param layout - the layout
/// @param channels - the planned channels, one per pair of neighboring layers
fn layer_starts(layout: &Layout, channels: &[Channel]) -> Vec<(usize, usize)> {
    let mut at = 0;
    let mut starts = Vec::new();
    for (index, layer) in layout.layers.iter().enumerate() {
        let size = layer.iter().filter(|&&s| layout.slots[s].real.is_some()).map(|&s| layout.slots[s].main).max().unwrap_or(1);
        starts.push((at, size));
        at += size + channels.get(index).map_or(0, |c| c.size);
    }
    starts
}

/// The drawing helpers that turn a position along and across the layers into a cell.
struct Pen<'a> {
    canvas: &'a mut Canvas,
    horizontal: bool,
}

impl Pen<'_> {
    /// The cell at a position along (`main`) and across (`cross`) the layers.
    /// @param main - the position along
    /// @param cross - the position across
    fn cell(&self, main: usize, cross: usize) -> (usize, usize) {
        if self.horizontal { (main, cross) } else { (cross, main) }
    }

    /// Draws a line along the layers.
    /// @param cross - the position across
    /// @param a - one end along
    /// @param b - the other end along
    /// @param stroke - how the line is drawn
    fn along(&mut self, cross: usize, a: usize, b: usize, stroke: Stroke) {
        if self.horizontal { self.canvas.hline(cross, a, b, stroke) } else { self.canvas.vline(cross, a, b, stroke) }
    }

    /// Draws a line across the layers.
    /// @param main - the position along
    /// @param a - one end across
    /// @param b - the other end across
    /// @param stroke - how the line is drawn
    fn across(&mut self, main: usize, a: usize, b: usize, stroke: Stroke) {
        if self.horizontal { self.canvas.vline(main, a, b, stroke) } else { self.canvas.hline(main, a, b, stroke) }
    }

    /// Puts a character at a position.
    /// @param main - the position along
    /// @param cross - the position across
    /// @param c - the character
    fn put(&mut self, main: usize, cross: usize, c: char) {
        let (x, y) = self.cell(main, cross);
        self.canvas.put(x, y, c);
    }
}

/// Draws the layout: the nodes, the placeholders and every segment.
/// @param layout - the layout
/// @param horizontal - true for LR and RL
fn paint(layout: &Layout, horizontal: bool) -> Canvas {
    let channels: Vec<Channel> = (0..layout.layers.len().saturating_sub(1)).map(|l| {
        let segs: Vec<usize> = (0..layout.segments.len()).filter(|&i| layout.slots[layout.segments[i].top].layer == l).collect();
        plan_channel(layout, &segs, horizontal)
    }).collect();
    let starts = layer_starts(layout, &channels);
    let mut canvas = Canvas::default();
    let mut pen = Pen { canvas: &mut canvas, horizontal };
    for slot in &layout.slots {
        let (start, size) = starts[slot.layer];
        paint_slot(&mut pen, slot, start, size);
    }
    for (index, seg) in layout.segments.iter().enumerate() {
        let layer = layout.slots[seg.top].layer;
        let (start, size) = starts[layer];
        paint_segment(&mut pen, layout, index, &channels[layer], start + size);
    }
    canvas
}

/// Draws a node's box, or a placeholder's line through its layer.
/// @param pen - the drawing helpers
/// @param slot - the slot
/// @param start - where its layer starts along
/// @param size - its layer's size along
fn paint_slot(pen: &mut Pen, slot: &Slot, start: usize, size: usize) {
    let Some(_) = slot.real else {
        pen.along(center(slot), start, start + size - 1, slot.stroke);
        return;
    };
    let corners: Corners = match slot.shape {
        Shape::Rect => ('┌', '┐', '└', '┘'),
        Shape::Round => ('╭', '╮', '╰', '╯'),
        Shape::Decision => ('╱', '╲', '╲', '╱'),
    };
    let (x, y) = pen.cell(start, slot.pos);
    let (w, h) = if pen.horizontal { (size, slot.cross) } else { (slot.cross, size) };
    draw_box(pen.canvas, (x, y), (w, h), corners, &slot.label);
}

/// Draws one segment in its channel: down from the upper slot, along its track when the two centers
/// differ, down to the lower slot, with its heads, its label and a tee where it leaves a box.
/// @param pen - the drawing helpers
/// @param layout - the layout
/// @param index - the segment
/// @param channel - the segment's channel
/// @param top - where the channel starts along
fn paint_segment(pen: &mut Pen, layout: &Layout, index: usize, channel: &Channel, top: usize) {
    let seg = &layout.segments[index];
    let (upper, lower) = (&layout.slots[seg.top], &layout.slots[seg.bottom]);
    let (a, b) = ends(layout, seg);
    let bottom = top + channel.size - 1;
    match channel.tracks[index] {
        Some(track) => {
            let turn = top + 1 + track;
            pen.along(a, top, turn, seg.stroke);
            pen.across(turn, a, b, seg.stroke);
            pen.along(b, turn, bottom, seg.stroke);
        }
        None => pen.along(a, top, bottom, seg.stroke),
    }
    paint_heads(pen, seg, (upper.real.is_some(), lower.real.is_some()), (a, b), (top, bottom));
    paint_label(pen, seg, channel, b, top + 1 + channel.rows + channel.label_rows[index], bottom);
}

/// Draws a segment's heads, or a tee on the box border where it leaves a box without a head.
/// @param pen - the drawing helpers
/// @param seg - the segment
/// @param real - whether the upper and lower slots are nodes
/// @param centers - the centers of the upper and lower slots
/// @param ends - the first and last positions of the channel along
fn paint_heads(pen: &mut Pen, seg: &Segment, real: (bool, bool), centers: (usize, usize), ends: (usize, usize)) {
    let (down, up, tee_top, tee_bottom) = if pen.horizontal { ('▶', '◀', '├', '┤') } else { ('▼', '▲', '┬', '┴') };
    let mark = |head: Head, arrow: char| match head {
        Head::Arrow => Some(arrow),
        Head::Circle => Some('o'),
        Head::Cross => Some('x'),
        Head::None => None,
    };
    if real.0 {
        match mark(seg.head_top, up) {
            Some(c) => pen.put(ends.0, centers.0, c),
            None if ends.0 > 0 => pen.put(ends.0 - 1, centers.0, tee_top),
            None => {}
        }
    }
    if real.1 {
        match mark(seg.head_bottom, down) {
            Some(c) => pen.put(ends.1, centers.1, c),
            None => pen.put(ends.1 + 1, centers.1, tee_bottom),
        }
    }
}

/// Writes a segment's label: on its own row across the line in a top down chart, or on the last
/// run of the line in a left to right chart.
/// @param pen - the drawing helpers
/// @param seg - the segment
/// @param channel - the channel
/// @param b - the center of the lower slot
/// @param row - the label row along, for a top down chart
/// @param bottom - the last position of the channel along
fn paint_label(pen: &mut Pen, seg: &Segment, channel: &Channel, b: usize, row: usize, bottom: usize) {
    if seg.label.is_empty() {
        return;
    }
    let width: usize = seg.label.chars().map(char_width).sum();
    let (main, cross) = if pen.horizontal { (bottom + 1 - channel.run + 1, b) } else { (row, b.saturating_sub(width / 2)) };
    let (x, y) = pen.cell(main, cross);
    pen.canvas.text(x, y, &seg.label);
}
