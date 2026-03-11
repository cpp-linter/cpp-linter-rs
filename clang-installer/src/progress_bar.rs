use std::io::{IsTerminal, Result, Write, stdin, stdout};

/// A simple progress bar implementation that supports both interactive and non-interactive terminals.
pub struct ProgressBar {
    /// The `total` size of the task being tracked, if known.
    total: Option<u64>,
    /// The `current` progress towards the `total`.
    current: u64,
    /// The number of `steps` completed in the progress bar.
    ///
    /// This is primarily used for tracking how many times the progress has changed.
    /// When `total` is unknown, this is the only way to track progress, and it will be incremented on every update..
    steps: u32,
    /// A mutex lock on the stdout.
    ///
    /// Using this instead of `print!()` allows for faster writes to stdout and
    /// prevents other threads from interrupting the output of the progress bar.
    stdout_handle: std::io::StdoutLock<'static>,
    /// Is the terminal session interactive?
    is_interactive: bool,
    /// The leading prompt to display before the progress bar (e.g. "Downloading")
    ///
    /// Note, an indentation is prefixed to this (to align better with `log::log!()` prefixes),
    /// and a space is added to separate the prompt from the progress bar.
    prompt: String,
}

impl ProgressBar {
    const BAR_CHAR: &str = "#";
    const EMPTY_CHAR: &str = "-";
    const MAX_BAR_WIDTH: u32 = 20;
    const LOG_INDENT: &str = "         ";

    /// Creates a new `ProgressBar` instance.
    ///
    /// This is considered infallible, but it is recommended to call [`Self::render()`] immediately after instantiation.
    /// ```
    /// use clang_installer::ProgressBar;
    /// let mut progress_bar = ProgressBar::new(Some(100), "Downloading");
    /// progress_bar.render().unwrap(); // render 0% state
    /// progress_bar.inc(50).unwrap(); // render 50% state
    /// progress_bar.inc(50).unwrap(); // render 100% state
    /// progress_bar.finish().unwrap(); // clean up and write a line break (move to next line)
    /// // stdout lock is released when `progress_bar` goes out of scope
    /// ```
    pub fn new(total: Option<u64>, prompt: &str) -> Self {
        Self {
            total,
            current: 0,
            steps: 0,
            stdout_handle: stdout().lock(),
            is_interactive: stdin().is_terminal(),
            prompt: prompt.to_string(),
        }
    }

    /// Increments the progress by the specified `delta` and updates the display.
    ///
    /// If the `total` is known, then the progress bar will be updated based on the percentage of `current` to `total`.
    /// If the `total` is unknown, then the progress bar will simply increment by one step for each call to this method.
    pub fn inc(&mut self, delta: u64) -> Result<()> {
        self.current += delta;
        self.render()
    }

    /// Finishes the progress bar and moves to the next line.
    pub fn finish(&mut self) -> Result<()> {
        writeln!(&mut self.stdout_handle)?; // Move to the next line after finishing
        self.stdout_handle.flush()
    }

    /// Renders the progress bar based on the current state.
    ///
    /// This should only be invoked once after [`Self::new()`].
    /// Subsequent updates should be made using [`Self::inc()`], which will call this method internally.
    pub fn render(&mut self) -> Result<()> {
        let advance_bar = self.total.map(|total| {
            let progress = self.current as f64 / total as f64;

            (progress * Self::MAX_BAR_WIDTH as f64).floor() as u32
        });
        if let Some(new_steps) = advance_bar
            && new_steps > self.steps
        {
            // self.total is Some() known value
            if self.is_interactive {
                // rewrite entire line including prompt
                let mut out = format!("{}{} ", Self::LOG_INDENT, self.prompt);
                for _ in 0..new_steps {
                    out.push_str(Self::BAR_CHAR);
                }
                for _ in new_steps..Self::MAX_BAR_WIDTH {
                    out.push_str(Self::EMPTY_CHAR);
                }
                out.push('\r');
                write!(&mut self.stdout_handle, "{}", out)?;
            } else {
                // only write chars to line (without new line)
                let mut out = if self.steps == 0 {
                    format!("{}{} ", Self::LOG_INDENT, self.prompt)
                } else {
                    String::new()
                };
                for _ in self.steps..new_steps {
                    out.push_str(Self::BAR_CHAR);
                }
                write!(&mut self.stdout_handle, "{}", out)?;
            }
            self.steps = new_steps;
            self.stdout_handle.flush()?;
        } else if self.total.is_none() {
            // self.total is None (unknown value)
            // in this case we'll use self.steps to record how many chunks were processed
            self.steps += 1;
            if self.is_interactive {
                // rewrite entire line including prompt
                let mut out = format!("{}{} ", Self::LOG_INDENT, self.prompt);
                for _ in 0..self.steps {
                    out.push_str(Self::BAR_CHAR);
                }
                out.push('\r'); // Move cursor back to the beginning of the line
                write!(&mut self.stdout_handle, "{}", out)?;
            } else {
                // only write chars to line (without new line)
                if self.steps == 1 {
                    write!(
                        &mut self.stdout_handle,
                        "{}{} ",
                        Self::LOG_INDENT,
                        self.prompt
                    )?;
                }
                write!(&mut self.stdout_handle, "{}", Self::BAR_CHAR)?;
            }
            self.stdout_handle.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ProgressBar;

    #[test]
    fn no_total() {
        let mut progress_bar = ProgressBar::new(None, "Processing");
        for _ in 0..5 {
            progress_bar.inc(1).unwrap();
        }
        progress_bar.finish().unwrap();
    }

    #[test]
    fn with_total() {
        let mut progress_bar = ProgressBar::new(Some(100), "Processing");
        for _ in 0..100 {
            progress_bar.inc(1).unwrap();
        }
        progress_bar.finish().unwrap();
    }
}
