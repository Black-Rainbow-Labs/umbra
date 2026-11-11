//! Long command output (task-2617, E6). The model sees the first and last 5,000 characters of an
//! output longer than 10,000. The whole output is also written to a file next to the session's
//! trajectory, and the observation names that file, so the model can search it with `grep` or
//! read a part with `sed` instead of running a slow command again. Single lines longer than 2,000
//! characters are cut in the excerpt the model sees; the file keeps them whole.

use std::path::{Path, PathBuf};

use crate::models::action::Observation;

/// Outputs longer than this many characters are cut to a head and a tail by the observation
/// template, so they are also saved to a file.
pub const SPILL_CHARS: usize = 10_000;

/// Lines longer than this many characters are cut in what the model sees.
pub const MAX_LINE_CHARS: usize = 2_000;

/// The folder a session's long outputs go in: `run.traj.json` has `run.traj.outputs/`.
/// @param trajectory - the trajectory path
pub fn outputs_dir(trajectory: &Path) -> PathBuf {
    trajectory.with_extension("outputs")
}

/// Prepares an observation for the model: an output too long to show whole is written to
/// `<dir>/<number>.txt`, the observation records that path, and lines over `MAX_LINE_CHARS` are
/// cut in the text the model sees. Short outputs are returned as they are. A file that cannot be
/// written leaves the observation without a path.
/// @param observation - the command's result
/// @param dir - the session's outputs folder
/// @param number - a number for the file name, unique in the session
pub fn spill_long_output(mut observation: Observation, dir: &Path, number: u32) -> Observation {
    if observation.output.chars().count() <= SPILL_CHARS {
        return observation;
    }
    let file = dir.join(format!("{number}.txt"));
    let written = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&file, &observation.output));
    if written.is_ok() {
        observation.output_file = Some(file.to_string_lossy().replace('\\', "/"));
    }
    observation.output = cut_long_lines(&observation.output, MAX_LINE_CHARS);
    observation
}

/// Cuts every line longer than `max` characters to its first `max`, followed by a note of how
/// many characters were cut.
/// @param text - the output
/// @param max - the longest line kept whole
pub fn cut_long_lines(text: &str, max: usize) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let count = line.chars().count();
            if count <= max {
                return line.to_string();
            }
            let kept: String = line.chars().take(max).collect();
            let ending = if line.ends_with('\n') { "\n" } else { "" };
            format!("{kept}[... {} characters of this line cut]{ending}", count - max)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_output_is_saved_whole_and_its_long_lines_are_cut_for_the_model() {
        let dir = std::env::temp_dir().join(format!("umbra-spill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let output = format!("{}\nend\n", "x".repeat(40_000_000));
        let observation = spill_long_output(Observation { output: output.clone(), ..Default::default() }, &dir, 7);
        let file = observation.output_file.clone().expect("the output was saved");
        assert_eq!(std::fs::read_to_string(&file).unwrap().len(), output.len(), "the file holds all of it");
        assert!(observation.output.len() < 3_000, "the model sees the cut line, not 40 MB");
        assert!(observation.output.contains("characters of this line cut") && observation.output.ends_with("end\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_short_output_is_left_alone() {
        let observation = spill_long_output(Observation { output: "ok\n".into(), ..Default::default() }, Path::new("unused"), 1);
        assert_eq!(observation.output, "ok\n");
        assert!(observation.output_file.is_none());
    }
}
