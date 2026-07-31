use std::error::Error;
use std::fmt;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

use aozora_proof_core::{
    DetectionClass, FixAlternative, FixOperation, SafeFixResult, TextEdit, apply_safe,
    apply_text_edits, official_items,
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

#[derive(Debug)]
pub(crate) struct ReviewError {
    message: String,
}

impl fmt::Display for ReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ReviewError {}

#[derive(Debug, Clone)]
struct Candidate {
    file: usize,
    finding: Option<usize>,
    title: String,
    message: String,
    authority: String,
    fixes: Vec<FixAlternative>,
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
    candidates: Vec<Candidate>,
    decisions: Vec<Option<Decision>>,
    cursor: usize,
    mode: Mode,
}

impl ReviewState {
    fn new(documents: &[Document]) -> Self {
        let mut candidates = Vec::new();
        for (file, document) in documents.iter().enumerate() {
            for (finding_index, finding) in document.report.findings.iter().enumerate() {
                let detection = aozora_proof_core::explain(finding.code)
                    .map_or(DetectionClass::Automatic, |rule| rule.detection);
                if detection != DetectionClass::Review {
                    continue;
                }
                let fixes = finding
                    .fixes
                    .iter()
                    .filter(|fix| fix.applicability == aozora_proof_core::FixApplicability::Review)
                    .cloned()
                    .collect();
                candidates.push(Candidate {
                    file,
                    finding: Some(finding_index),
                    title: finding.code.to_owned(),
                    message: finding.message.clone(),
                    authority: finding.authority_url.to_owned(),
                    fixes,
                });
            }
        }
        for item in official_items()
            .iter()
            .filter(|item| item.detection == DetectionClass::Manual)
        {
            candidates.push(Candidate {
                file: 0,
                finding: None,
                title: item.title.to_owned(),
                message: "This requirement must be confirmed manually.".to_owned(),
                authority: item.authority_url.to_owned(),
                fixes: Vec::new(),
            });
        }
        let decisions = vec![None; candidates.len()];
        Self {
            candidates,
            decisions,
            cursor: 0,
            mode: Mode::Review,
        }
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
                    .candidates
                    .get(self.cursor)
                    .is_some_and(|candidate| !candidate.fixes.is_empty())
                    && let Some(decision) = self.decisions.get_mut(self.cursor)
                {
                    *decision = Some(Decision::Accepted(0));
                }
                self.next();
            }
            KeyCode::Char('n') => {
                if let Some(decision) = self.decisions.get_mut(self.cursor) {
                    *decision = Some(Decision::Rejected);
                }
                self.next();
            }
            KeyCode::Char('a') => {
                for (candidate, decision) in self.candidates.iter().zip(&mut self.decisions) {
                    if !candidate.fixes.is_empty() {
                        *decision = Some(Decision::Accepted(0));
                    }
                }
            }
            KeyCode::Char('d') => self
                .decisions
                .iter_mut()
                .for_each(|decision| *decision = Some(Decision::Rejected)),
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
        if !self.candidates.is_empty() {
            self.cursor = (self.cursor + 1) % self.candidates.len();
        }
    }

    fn previous(&mut self) {
        if !self.candidates.is_empty() {
            self.cursor = self
                .cursor
                .checked_sub(1)
                .unwrap_or(self.candidates.len() - 1);
        }
    }

    fn cycle_alternative(&mut self) {
        let Some(candidate) = self.candidates.get(self.cursor) else {
            return;
        };
        if candidate.fixes.is_empty() {
            return;
        }
        let next = match self.decisions.get(self.cursor).copied().flatten() {
            Some(Decision::Accepted(index)) => (index + 1) % candidate.fixes.len(),
            Some(Decision::Rejected) | None => 0,
        };
        if let Some(decision) = self.decisions.get_mut(self.cursor) {
            *decision = Some(Decision::Accepted(next));
        }
    }

    fn next_same_rule(&mut self) {
        let Some(current) = self.candidates.get(self.cursor) else {
            return;
        };
        if let Some(index) = self
            .candidates
            .iter()
            .enumerate()
            .skip(self.cursor + 1)
            .find(|(_, candidate)| candidate.title == current.title)
            .map(|(index, _)| index)
        {
            self.cursor = index;
        }
    }

    fn selected_edits(&self, file: usize) -> Vec<TextEdit> {
        self.candidates
            .iter()
            .zip(&self.decisions)
            .filter(|(candidate, _)| candidate.file == file)
            .filter_map(|(candidate, decision)| match decision {
                Some(Decision::Accepted(index)) => candidate.fixes.get(*index),
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
    let mut terminal = Terminal::new(backend).map_err(|source| io_error(&source))?;
    let mut state = ReviewState::new(documents);
    let action = loop {
        terminal
            .draw(|frame| draw(frame, &state, documents))
            .map_err(|source| io_error(&source))?;
        let event = event::read().map_err(|source| io_error(&source))?;
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
    draw_context(frame, state, documents, [*original, *reason]);
    draw_diff(frame, state, documents, *diff);
    draw_progress(frame, state, *progress);
}

fn draw_context(
    frame: &mut ratatui::Frame<'_>,
    state: &ReviewState,
    documents: &[Document],
    areas: [Rect; 2],
) {
    let [original_area, reason_area] = areas;
    let candidate = state.candidates.get(state.cursor);
    let source = candidate
        .and_then(|item| documents.get(item.file))
        .map_or("", |document| document.report.decoded.as_str());
    let original = candidate
        .and_then(|item| {
            item.finding
                .and_then(|index| documents.get(item.file)?.report.findings.get(index))
        })
        .map_or_else(
            || "Manual checklist item".to_owned(),
            |finding| {
                let position = finding.position(source);
                source
                    .lines()
                    .nth(position.line.saturating_sub(1))
                    .unwrap_or("")
                    .to_owned()
            },
        );
    frame.render_widget(
        Paragraph::new(original)
            .block(Block::default().title("Original").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        original_area,
    );

    let reason = candidate.map_or_else(
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
    let candidate = state.candidates.get(state.cursor);
    let diff = if state.mode == Mode::Confirm {
        selected_diff(state, documents)
    } else {
        candidate
            .and_then(|item| {
                let selected = state
                    .decisions
                    .get(state.cursor)
                    .copied()
                    .flatten()
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
        .decisions
        .iter()
        .filter(|decision| matches!(decision, Some(Decision::Accepted(_))))
        .count();
    let rejected = state
        .decisions
        .iter()
        .filter(|decision| matches!(decision, Some(Decision::Rejected)))
        .count();
    let progress = Line::from(vec![
        TextSpan::styled(
            format!(
                " {}/{}  accepted {accepted}  rejected {rejected} ",
                state.cursor.saturating_add(1),
                state.candidates.len()
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
                let _ = writeln!(
                    output,
                    "# {}: output encoding Shift_JIS; line endings CRLF",
                    document.label
                );
            }
            Err(source) => {
                let _ = writeln!(output, "# {}: {source}", document.label);
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
        atomic_write(path, original, bytes)
            .map_err(|source| message(format!("{}: {source}", path.display())))?;
    }
    Ok(prepared.len())
}

fn prepare(document: &Document, edits: &[TextEdit]) -> Result<SafeFixResult, ReviewError> {
    let staged = apply_text_edits(&document.report.decoded, edits)
        .map_err(|source| message(format!("{}: {source}", document.label)))?;
    apply_safe(staged.as_bytes(), document.report.orthography)
        .map_err(|source| message(format!("{}: {source}", document.label)))
}

struct TerminalSession;

impl TerminalSession {
    fn enter() -> Result<Self, ReviewError> {
        enable_raw_mode().map_err(|source| io_error(&source))?;
        if let Err(source) = execute!(io::stdout(), EnterAlternateScreen) {
            drop(disable_raw_mode());
            return Err(io_error(&source));
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

fn io_error(source: &io::Error) -> ReviewError {
    message(source.to_string())
}

fn message(value: impl Into<String>) -> ReviewError {
    ReviewError {
        message: value.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use aozora_proof_core::{Orthography, run_submission_with_orthography};
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn key_transitions_never_commit_before_final_confirmation() {
        let mut state = ReviewState {
            candidates: vec![Candidate {
                file: 0,
                finding: None,
                title: "manual".to_owned(),
                message: String::new(),
                authority: String::new(),
                fixes: Vec::new(),
            }],
            decisions: vec![None],
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
            candidates: Vec::new(),
            decisions: Vec::new(),
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
            candidates: Vec::new(),
            decisions: Vec::new(),
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
        let candidate = |title: &str| Candidate {
            file: 0,
            finding: None,
            title: title.to_owned(),
            message: String::new(),
            authority: String::new(),
            fixes: vec![replacement("one"), replacement("two")],
        };
        let mut state = ReviewState {
            candidates: vec![candidate("same"), candidate("same"), candidate("other")],
            decisions: vec![None; 3],
            cursor: 0,
            mode: Mode::Review,
        };
        let key = |value| KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE);

        assert_eq!(state.handle(key('g')), StateResult::Continue);
        assert_eq!(state.decisions.first(), Some(&Some(Decision::Accepted(0))));
        assert_eq!(state.handle(key('g')), StateResult::Continue);
        assert_eq!(state.decisions.first(), Some(&Some(Decision::Accepted(1))));

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
                .decisions
                .iter()
                .all(|decision| matches!(decision, Some(Decision::Accepted(0))))
        );
        state.handle(key('d'));
        assert!(
            state
                .decisions
                .iter()
                .all(|decision| *decision == Some(Decision::Rejected))
        );

        state.handle(key('?'));
        assert_eq!(state.mode, Mode::Help);
        state.handle(key('n'));
        assert_eq!(state.mode, Mode::Review);
        assert_eq!(state.handle(key('q')), StateResult::ExitWithoutChanges);
    }

    #[test]
    fn test_backend_renders_all_review_regions() -> Result<(), Box<dyn Error>> {
        let raw = "一ヶ月\n".as_bytes().to_vec();
        let documents = vec![Document {
            label: "work.txt".to_owned(),
            path: None,
            report: run_submission_with_orthography(&raw, Orthography::Mixed),
            raw,
        }];
        let state = ReviewState::new(&documents);
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend)?;

        terminal.draw(|frame| draw(frame, &state, &documents))?;

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
        Ok(())
    }
}
