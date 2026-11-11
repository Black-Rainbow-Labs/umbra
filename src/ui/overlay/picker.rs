//! A list to choose from, drawn above the status bar: a title row, then one or two rows per item.
//! Up and Down move, Enter chooses, typing filters, Esc closes. An item can ask for text instead,
//! such as the model picker's `Other...`, which opens a field for a `provider/model` string.

use super::{Outcome, Overlay, Placement};
use crate::ui::block::{paint, LayoutCtx, Row, Target};
use crate::utils::ansi;
use crate::utils::keys::{Key, Mouse};

/// One item of a picker.
#[derive(Debug, Clone, Default)]
pub struct PickItem {
    pub label: String,
    /// Shown after the label, dim, right aligned.
    pub detail: String,
    /// A second row under the label, dim, such as a session's summary.
    pub summary: Option<String>,
    /// The current choice, drawn with a filled marker.
    pub current: bool,
    /// Choosing it opens a text field, whose text is passed to the choose function.
    pub asks_text: Option<String>,
    /// What the choose function gets.
    pub value: String,
}

/// What happens when an item is chosen: the item and the text typed for it. An error is shown in
/// the picker, which stays open.
pub type Choose = Box<dyn FnMut(&PickItem, Option<&str>) -> Result<Outcome, String> + Send>;

/// A picker.
pub struct Picker {
    title: String,
    hint: String,
    items: Vec<PickItem>,
    filter: String,
    selected: usize,
    error: Option<String>,
    /// The text typed into an item's field, while the field is open.
    entering: Option<String>,
    on_choose: Choose,
    /// Draw ○ and ● markers, as for a choice with one current value.
    radio: bool,
}

impl Picker {
    /// Builds a picker with the current item selected.
    /// @param title - the title row's text
    /// @param items - the items
    /// @param radio - draw ○ and ● markers
    /// @param on_choose - what choosing an item does
    pub fn new(title: &str, items: Vec<PickItem>, radio: bool, on_choose: Choose) -> Self {
        let selected = items.iter().position(|i| i.current).unwrap_or(0);
        Picker { title: title.to_string(), hint: "↑↓ move · Enter choose · type to filter · Esc close".into(), items, filter: String::new(), selected, error: None, entering: None, on_choose, radio }
    }

    /// Sets the hint shown at the right of the title row.
    /// @param hint - the keys
    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hint = hint.to_string();
        self
    }

    /// The items that match the filter, as positions in `items`. Matching is by prefix first,
    /// then by substring, ignoring case.
    fn visible(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        if needle.is_empty() {
            return (0..self.items.len()).collect();
        }
        let text = |i: usize| format!("{} {}", self.items[i].label, self.items[i].detail).to_lowercase();
        let mut found: Vec<usize> = (0..self.items.len()).filter(|&i| self.items[i].label.to_lowercase().starts_with(&needle)).collect();
        let more: Vec<usize> = (0..self.items.len()).filter(|&i| !found.contains(&i) && text(i).contains(&needle)).collect();
        found.extend(more);
        found
    }

    /// Chooses the selected item, or opens its text field.
    fn choose(&mut self) -> Outcome {
        let visible = self.visible();
        let Some(&index) = visible.get(self.selected) else { return Outcome::Keep };
        let item = self.items[index].clone();
        if item.asks_text.is_some() && self.entering.is_none() {
            self.entering = Some(String::new());
            return Outcome::Keep;
        }
        let typed = self.entering.clone();
        match (self.on_choose)(&item, typed.as_deref()) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.error = Some(error);
                Outcome::Keep
            }
        }
    }

    /// Handles a key while the text field is open.
    /// @param key - the key
    fn field_key(&mut self, key: &Key) -> Outcome {
        let field = self.entering.as_mut().expect("the field is open");
        match key {
            Key::Text(text) | Key::Paste(text) => field.push_str(text.trim_end_matches(['\r', '\n'])),
            Key::Backspace => {
                field.pop();
            }
            Key::Enter => return self.choose(),
            Key::Escape => self.entering = None,
            _ => {}
        }
        Outcome::Keep
    }

    /// The row of one item, with the selection marker, the radio marker, the label and the detail.
    /// @param item - the item
    /// @param selected - true when it is the selected item
    /// @param width - the columns available
    /// @param ctx - the frame
    fn item_row(&self, item: &PickItem, selected: bool, width: usize, ctx: &LayoutCtx) -> String {
        let marker = if !self.radio { "" } else if item.current { "● " } else { "○ " };
        let pointer = if selected { "›" } else { " " };
        let left = format!(" {pointer} {marker}{}", item.label);
        let detail = crate::ui::block::cut_to(&item.detail, width.saturating_sub(ansi::plain(&left).chars().count() + 4));
        let pad = width.saturating_sub(crate::utils::screen::visible_width(&left) + crate::utils::screen::visible_width(&detail) + 1);
        let text = format!("{left}{}{}", " ".repeat(pad.max(1)), paint(ctx, "90", &detail));
        let text = ansi::fit(&text, width);
        if selected { paint(ctx, "1", &text) } else { text }
    }
}

