use std::time::Instant;

use alacritty_terminal::{
    Term,
    grid::Dimensions,
    index::{Column, Line},
    term::cell::Flags as CellFlags,
};

use crate::terminal::{TerminalContent, ZedListener};

/// How the block boundary was detected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockDetection {
    ShellIntegration,
    Heuristic,
}

/// A single command block: the command text and its captured output.
#[derive(Clone, Debug)]
pub struct Block {
    pub id: usize,
    pub command: String,
    pub output: String,
    pub timestamp: Instant,
    pub exit_code: Option<i32>,
    pub detection: BlockDetection,
}

/// Internal state machine for the block tracker.
#[derive(Debug)]
enum TrackerState {
    Idle,
    Running {
        command: String,
        output_start_line: Line,
        history_at_start: usize,
    },
}

/// Tracks terminal blocks by detecting Enter keystrokes and prompt lines.
pub struct BlockTracker {
    state: TrackerState,
    blocks: Vec<Block>,
    next_id: usize,
}

impl BlockTracker {
    pub fn new() -> Self {
        Self {
            state: TrackerState::Idle,
            blocks: Vec::new(),
            next_id: 0,
        }
    }

    /// Called when Enter is pressed. Captures the current prompt line as the command.
    pub fn on_enter(&mut self, term: &Term<ZedListener>) {
        let cursor = term.grid().cursor.point;
        let command = read_command_at_cursor(term, cursor.line, cursor.column);

        // Skip empty commands or bare prompt characters
        if command.is_empty() || is_bare_prompt(&command) {
            return;
        }

        // Strip the prompt prefix to get the actual command
        let cmd = strip_prompt(&command);
        if cmd.is_empty() {
            return;
        }

        // Output starts on the line after the cursor (last line of possibly-wrapped command)
        let output_start = Line(cursor.line.0 + 1);

        self.state = TrackerState::Running {
            command: cmd,
            output_start_line: output_start,
            history_at_start: term.history_size(),
        };
    }

    /// Called during `sync()` to check whether a new prompt has appeared,
    /// which signals the end of the previous command's output.
    pub fn on_sync(&mut self, term: &Term<ZedListener>, content: &TerminalContent) {
        let TrackerState::Running {
            ref command,
            output_start_line,
            history_at_start,
        } = self.state
        else {
            return;
        };

        let cursor = &content.cursor;
        let cursor_line = cursor.point.line;
        let cursor_col = cursor.point.column;

        // Adjust output_start_line for scrollback growth
        let history_delta = term.history_size() as i32 - history_at_start as i32;
        let adjusted_start = Line(output_start_line.0 - history_delta);

        // Cursor must have moved past the output start region.
        // This prevents false-positive prompt detection on the same frame
        // where Enter was pressed (PTY hasn't processed it yet).
        if cursor_line.0 < adjusted_start.0 {
            return;
        }

        let line_text = read_line_text(term, cursor_line);

        if !is_prompt(&line_text, cursor_col) {
            return;
        }

        let topmost = term.topmost_line();
        let from = if adjusted_start.0 < topmost.0 {
            topmost
        } else {
            adjusted_start
        };
        let output = read_range_text(term, from, Line(cursor_line.0 - 1));

        let block = Block {
            id: self.next_id,
            command: command.clone(),
            output,
            timestamp: Instant::now(),
            exit_code: None,
            detection: BlockDetection::Heuristic,
        };

        self.next_id += 1;
        self.blocks.push(block);
        self.state = TrackerState::Idle;
    }

    /// Process an OSC 133 event (from future shell integration).
    #[allow(dead_code)]
    pub fn on_osc133(&mut self, event: Osc133Event, term: &Term<ZedListener>) {
        match event {
            Osc133Event::PromptStart => {
                if let TrackerState::Running {
                    ref command,
                    output_start_line,
                    history_at_start,
                } = self.state
                {
                    let cursor_line = term.grid().cursor.point.line;
                    let output =
                        read_output(term, output_start_line, history_at_start, cursor_line);

                    let block = Block {
                        id: self.next_id,
                        command: command.clone(),
                        output,
                        timestamp: Instant::now(),
                        exit_code: None,
                        detection: BlockDetection::ShellIntegration,
                    };
                    self.next_id += 1;
                    self.blocks.push(block);
                    self.state = TrackerState::Idle;
                }
            }
            Osc133Event::CommandFinished(exit_code) => {
                if let TrackerState::Running {
                    ref command,
                    output_start_line,
                    history_at_start,
                } = self.state
                {
                    let cursor_line = term.grid().cursor.point.line;
                    let output =
                        read_output(term, output_start_line, history_at_start, cursor_line);

                    let block = Block {
                        id: self.next_id,
                        command: command.clone(),
                        output,
                        timestamp: Instant::now(),
                        exit_code: Some(exit_code),
                        detection: BlockDetection::ShellIntegration,
                    };
                    self.next_id += 1;
                    self.blocks.push(block);
                    self.state = TrackerState::Idle;
                }
            }
            _ => {}
        }
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn current_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, TrackerState::Running { .. })
    }
}

