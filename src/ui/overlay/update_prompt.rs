//! The prompt above the input line that offers a newer Umbra release: Install and restart, Remind
//! me tomorrow, or Skip this version. Left, Right, Tab and a click choose, Enter confirms, Esc is
//! Remind me tomorrow. Typing a character closes the prompt and the character goes to the input
//! line, so the prompt never takes a message the person is typing.

use super::{Outcome, Overlay, Placement};
use crate::services::update_service;
use crate::ui::block::{paint, LayoutCtx, Row, Target};
use crate::utils::ansi;
use crate::utils::keys::Key;

/// The choices, in the order they are drawn.
const CHOICES: [&str; 3] = ["Install and restart", "Remind me tomorrow", "Skip this version"];

/// The prompt.
pub struct UpdatePrompt {
    version: String,
    selected: usize,
}

impl UpdatePrompt {
    /// A prompt for a version, with Install selected.
    /// @param version - the newer version
    pub fn new(version: &str) -> Self {
        UpdatePrompt { version: version.to_string(), selected: 0 }
    }

    /// Does what a choice says.
    /// @param choice - its position in `CHOICES`
    fn choose(&self, choice: usize) -> Outcome {
        let version = self.version.clone();
        match choice {
            0 => {
                update_service::set_available(None);
                Outcome::Line("/update".into())
            }
            1 => Outcome::CloseWith(vec![Box::new(update_service::remind_later)]),
            _ => Outcome::CloseWith(vec![Box::new(move || update_service::skip(&version))]),
        }
    }
}

impl Overlay for UpdatePrompt {
    /// Drawn above the input line.
    fn placement(&self) -> Placement {
        Placement::AboveStatus
    }

    /// The line naming the versions, then the choices with the selected one marked.
    /// @param width - the columns available
    /// @param _height - the most rows it may take
    /// @param ctx - the frame
    fn rows(&mut self, width: usize, _height: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let title = format!(" {} Umbra {} is available. You have {}.", paint(ctx, "1;33", "▲"), self.version, update_service::VERSION);
        let mut rows = vec![Row::text(ansi::fit(&format!("{title}{}", paint(ctx, "90", "   ←→ choose · Enter confirms · Esc later")), width))];
        let mut line = String::from("   ");
        for (index, choice) in CHOICES.iter().enumerate() {
            let text = if index == self.selected { paint(ctx, "1;7", &format!(" › {choice} ")) } else { format!("   {choice} ") };
            line.push_str(&text);
            line.push_str("  ");
        }
        rows.push(Row { text: ansi::fit(&line, width), target: Some(Target::Item(self.selected)), ..Default::default() });
        rows
    }

    /// Left, Right, Tab and Shift+Tab move, Enter confirms, Esc reminds tomorrow.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome {
        match key {
            Key::Left | Key::BackTab | Key::Up => self.selected = (self.selected + CHOICES.len() - 1) % CHOICES.len(),
            Key::Right | Key::Tab | Key::Down => self.selected = (self.selected + 1) % CHOICES.len(),
            Key::Enter => return self.choose(self.selected),
            Key::Escape => return self.choose(1),
            _ => {}
        }
        Outcome::Keep
    }

    /// A click on the choices row confirms the selected choice.
    /// @param item - the selected choice
    fn click(&mut self, item: usize) -> Outcome {
        self.choose(item)
    }

    /// Typed text goes to the input line, which closes the prompt.
    fn passes_typing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choices_move_and_install_sends_the_update_command() {
        let mut prompt = UpdatePrompt::new("99.0.0");
        prompt.key(&Key::Right);
        assert_eq!(prompt.selected, 1);
        prompt.key(&Key::Left);
        prompt.key(&Key::Left);
        assert_eq!(prompt.selected, 2);
        prompt.key(&Key::Right);
        assert!(matches!(prompt.key(&Key::Enter), Outcome::Line(line) if line == "/update"));
    }
}