impl Overlay for Picker {
    /// Pickers are drawn above the status bar.
    fn placement(&self) -> Placement {
        Placement::AboveStatus
    }

    /// The title row, the items around the selection that fit, the text field and the error.
    /// @param width - the columns available
    /// @param height - the most rows it may take
    /// @param ctx - the frame
    fn rows(&mut self, width: usize, height: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let visible = self.visible();
        self.selected = self.selected.min(visible.len().saturating_sub(1));
        let filter = if self.filter.is_empty() { String::new() } else { format!(" · filter: {}", self.filter) };
        let title = format!(" {}{filter}", self.title);
        let pad = width.saturating_sub(title.chars().count() + self.hint.chars().count() + 1);
        let mut rows = vec![Row::text(ansi::fit(&format!("{}{}{}", paint(ctx, "1", &title), " ".repeat(pad.max(1)), paint(ctx, "90", &self.hint)), width))];
        let per_item = if self.items.iter().any(|i| i.summary.is_some()) { 2 } else { 1 };
        let extra = usize::from(self.entering.is_some()) + usize::from(self.error.is_some());
        let room = (height.saturating_sub(1 + extra) / per_item).max(1);
        let top = self.selected.saturating_sub(room.saturating_sub(1));
        for (position, &index) in visible.iter().enumerate().skip(top).take(room) {
            let item = &self.items[index];
            rows.push(Row { text: self.item_row(item, position == self.selected, width, ctx), target: Some(Target::Item(position)), ..Default::default() });
            if let Some(summary) = &item.summary {
                let line = crate::ui::block::cut_to(summary, width.saturating_sub(6));
                rows.push(Row { text: paint(ctx, "90", &format!("     {line}")), target: Some(Target::Item(position)), ..Default::default() });
            }
        }
        if visible.is_empty() {
            rows.push(Row::text(paint(ctx, "90", "   nothing matches")));
        }
        if let (Some(field), Some(&index)) = (&self.entering, visible.get(self.selected)) {
            let label = self.items[index].asks_text.clone().unwrap_or_default();
            rows.push(Row::text(ansi::fit(&format!("   {label}: {field}{}", paint(ctx, "7", " ")), width)));
        }
        if let Some(error) = &self.error {
            rows.push(Row::text(ansi::fit(&paint(ctx, "31", &format!("   {error}")), width)));
        }
        rows
    }

    /// Up and Down move, Enter chooses, typing filters, Backspace edits the filter, Esc closes.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome {
        if self.entering.is_some() {
            return self.field_key(key);
        }
        let count = self.visible().len();
        match key {
            Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Down | Key::Tab => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            Key::BackTab => self.selected = self.selected.saturating_sub(1),
            Key::Enter => return self.choose(),
            Key::Escape => return Outcome::Close,
            Key::Backspace => {
                self.filter.pop();
            }
            Key::Text(text) => {
                self.filter.push_str(text);
                self.selected = 0;
                self.error = None;
            }
            Key::Mouse(Mouse { .. }) => {}
            _ => {}
        }
        Outcome::Keep
    }

    /// Selects and chooses the clicked item.
    /// @param item - the item's position among the matching items
    fn click(&mut self, item: usize) -> Outcome {
        self.selected = item;
        self.choose()
    }
}
