use std::io::{self, IsTerminal};

use aozora_proof_core::{
    CheckError, DetectionClass, FixAlternative, FixError, FixOperation, SafeFixResult, TextEdit,
    apply_safe, apply_text_edits, official_items,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as TextSpan};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::document::Document;
use crate::fix_command::{atomic_write, unified_diff};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ReviewError {
    #[error("{message}")]
    Message { message: String },
    #[error("terminal I/O failed: {source}")]
    Io {
        #[source]
        source: io::Error,
    },
    #[error("report coordinates are invalid: {source}")]
    Check {
        #[source]
        source: CheckError,
    },
    #[error("{label}: reviewed fixes could not be prepared: {source}")]
    Fix {
        label: String,
        #[source]
        source: FixError,
    },
    #[error("{label}: reviewed output could not be written: {source}")]
    Write {
        label: String,
        #[source]
        source: crate::fix_command::FixCommandError,
    },
}

impl ReviewError {
    pub(crate) const fn is_internal(&self) -> bool {
        match self {
            Self::Check { .. } => true,
            Self::Fix { source, .. } => source.is_internal(),
            Self::Write { source, .. } => source.is_internal(),
            Self::Message { .. } | Self::Io { .. } => false,
        }
    }
}

#[derive(Debug, Clone)]
struct ReviewItem {
    file: usize,
    original: String,
    title: String,
    message: String,
    authority: String,
    fixes: Vec<FixAlternative>,
    decision: Option<Decision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Accepted(usize),
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Review,
    Confirm,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateResult {
    Continue,
    ExitWithoutChanges,
    Commit,
}

#[derive(Debug)]
struct ReviewState {
    items: Vec<ReviewItem>,
    cursor: usize,
    mode: Mode,
}

impl ReviewState {
    fn new(documents: &[Document]) -> Result<Self, ReviewError> {
        let mut items = Vec::new();
        for (file, document) in documents.iter().enumerate() {
            for finding in &document.report.findings {
                if finding.detection != DetectionClass::Review {
                    continue;
                }
                let position = finding
                    .position(&document.report.decoded)
                    .map_err(|source| ReviewError::Check { source })?;
                let line_index =
                    position
                        .line
                        .checked_sub(1)
                        .ok_or_else(|| ReviewError::Message {
                            message: "a one-based source line was zero".to_owned(),
                        })?;
                let original = document
                    .report
                    .decoded
                    .lines()
                    .nth(line_index)
                    .unwrap_or("")
                    .to_owned();
                let fixes = finding
                    .fixes
                    .iter()
                    .filter(|fix| fix.applicability == aozora_proof_core::FixApplicability::Review)
                    .cloned()
                    .collect();
                items.push(ReviewItem {
                    file,
                    original,
                    title: finding.code.to_owned(),
                    message: finding.message.clone(),
                    authority: finding.authority_url.to_owned(),
                    fixes,
                    decision: None,
                });
            }
        }
        for item in official_items()
            .iter()
            .filter(|item| item.detection == DetectionClass::Manual)
        {
            items.push(ReviewItem {
                file: 0,
                original: "Manual checklist item".to_owned(),
                title: item.title.to_owned(),
                message: "This requirement must be confirmed manually.".to_owned(),
                authority: item.authority_url.to_owned(),
                fixes: Vec::new(),
                decision: None,
            });
        }
        Ok(Self {
            items,
            cursor: 0,
            mode: Mode::Review,
        })
    }

