//! 文件区和图形终端共享状态；只经 FileService / Shell 执行领域请求。
mod changes;
mod render;
pub mod replay;
mod text;
use crate::{
    input::{Event, KeyCode, Modifiers, Phase},
    services::{self, *},
    shell::{
        self, Shell,
        editor::{Edit, Editor},
    },
    surface::Geometry,
};
use alloc::{collections::VecDeque, format, string::String};
pub use changes::{UpdateReason, UpdateState};
use core::fmt::Write;
use embedded_graphics::prelude::Point;
pub use render::Layout;
use services::path::Path;
pub use text::Text;

pub const COMMANDS: usize = 16;
pub const RESULT_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Files,
    Refresh,
    NextPage,
    FilesUp,
    FilesDown,
    Input,
    PreviousResult,
    NextResult,
    OutputUp,
    OutputDown,
    Details,
    Panels,
    CloseDetails,
    DetailsUp,
    DetailsDown,
}
const FOCUS: [Focus; 15] = [
    Focus::Files,
    Focus::Refresh,
    Focus::NextPage,
    Focus::FilesUp,
    Focus::FilesDown,
    Focus::Input,
    Focus::PreviousResult,
    Focus::NextResult,
    Focus::OutputUp,
    Focus::OutputDown,
    Focus::Details,
    Focus::Panels,
    Focus::CloseDetails,
    Focus::DetailsUp,
    Focus::DetailsDown,
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Ready,
    Executing,
    InputReset,
    InputFull,
    InputUnavailable,
    IdentityExhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intent {
    List(bool),
    Command,
    Details(OperationId),
}
pub struct CommandResult {
    pub line: String,
    pub reply: shell::Reply,
}
pub struct View {
    geometry: Geometry,
    pub editor: Editor,
    shell: Shell,
    pub page: Result<ListPage, shell::Error>,
    pub list_operation: Option<OperationId>,
    pub list_generation: u64,
    pub volume: Option<VolumeInfo>,
    pub results: VecDeque<CommandResult>,
    pub evicted: u64,
    pub selected: usize,
    pub file_row: usize,
    pub file_scroll: usize,
    pub output_scroll: usize,
    pub detail_scroll: usize,
    pub details: Option<shell::Reply>,
    pub focus: Focus,
    pub notice: Notice,
    pub pointer: Point,
    pub left: bool,
    pub modifiers: Modifiers,
    pressed: Option<Focus>,
    pending: Option<Intent>,
    submitted: Option<String>,
    input_rejected: bool,
    pub events: u64,
    pub releases: u64,
    pub button_releases: u64,
    pub resets: u64,
    pub single: bool,
    pub terminal: bool,
    dirty: bool,
    subscription: Option<changes::Subscription>,
    refresh: Option<changes::Refresh>,
    pub update: UpdateState,
    pub change_operation: Option<OperationId>,
}
impl View {
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            editor: Editor::new(),
            shell: Shell::new(),
            page: Ok(ListPage {
                entries: alloc::vec![],
                next: None,
            }),
            list_operation: None,
            list_generation: 0,
            volume: None,
            results: VecDeque::new(),
            evicted: 0,
            selected: 0,
            file_row: 0,
            file_scroll: 0,
            output_scroll: 0,
            detail_scroll: 0,
            details: None,
            focus: Focus::Input,
            notice: Notice::Executing,
            pointer: Point::new(geometry.width() as i32 / 2, geometry.height() as i32 / 2),
            left: false,
            modifiers: Modifiers::default(),
            pressed: None,
            pending: Some(Intent::List(false)),
            submitted: None,
            input_rejected: false,
            events: 0,
            releases: 0,
            button_releases: 0,
            resets: 0,
            single: geometry.width() < 800 || geometry.height() < 600,
            terminal: true,
            dirty: true,
            subscription: None,
            refresh: None,
            update: UpdateState::Idle,
            change_operation: None,
        }
    }
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some() || self.refresh.is_some()
    }
    pub fn unavailable(&mut self) {
        self.event(Event::InputReset);
        self.notice = Notice::InputUnavailable;
    }
    fn schedule(&mut self, intent: Intent) {
        if self.pending.is_none() {
            self.pending = Some(intent);
            self.notice = Notice::Executing;
            self.dirty = true;
        }
    }
    /// 调用者必须先绘制 Executing；每轮至多执行一个已有服务界限内的请求。
    pub fn execute_pending<B: Backend>(&mut self, files: &mut FileService<B>) {
        let Some(intent) = self.pending.take() else {
            self.refresh_step(files);
            self.observe_changes(files);
            return;
        };
        self.notice = Notice::Ready;
        match intent {
            Intent::List(next) => {
                self.refresh = None;
                self.update = UpdateState::Idle;
                self.change_operation = None;
                let cursor = if next {
                    self.page.as_ref().ok().and_then(|p| p.next.as_ref())
                } else {
                    None
                };
                let reply = files.execute(
                    &Path::root(),
                    Request::List {
                        path: "/data",
                        cursor,
                    },
                );
                self.page = match reply {
                    Ok(reply) => {
                        self.list_operation = Some(reply.operation_id);
                        reply
                            .result
                            .map(|output| match output {
                                Output::List(page) => page,
                                _ => unreachable!("list service reply"),
                            })
                            .map_err(shell::Error::Service)
                    }
                    Err(code) => {
                        self.list_operation = None;
                        Err(shell::Error::ServiceIdentity(code))
                    }
                };
                self.list_generation = files.generation();
                self.file_row = 0;
                self.file_scroll = 0;
            }
            Intent::Command => {
                let line = self.submitted.take().expect("submitted command intent");
                match self.shell.execute(&line, files) {
                    Ok(reply) => {
                        // 服务边界限制单个结果；最多保留 16 个结果，旧快照不会被刷新改写。
                        while self.results.len() == COMMANDS
                            || self.results.iter().map(result_bytes).sum::<usize>()
                                + reply_bytes(&reply)
                                + line.capacity()
                                > RESULT_BYTES
                        {
                            if self.results.is_empty() {
                                break;
                            }
                            self.results.pop_front();
                            self.evicted = self.evicted.saturating_add(1);
                        }
                        self.results.push_back(CommandResult { line, reply });
                        self.selected = self.results.len() - 1;
                        self.output_scroll = 0;
                    }
                    Err(_) => self.notice = Notice::IdentityExhausted,
                }
            }
            Intent::Details(id) => {
                self.details = Some(
                    match self.shell.execute(
                        &format!(
                            "explain {:032x}:{}",
                            u128::from_be_bytes(id.boot.0),
                            id.sequence
                        ),
                        files,
                    ) {
                        Ok(reply) => reply,
                        Err(code) => shell::Reply {
                            command_id: 0,
                            operation_id: None,
                            result: Err(shell::Error::ServiceIdentity(code)),
                        },
                    },
                );
                self.detail_scroll = 0;
                self.focus = Focus::CloseDetails;
            }
        }
        self.volume = Some(files.volume());
        self.observe_changes(files);
        self.dirty = true;
    }
    fn visible(&self, focus: Focus) -> bool {
        Layout::new(self).controls.iter().any(|(f, _)| *f == focus)
    }
    fn tab(&mut self, backwards: bool) {
        let start = FOCUS.iter().position(|f| *f == self.focus).unwrap();
        for step in 1..=FOCUS.len() {
            let at = if backwards {
                (start + FOCUS.len() - step) % FOCUS.len()
            } else {
                (start + step) % FOCUS.len()
            };
            if self.visible(FOCUS[at]) {
                self.focus = FOCUS[at];
                break;
            }
        }
    }
    fn activate(&mut self, focus: Focus) {
        if self.pending.is_some() {
            return;
        }
        match focus {
            Focus::Refresh => self.schedule(Intent::List(false)),
            Focus::NextPage => {
                if self.refresh.is_none() && self.page.as_ref().is_ok_and(|p| p.next.is_some()) {
                    self.schedule(Intent::List(true));
                }
            }
            Focus::Input => {
                if self.input_rejected {
                    let _ = self.editor.apply(Edit::Cancel);
                    self.input_rejected = false;
                    return;
                }
                if let Ok(Some(line)) = self.editor.apply(Edit::Submit)
                    && !line.trim().is_empty()
                {
                    self.submitted = Some(line);
                    self.schedule(Intent::Command);
                }
            }
            Focus::Files | Focus::Details => {
                let id = if focus == Focus::Files {
                    self.change_operation.or(self.list_operation)
                } else {
                    self.results
                        .get(self.selected)
                        .and_then(|r| match &r.reply.result {
                            Ok(shell::Output::Explanation { record, .. }) => Some(record.id),
                            _ => r.reply.operation_id,
                        })
                };
                if let Some(id) = id {
                    self.schedule(Intent::Details(id));
                }
            }
            Focus::PreviousResult => {
                self.selected = self.selected.saturating_sub(1);
                self.output_scroll = 0;
            }
            Focus::NextResult => {
                self.selected = (self.selected + 1).min(self.results.len().saturating_sub(1));
                self.output_scroll = 0;
            }
            Focus::FilesUp => self.move_file(false),
            Focus::FilesDown => self.move_file(true),
            Focus::OutputUp => self.output_scroll = self.output_scroll.saturating_sub(5),
            Focus::OutputDown => {
                self.output_scroll =
                    (self.output_scroll + 5).min(self.output_text().lines.len().saturating_sub(1))
            }
            Focus::DetailsUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            Focus::DetailsDown => {
                self.detail_scroll =
                    (self.detail_scroll + 5).min(self.detail_text().lines.len().saturating_sub(1))
            }
            Focus::CloseDetails => {
                self.details = None;
                self.focus = Focus::Input;
            }
            Focus::Panels => {
                if self.geometry.width() >= 800 && self.geometry.height() >= 600 {
                    if !self.single {
                        self.single = true;
                        self.terminal = true;
                    } else if self.terminal {
                        self.terminal = false;
                    } else {
                        self.single = false;
                    }
                } else {
                    self.terminal = !self.terminal;
                }
                self.focus = if self.terminal {
                    Focus::Input
                } else {
                    Focus::Files
                };
            }
        }
    }
    fn move_file(&mut self, down: bool) {
        let n = self.page.as_ref().map_or(0, |p| p.entries.len());
        self.file_row = if down {
            (self.file_row + 1).min(n.saturating_sub(1))
        } else {
            self.file_row.saturating_sub(1)
        };
        let rows = Layout::new(self).file_rows();
        if self.file_row < self.file_scroll {
            self.file_scroll = self.file_row;
        }
        if self.file_row >= self.file_scroll + rows {
            self.file_scroll = self.file_row + 1 - rows;
        }
    }
    pub fn event(&mut self, event: Event) {
        self.events = self.events.saturating_add(1);
        self.dirty = true;
        match event {
            Event::InputReset => {
                self.input_rejected = false;
                let _ = self.editor.apply(Edit::Cancel);
                if self.pending == Some(Intent::Command) {
                    self.pending = None;
                    self.submitted = None;
                }
                self.left = false;
                self.pressed = None;
                self.modifiers = Modifiers::default();
                self.notice = Notice::InputReset;
                self.resets = self.resets.saturating_add(1);
            }
            Event::PointerMove { dx, dy } => {
                self.pointer.x =
                    (self.pointer.x + i32::from(dx)).clamp(0, self.geometry.width() as i32 - 1);
                self.pointer.y =
                    (self.pointer.y + i32::from(dy)).clamp(0, self.geometry.height() as i32 - 1);
            }
            Event::PointerButton { down } => {
                let hit = Layout::new(self).hit(self.pointer);
                if down && !self.left {
                    self.pressed = hit;
                    if let Some(f) = hit {
                        self.focus = f;
                    }
                }
                if !down && self.left {
                    self.button_releases = self.button_releases.saturating_add(1);
                    if hit == self.pressed
                        && let Some(f) = hit
                    {
                        self.activate(f);
                    }
                    self.pressed = None;
                }
                self.left = down;
            }
            Event::Text(c)
                if self.focus == Focus::Input && self.pending.is_none() && !self.input_rejected =>
            {
                if self.editor.apply(Edit::Insert(c)).is_err() {
                    self.notice = Notice::InputFull;
                    self.input_rejected = true;
                }
            }
            Event::Key {
                code,
                phase,
                modifiers,
            } => {
                self.modifiers = modifiers;
                if phase == Phase::Release {
                    self.releases = self.releases.saturating_add(1);
                    return;
                }
                match code {
                    KeyCode::Tab => self.tab(modifiers.shift),
                    KeyCode::Escape => {
                        self.input_rejected = false;
                        let _ = self.editor.apply(Edit::Cancel);
                        self.details = None;
                        self.focus = Focus::Input;
                    }
                    KeyCode::Return | KeyCode::NumpadEnter => self.activate(self.focus),
                    KeyCode::ArrowUp if self.focus == Focus::Files => self.move_file(false),
                    KeyCode::ArrowDown if self.focus == Focus::Files => self.move_file(true),
                    _ if self.focus == Focus::Input && self.pending.is_none() => {
                        let edit = match code {
                            KeyCode::Backspace => Some(Edit::Backspace),
                            KeyCode::Delete => Some(Edit::Delete),
                            KeyCode::ArrowLeft => Some(Edit::Left),
                            KeyCode::ArrowRight => Some(Edit::Right),
                            KeyCode::Home => Some(Edit::Home),
                            KeyCode::End => Some(Edit::End),
                            KeyCode::ArrowUp => Some(Edit::Previous),
                            KeyCode::ArrowDown => Some(Edit::Next),
                            KeyCode::C if modifiers.control => Some(Edit::Cancel),
                            _ => None,
                        };
                        if let Some(edit) = edit {
                            if edit == Edit::Cancel {
                                self.input_rejected = false;
                            }
                            let _ = self.editor.apply(edit);
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }
    pub fn output_text(&self) -> Text {
        let layout = Layout::new(self);
        let mut text = Text::new(layout.output_columns());
        if let Some(r) = self.results.get(self.selected) {
            let _ = writeln!(text, "> {}", r.line);
            describe(&mut text, &r.reply);
        } else {
            let _ = writeln!(
                text,
                "Type help, then Enter.\nBuilt-in commands; /data is shared."
            );
        }
        text
    }
    pub fn detail_text(&self) -> Text {
        let layout = Layout::new(self);
        let mut text = Text::new(layout.detail_columns());
        if let Some(reply) = &self.details {
            describe(&mut text, reply);
        }
        text
    }
}
fn describe(text: &mut Text, reply: &shell::Reply) {
    if let Some(id) = reply.operation_id {
        let _ = writeln!(
            text,
            "Operation {:032x}:{}",
            u128::from_be_bytes(id.boot.0),
            id.sequence
        );
    }
    match &reply.result {
        Err(shell::Error::Service(e)) => {
            let _ = writeln!(
                text,
                "FAILED {:?} at {:?}\nTarget: {:?}\nCause: {:?}\nEffects: {:?}",
                e.code,
                e.stage,
                e.object.as_ref().map(Path::as_str),
                e.cause,
                e.partial_effect
            );
        }
        Err(e) => {
            let _ = writeln!(text, "FAILED {e:?}");
        }
        Ok(shell::Output::Help) => {
            for c in shell::parser::COMMANDS {
                let _ = writeln!(text, "{}", c.usage);
            }
        }
        Ok(shell::Output::Files(page)) => {
            let _ = writeln!(
                text,
                "Files: {}  more={}",
                page.entries.len(),
                page.next.is_some()
            );
            for e in &page.entries {
                let _ = writeln!(text, "{}  {:?}  {:?}", e.name, e.kind, e.size_bytes);
            }
        }
        Ok(shell::Output::Bytes { bytes, eof, offset }) => {
            let _ = writeln!(text, "Bytes: {} offset={offset} eof={eof}", bytes.len());
            if let Ok(s) = core::str::from_utf8(bytes) {
                let _ = writeln!(text, "Text (display projection):\n{s}");
            }
            let _ = writeln!(text, "Raw hex:");
            for chunk in bytes.chunks(16) {
                for b in chunk {
                    let _ = write!(text, "{b:02x} ");
                }
                let _ = writeln!(text);
            }
        }
        Ok(shell::Output::Created(r)) => {
            let _ = writeln!(
                text,
                "SUCCEEDED: {}\nAll writeback stages confirmed.",
                r.path.as_str()
            );
        }
        Ok(shell::Output::Operations { records, evicted }) => {
            let _ = writeln!(text, "Operations (evicted={evicted}):");
            for r in records {
                let _ = writeln!(
                    text,
                    "{:032x}:{} {:?} {:?} {:?}",
                    u128::from_be_bytes(r.id.boot.0),
                    r.id.sequence,
                    r.status,
                    r.stage,
                    r.target.as_ref().map(Path::as_str)
                );
            }
        }
        Ok(shell::Output::Explanation {
            record,
            events,
            events_evicted,
        }) => {
            let _ = writeln!(
                text,
                "Operation {:032x}:{}\n{:?} at {:?}; last={:?}\nTarget: {:?}\nEffects: {:?}\nError: {:?}\nEvents evicted: {events_evicted}",
                u128::from_be_bytes(record.id.boot.0),
                record.id.sequence,
                record.status,
                record.stage,
                record.last_completed,
                record.target.as_ref().map(Path::as_str),
                record.partial_effect,
                record.error
            );
            for e in events {
                let _ = writeln!(text, "{} {:?} {:?}", e.seq, e.stage, e.status);
            }
        }
    }
}

// 将字符串按最多双倍分配保守计入；Vec 元素容量和内联结构另计，避免只按条数限额。
fn record_bytes(r: &OperationRecord) -> usize {
    r.target.as_ref().map_or(0, |p| p.as_str().len() * 2)
        + r.error
            .as_ref()
            .and_then(|e| e.object.as_ref())
            .map_or(0, |p| p.as_str().len() * 2)
}
fn reply_bytes(r: &shell::Reply) -> usize {
    core::mem::size_of::<shell::Reply>()
        + match &r.result {
            Ok(shell::Output::Files(p)) => {
                p.entries.capacity() * core::mem::size_of::<FileEntry>()
                    + p.entries
                        .iter()
                        .map(|e| e.name.capacity() + e.reference.path.as_str().len() * 2)
                        .sum::<usize>()
                    + p.next
                        .as_ref()
                        .map_or(0, |c| c.directory.path.as_str().len() * 2)
            }
            Ok(shell::Output::Bytes { bytes, .. }) => bytes.capacity(),
            Ok(shell::Output::Created(r)) => r.path.as_str().len() * 2,
            Ok(shell::Output::Operations { records, .. }) => {
                records.capacity() * core::mem::size_of::<OperationRecord>()
                    + records.iter().map(record_bytes).sum::<usize>()
            }
            Ok(shell::Output::Explanation { record, events, .. }) => {
                record_bytes(record) + events.capacity() * core::mem::size_of::<OperationEvent>()
            }
            Err(shell::Error::Service(e)) => e.object.as_ref().map_or(0, |p| p.as_str().len() * 2),
            _ => 0,
        }
}
fn result_bytes(r: &CommandResult) -> usize {
    r.line.capacity() + reply_bytes(&r.reply)
}

#[cfg(test)]
mod tests;