// ---------------------------------------------------------------------------
// OSC 133 scanner
// ---------------------------------------------------------------------------

/// Events decoded from the OSC 133 semantic prompt protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Osc133Event {
    /// `\x1b]133;A` — Prompt start
    PromptStart,
    /// `\x1b]133;B` — Command start (after user presses Enter)
    CommandStart,
    /// `\x1b]133;C` — Command output start
    OutputStart,
    /// `\x1b]133;D;exit_code` — Command finished
    CommandFinished(i32),
}

/// Byte-level scanner for OSC 133 sequences.
///
/// Feeds raw PTY bytes and emits `Osc133Event` values. This parser handles
/// both BEL-terminated (`\x07`) and ST-terminated (`\x1b\\`) OSC sequences.
#[allow(dead_code)]
pub struct Osc133Scanner {
    state: ScanState,
    buf: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ScanState {
    Ground,
    Esc,           // saw \x1b
    OscStart,      // saw \x1b]
    Osc1,          // saw \x1b]1
    Osc13,         // saw \x1b]13
    Osc133,        // saw \x1b]133
    OscSemi,       // saw \x1b]133;
    OscPayload,    // collecting payload until ST or BEL
    OscPayloadEsc, // saw \x1b inside payload (possible ST)
}

#[allow(dead_code)]
impl Osc133Scanner {
    pub fn new() -> Self {
        Self {
            state: ScanState::Ground,
            buf: Vec::new(),
        }
    }

    /// Feed a slice of raw PTY bytes and return any decoded events.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Osc133Event> {
        let mut events = Vec::new();

        for &b in bytes {
            match self.state {
                ScanState::Ground => {
                    if b == 0x1b {
                        self.state = ScanState::Esc;
                    }
                }
                ScanState::Esc => {
                    if b == b']' {
                        self.state = ScanState::OscStart;
                    } else {
                        self.state = ScanState::Ground;
                    }
                }
                ScanState::OscStart => {
                    if b == b'1' {
                        self.state = ScanState::Osc1;
                    } else {
                        self.state = ScanState::Ground;
                    }
                }
                ScanState::Osc1 => {
                    if b == b'3' {
                        self.state = ScanState::Osc13;
                    } else {
                        self.state = ScanState::Ground;
                    }
                }
                ScanState::Osc13 => {
                    if b == b'3' {
                        self.state = ScanState::Osc133;
                    } else {
                        self.state = ScanState::Ground;
                    }
                }
                ScanState::Osc133 => {
                    if b == b';' {
                        self.state = ScanState::OscSemi;
                        self.buf.clear();
                    } else {
                        self.state = ScanState::Ground;
                    }
                }
                ScanState::OscSemi => {
                    // First payload byte
                    self.buf.push(b);
                    if b == 0x07 {
                        // BEL terminator — payload is just what was before
                        self.buf.pop(); // remove the BEL
                        if let Some(ev) = Self::parse_payload(&self.buf) {
                            events.push(ev);
                        }
                        self.state = ScanState::Ground;
                    } else if b == 0x1b {
                        self.buf.pop();
                        self.state = ScanState::OscPayloadEsc;
                    } else {
                        self.state = ScanState::OscPayload;
                    }
                }
                ScanState::OscPayload => {
                    if b == 0x07 {
                        if let Some(ev) = Self::parse_payload(&self.buf) {
                            events.push(ev);
                        }
                        self.buf.clear();
                        self.state = ScanState::Ground;
                    } else if b == 0x1b {
                        self.state = ScanState::OscPayloadEsc;
                    } else {
                        self.buf.push(b);
                    }
                }
                ScanState::OscPayloadEsc => {
                    if b == b'\\' {
                        // ST terminator
                        if let Some(ev) = Self::parse_payload(&self.buf) {
                            events.push(ev);
                        }
                        self.buf.clear();
                        self.state = ScanState::Ground;
                    } else {
                        // Not ST — discard
                        self.buf.clear();
                        self.state = ScanState::Ground;
                    }
                }
            }
        }