    fn handle(&mut self, key: KeyEvent) -> StateResult {
        if key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return StateResult::ExitWithoutChanges;
        }
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.mode = Mode::Confirm;
            return StateResult::Continue;
        }
        match self.mode {
            Mode::Confirm => self.handle_confirm(key),
            Mode::Help => {
                self.mode = Mode::Review;
                StateResult::Continue
            }
            Mode::Review => self.handle_review(key),
        }
    }

    const fn handle_confirm(&mut self, key: KeyEvent) -> StateResult {
        match key.code {
            KeyCode::Char('y') => StateResult::Commit,
            KeyCode::Char('n') => {
                self.mode = Mode::Review;
                StateResult::Continue
            }
            KeyCode::Char('q') => StateResult::ExitWithoutChanges,
            _ => StateResult::Continue,
        }
    }

    fn handle_review(&mut self, key: KeyEvent) -> StateResult {
        match key.code {
            KeyCode::Char('y') => {
                if self
                    .items
                    .get(self.cursor)
                    .is_some_and(|candidate| !candidate.fixes.is_empty())
                    && let Some(item) = self.items.get_mut(self.cursor)
                {
                    item.decision = Some(Decision::Accepted(0));
                }
                self.next();
            }
            KeyCode::Char('n') => {
                if let Some(item) = self.items.get_mut(self.cursor) {
                    item.decision = Some(Decision::Rejected);
                }
                self.next();
            }
            KeyCode::Char('a') => {
                for item in &mut self.items {
                    if !item.fixes.is_empty() {
                        item.decision = Some(Decision::Accepted(0));
                    }
                }
            }
            KeyCode::Char('d') => {
                for item in &mut self.items {
                    item.decision = Some(Decision::Rejected);
                }
            }
            KeyCode::Char('g') => self.cycle_alternative(),
            KeyCode::Char('j') | KeyCode::Down => self.next(),
            KeyCode::Char('k' | 'p') | KeyCode::Up => self.previous(),
            KeyCode::Char('/') => self.next_same_rule(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('q') => return StateResult::ExitWithoutChanges,
            _ => {}
        }
        StateResult::Continue
    }

    const fn next(&mut self) {
        if !self.items.is_empty() {
            let next = self.cursor.saturating_add(1);
            self.cursor = if next == self.items.len() { 0 } else { next };
        }
    }

    fn previous(&mut self) {
        if !self.items.is_empty() {
            self.cursor = self
                .cursor
                .checked_sub(1)
                .unwrap_or_else(|| self.items.len().saturating_sub(1));
        }
    }

    fn cycle_alternative(&mut self) {
        let Some(item) = self.items.get(self.cursor) else {
            return;
        };
        if item.fixes.is_empty() {
            return;
        }
        let next = match item.decision {
            Some(Decision::Accepted(index)) => {
                let next = index.saturating_add(1);
                if next == item.fixes.len() { 0 } else { next }
            }
            Some(Decision::Rejected) | None => 0,
        };
        if let Some(item) = self.items.get_mut(self.cursor) {
            item.decision = Some(Decision::Accepted(next));
        }
    }

    fn next_same_rule(&mut self) {
        let Some(current) = self.items.get(self.cursor) else {
            return;
        };
        if let Some(index) = self
            .items
            .iter()
            .enumerate()
            .skip(self.cursor.saturating_add(1))
            .find(|(_, item)| item.title == current.title)
            .map(|(index, _)| index)
        {
            self.cursor = index;
        }
    }

    fn selected_edits(&self, file: usize) -> Vec<TextEdit> {
        self.items
            .iter()
            .filter(|item| item.file == file)
            .filter_map(|item| match item.decision {
                Some(Decision::Accepted(index)) => item.fixes.get(index),
                Some(Decision::Rejected) | None => None,
            })
            .filter_map(|fix| match &fix.operation {
                FixOperation::Text(edit) => Some(edit.clone()),
                FixOperation::RemoveBom
                | FixOperation::NormalizeCrLf
                | FixOperation::EnsureFinalNewline
                | FixOperation::EncodeShiftJis => None,
            })
            .collect()
    }
}

pub(crate) fn run(documents: &[Document]) -> Result<usize, ReviewError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(message("review requires an interactive terminal"));
    }
    if documents.iter().any(|document| document.path.is_none()) {
        return Err(message("review does not accept standard input"));
    }

    let session = TerminalSession::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(io_error)?;
    let mut state = ReviewState::new(documents)?;
    let action = loop {
        terminal
            .draw(|frame| draw(frame, &state, documents))
            .map_err(io_error)?;
        let event = event::read().map_err(io_error)?;
        if let Event::Key(key) = event {
            match state.handle(key) {
                StateResult::Continue => {}
                result => break result,
            }
        }
    };
    drop(terminal);
    drop(session);

    if action != StateResult::Commit {
        return Ok(0);
    }
    commit(documents, &state)
}

fn draw(frame: &mut ratatui::Frame<'_>, state: &ReviewState, documents: &[Document]) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(24),
            Constraint::Percentage(28),
            Constraint::Percentage(10),
        ])
        .split(area);

    if state.mode == Mode::Help {
        frame.render_widget(
            Paragraph::new(
                "y accept  n reject  a accept all  d reject all  g next alternative\n\
                 j/k move  / next same rule  p previous  Ctrl-S final diff\n\
                 q/Esc/Ctrl-C exit without changes  ? help",
            )
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let [original, reason, diff, progress] = chunks.as_ref() else {
        return;
    };
    draw_context(frame, state, [*original, *reason]);
    draw_diff(frame, state, documents, *diff);
    draw_progress(frame, state, *progress);
}

fn draw_context(frame: &mut ratatui::Frame<'_>, state: &ReviewState, areas: [Rect; 2]) {
    let [original_area, reason_area] = areas;
    let item = state.items.get(state.cursor);
    let original = item.map_or("Manual checklist item", |review_item| &review_item.original);
    frame.render_widget(
        Paragraph::new(original)
            .block(Block::default().title("Original").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        original_area,
    );

    let reason = item.map_or_else(
        || "No review candidates.".to_owned(),
        |item| format!("{}\n{}\n{}", item.title, item.message, item.authority),
    );
    frame.render_widget(
        Paragraph::new(reason)
            .block(
                Block::default()
                    .title("Reason / authority")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        reason_area,
    );
}

fn draw_diff(
    frame: &mut ratatui::Frame<'_>,
    state: &ReviewState,
    documents: &[Document],
    area: Rect,
) {
    let item = state.items.get(state.cursor);
    let diff = if state.mode == Mode::Confirm {
        selected_diff(state, documents)
    } else {
        item.and_then(|item| {
            let selected = item
                .decision
                .and_then(|decision| match decision {
                    Decision::Accepted(index) => Some(index),
                    Decision::Rejected => None,
                })
                .unwrap_or(0);
            item.fixes.get(selected)
        })
        .map_or_else(
            || "No automatic replacement; confirm manually.".to_owned(),
            |fix| match &fix.operation {
                FixOperation::Text(edit) => format!(
                    "- decoded bytes {}..{}\n+ {}",
                    edit.span.start, edit.span.end, edit.replacement
                ),
                operation => format!("+ {}", operation.as_wire_str()),
            },
        )
    };
    let title = if state.mode == Mode::Confirm {
        "Final diff — y write / n back / q abort"
    } else {
        "Candidate diff"
    };
    frame.render_widget(
        Paragraph::new(diff)
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_progress(frame: &mut ratatui::Frame<'_>, state: &ReviewState, area: Rect) {
    let accepted = state
        .items
        .iter()
        .filter(|item| matches!(item.decision, Some(Decision::Accepted(_))))
        .count();
    let rejected = state
        .items
        .iter()
        .filter(|item| matches!(item.decision, Some(Decision::Rejected)))
        .count();
    let progress = Line::from(vec![
        TextSpan::styled(
            format!(
                " {}/{}  accepted {accepted}  rejected {rejected} ",
                state.cursor.saturating_add(1),
                state.items.len()
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        TextSpan::raw(" y/n/q/a/d/g/j/k//p/?  Ctrl-S "),
    ]);
    frame.render_widget(
        Paragraph::new(progress).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn selected_diff(state: &ReviewState, documents: &[Document]) -> String {
    let mut output = String::new();
    for (file, document) in documents.iter().enumerate() {
        let edits = state.selected_edits(file);
        if edits.is_empty() {
            continue;
        }
        match prepare(document, &edits) {
            Ok(fixed) => {
                output.push_str(&unified_diff(
                    &document.label,
                    &document.report.decoded,
                    &fixed.decoded,
                ));
                output.push_str("# ");
                output.push_str(&document.label);
                output.push_str(": output encoding Shift_JIS; line endings CRLF\n");
            }
            Err(source) => {
                output.push_str("# ");
                output.push_str(&document.label);
                output.push_str(": ");
                output.push_str(&source.to_string());
                output.push('\n');
            }
        }
    }
    if output.is_empty() {
        "No staged text edits. Manual checklist items remain.".to_owned()
    } else {
        output
    }
}

fn commit(documents: &[Document], state: &ReviewState) -> Result<usize, ReviewError> {
    let mut prepared = Vec::new();
    for (file, document) in documents.iter().enumerate() {
        let edits = state.selected_edits(file);
        if edits.is_empty() {
            continue;
        }
        let fixed = prepare(document, &edits)?;
        let path = document
            .path
            .as_deref()
            .ok_or_else(|| message("review cannot write standard input"))?;
        prepared.push((path, document.raw.as_slice(), fixed.bytes));
    }
    for (path, original, bytes) in &prepared {
        atomic_write(path, original, bytes).map_err(|source| ReviewError::Write {
            label: path.display().to_string(),
            source,
        })?;
    }
    Ok(prepared.len())
}

fn prepare(document: &Document, edits: &[TextEdit]) -> Result<SafeFixResult, ReviewError> {
    let staged =
        apply_text_edits(&document.report.decoded, edits).map_err(|source| ReviewError::Fix {
            label: document.label.clone(),
            source,
        })?;
    apply_safe(staged.as_bytes(), document.report.orthography).map_err(|source| ReviewError::Fix {
        label: document.label.clone(),
        source,
    })
}

struct TerminalSession;

impl TerminalSession {
    fn enter() -> Result<Self, ReviewError> {
        enable_raw_mode().map_err(io_error)?;
        if let Err(source) = execute!(io::stdout(), EnterAlternateScreen) {
            drop(disable_raw_mode());
            return Err(io_error(source));
        }
        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        drop(execute!(io::stdout(), LeaveAlternateScreen));
        drop(disable_raw_mode());
    }
}

const fn io_error(source: io::Error) -> ReviewError {
    ReviewError::Io { source }
}

fn message(value: impl Into<String>) -> ReviewError {
    ReviewError::Message {
        message: value.into(),
    }
}

#[cfg(test)]
mod tests {
    use aozora_proof_core::{Orthography, run_submission_with_orthography};
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn key_transitions_never_commit_before_final_confirmation() {
        let mut state = ReviewState {
            items: vec![ReviewItem {
                file: 0,
                original: "manual".to_owned(),
                title: "manual".to_owned(),
                message: String::new(),
                authority: String::new(),
                fixes: Vec::new(),
                decision: None,
            }],
            cursor: 0,
            mode: Mode::Review,
        };
        assert_eq!(
            state.handle(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            StateResult::Continue
        );
        assert_eq!(
            state.handle(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            StateResult::Continue
        );
        assert_eq!(state.mode, Mode::Confirm);
        assert_eq!(
            state.handle(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            StateResult::Commit
        );
    }

    #[test]
    fn escape_discards_staged_state() {
        let mut state = ReviewState {
            items: Vec::new(),
            cursor: 0,
            mode: Mode::Review,
        };
        assert_eq!(
            state.handle(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            StateResult::ExitWithoutChanges
        );
    }

    #[test]
    fn control_c_discards_staged_state() {
        let mut state = ReviewState {
            items: Vec::new(),
            cursor: 0,
            mode: Mode::Review,
        };
        assert_eq!(
            state.handle(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            StateResult::ExitWithoutChanges
        );
    }

    #[test]
    fn patch_keys_navigate_and_update_decisions() {
        let replacement = |value: &str| {
            FixAlternative::review_text(
                aozora_proof_core::Span { start: 0, end: 0 },
                value.to_owned(),
                "replace".to_owned(),
                "置換".to_owned(),
            )
        };
        let item = |title: &str| ReviewItem {
            file: 0,
            original: String::new(),
            title: title.to_owned(),
            message: String::new(),
            authority: String::new(),
            fixes: vec![replacement("one"), replacement("two")],
            decision: None,
        };
        let mut state = ReviewState {
            items: vec![item("same"), item("same"), item("other")],
            cursor: 0,
            mode: Mode::Review,
        };
        let key = |value| KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE);

        assert_eq!(state.handle(key('g')), StateResult::Continue);
        assert_eq!(
            state.items.first().and_then(|item| item.decision),
            Some(Decision::Accepted(0))
        );
        assert_eq!(state.handle(key('g')), StateResult::Continue);
        assert_eq!(
            state.items.first().and_then(|item| item.decision),
            Some(Decision::Accepted(1))
        );

        state.handle(key('/'));
        assert_eq!(state.cursor, 1);
        state.handle(key('j'));
        assert_eq!(state.cursor, 2);
        state.handle(key('k'));
        assert_eq!(state.cursor, 1);
        state.handle(key('p'));
        assert_eq!(state.cursor, 0);

        state.handle(key('a'));
        assert!(
            state
                .items
                .iter()
                .all(|item| matches!(item.decision, Some(Decision::Accepted(0))))
        );
        state.handle(key('d'));
        assert!(
            state
                .items
                .iter()
                .all(|item| item.decision == Some(Decision::Rejected))
        );

        state.handle(key('?'));
        assert_eq!(state.mode, Mode::Help);
        state.handle(key('n'));
        assert_eq!(state.mode, Mode::Review);
        assert_eq!(state.handle(key('q')), StateResult::ExitWithoutChanges);
    }

    #[test]
    fn test_backend_renders_all_review_regions() {
        let raw = "一ヶ月\n".as_bytes().to_vec();
        let documents = vec![Document {
            label: "work.txt".to_owned(),
            path: None,
            report: run_submission_with_orthography(&raw, Orthography::Mixed)
                .expect("valid report"),
            raw,
        }];
        let state = ReviewState::new(&documents).expect("valid review state");
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|frame| draw(frame, &state, &documents))
            .expect("draw review state");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("Original"));
        assert!(rendered.contains("Reason / authority"));
        assert!(rendered.contains("Candidate diff"));
        assert!(rendered.contains("Ctrl-S"));
    }
}