        events
    }

    fn parse_payload(buf: &[u8]) -> Option<Osc133Event> {
        if buf.is_empty() {
            return None;
        }
        match buf[0] {
            b'A' => Some(Osc133Event::PromptStart),
            b'B' => Some(Osc133Event::CommandStart),
            b'C' => Some(Osc133Event::OutputStart),
            b'D' => {
                // D may be followed by ;exit_code
                if buf.len() > 2 && buf[1] == b';' {
                    let code_str = std::str::from_utf8(&buf[2..]).ok()?;
                    let code = code_str.trim().parse::<i32>().ok()?;
                    Some(Osc133Event::CommandFinished(code))
                } else {
                    Some(Osc133Event::CommandFinished(0))
                }
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read output text between a recorded start line and the current prompt line,
/// compensating for scrollback history growth since the command started.
fn read_output(
    term: &Term<ZedListener>,
    output_start_line: Line,
    history_at_start: usize,
    prompt_line: Line,
) -> String {
    let history_delta = term.history_size() as i32 - history_at_start as i32;
    let adjusted_start = Line(output_start_line.0 - history_delta);

    let topmost = term.topmost_line();
    let from = if adjusted_start.0 < topmost.0 {
        topmost
    } else {
        adjusted_start
    };

    read_range_text(term, from, Line(prompt_line.0 - 1))
}

/// Read the command text at the cursor position, handling:
/// - Wrapped lines (WRAPLINE flag): walks backward to find the start of the command
/// - Auto-suggestions: only reads up to cursor column on the last line
fn read_command_at_cursor(
    term: &Term<ZedListener>,
    cursor_line: Line,
    cursor_col: Column,
) -> String {
    let cols = term.columns();
    let topmost = term.topmost_line();

    // Walk backward through wrapped lines to find the start of this logical line.
    // A line whose last cell has WRAPLINE means it continues on the next row.
    let mut start_line = cursor_line;
    while start_line.0 > topmost.0 {
        let prev = Line(start_line.0 - 1);
        let last_cell = &term.grid()[prev][Column(cols - 1)];
        if last_cell.flags.contains(CellFlags::WRAPLINE) {
            start_line = prev;
        } else {
            break;
        }
    }

    let mut text = String::new();

    // Read full content of all wrapped lines before the cursor line
    for line_idx in start_line.0..cursor_line.0 {
        let row = &term.grid()[Line(line_idx)];
        for col in 0..cols {
            text.push(row[Column(col)].c);
        }
        // No newline — these are wrapped continuations of the same logical line
    }

    // On the cursor line, only read up to cursor column (excludes auto-suggestions)
    let row = &term.grid()[cursor_line];
    for col in 0..cursor_col.0 {
        text.push(row[Column(col)].c);
    }

    text.trim_end().to_string()
}

/// Read a single line from the terminal grid.
fn read_line_text(term: &Term<ZedListener>, line: Line) -> String {
    let cols = term.columns();
    let mut text = String::with_capacity(cols);
    for col in 0..cols {
        text.push(term.grid()[line][Column(col)].c);
    }
    text.trim_end().to_string()
}

/// Read a range of lines (inclusive) from the terminal grid.
fn read_range_text(term: &Term<ZedListener>, from: Line, to: Line) -> String {
    if to.0 < from.0 {
        return String::new();
    }
    let mut lines = Vec::new();
    for line_idx in from.0..=to.0 {
        lines.push(read_line_text(term, Line(line_idx)));
    }
    // Trim trailing empty lines
    while matches!(lines.last(), Some(l) if l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Heuristic: does this line look like a shell prompt?
/// Checks only the text before cursor_col — the cursor should be right after the prompt.
fn is_prompt(line_text: &str, cursor_col: Column) -> bool {
    // Prompt usually isn't very long
    if cursor_col.0 > 40 {
        return false;
    }

    let prefix = line_text.get(..cursor_col.0).unwrap_or(line_text);
    let trimmed = prefix.trim();

    if trimmed.is_empty() {
        return false;
    }

    // The text before the cursor must end with a prompt character.
    // PowerShell "PS path>" ends with '>', bash "user@host:~$" ends with '$', etc.
    trimmed.ends_with('$')
        || trimmed.ends_with('>')
        || trimmed.ends_with('#')
        || trimmed.ends_with('%')
}

/// Check if a string is just a bare prompt character (no actual command).
fn is_bare_prompt(s: &str) -> bool {
    let t = s.trim();
    matches!(t, "$" | ">" | "#" | "%" | "PS>" | "PS>>" | ">>")
}

/// Strip prompt prefix from a line to get just the command text.
fn strip_prompt(line: &str) -> String {
    let trimmed = line.trim();

    // Try to find the last prompt character and take everything after it
    for sep in ['$', '>', '#', '%'] {
        if let Some(pos) = trimmed.rfind(sep) {
            let after = trimmed[pos + 1..].trim();
            if !after.is_empty() {
                return after.to_string();
            }
        }
    }

    // PowerShell "PS path>" pattern
    if trimmed.starts_with("PS ") {
        if let Some(pos) = trimmed.find('>') {
            let after = trimmed[pos + 1..].trim();
            if !after.is_empty() {
                return after.to_string();
            }
        }
    }

    // If no prompt separator found, return the whole line
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_prompt() {
        assert!(is_prompt("user@host:~$ ", Column(13)));
        assert!(is_prompt("PS C:\\Users> ", Column(12)));
        assert!(is_prompt("# ", Column(2)));
        assert!(is_prompt("% ", Column(2)));
        assert!(!is_prompt(
            "hello world this is a very long output line",
            Column(43)
        ));
        assert!(!is_prompt("", Column(0)));
        // Must NOT match when cursor is past the prompt (command was typed)
        assert!(!is_prompt("PS D:\\agentx> ls", Column(15)));
        assert!(!is_prompt("user@host:~$ echo hello", Column(24)));
    }

    #[test]
    fn test_strip_prompt() {
        assert_eq!(strip_prompt("user@host:~$ echo hello"), "echo hello");
        assert_eq!(strip_prompt("PS C:\\Users> dir"), "dir");
        assert_eq!(strip_prompt("# ls -la"), "ls -la");
        assert_eq!(strip_prompt("% whoami"), "whoami");
        assert_eq!(strip_prompt("echo hello"), "echo hello");
    }

    #[test]
    fn test_is_bare_prompt() {
        assert!(is_bare_prompt("$"));
        assert!(is_bare_prompt(" > "));
        assert!(is_bare_prompt("#"));
        assert!(!is_bare_prompt("$ echo hi"));
    }

    #[test]
    fn test_osc133_scanner_bel() {
        let mut scanner = Osc133Scanner::new();
        let events = scanner.feed(b"\x1b]133;A\x07");
        assert_eq!(events, vec![Osc133Event::PromptStart]);
    }

    #[test]
    fn test_osc133_scanner_st() {
        let mut scanner = Osc133Scanner::new();
        let events = scanner.feed(b"\x1b]133;D;0\x1b\\");
        assert_eq!(events, vec![Osc133Event::CommandFinished(0)]);
    }

    #[test]
    fn test_osc133_scanner_exit_code() {
        let mut scanner = Osc133Scanner::new();
        let events = scanner.feed(b"\x1b]133;D;127\x07");
        assert_eq!(events, vec![Osc133Event::CommandFinished(127)]);
    }

    #[test]
    fn test_osc133_scanner_multiple() {
        let mut scanner = Osc133Scanner::new();
        let events = scanner.feed(b"\x1b]133;A\x07some output\x1b]133;B\x07");
        assert_eq!(
            events,
            vec![Osc133Event::PromptStart, Osc133Event::CommandStart]
        );
    }

    #[test]
    fn test_osc133_scanner_split_across_feeds() {
        let mut scanner = Osc133Scanner::new();
        let e1 = scanner.feed(b"\x1b]133");
        assert!(e1.is_empty());
        let e2 = scanner.feed(b";C\x07");
        assert_eq!(e2, vec![Osc133Event::OutputStart]);
    }
}
