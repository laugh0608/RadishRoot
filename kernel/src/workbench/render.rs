use super::*;
use crate::{input_view::style, surface::Surface};
use alloc::vec::Vec;
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::Rgb888,
    prelude::*,
    primitives::Rectangle,
    text::Text as Label,
};

pub struct Layout {
    pub files: Option<Rectangle>,
    pub terminal: Option<Rectangle>,
    pub output: Rectangle,
    pub input: Rectangle,
    pub detail: Option<Rectangle>,
    pub controls: Vec<(Focus, Rectangle)>,
    pub cw: u32,
    pub row: u32,
}
impl Layout {
    pub fn new(view: &View) -> Self {
        let g = view.geometry;
        let (w, h) = (g.width(), g.height());
        let cw = if w >= 800 && h >= 600 { 10 } else { 6 };
        let row = if cw == 10 { 24 } else { 14 };
        let top = row * 3 + 8;
        let detail_height = if view.details.is_some() {
            (h / 3).max(row * 5)
        } else {
            0
        };
        let height = h.saturating_sub(top + row + 12 + detail_height);
        let left_width = if view.single { w - 16 } else { w * 2 / 5 - 12 };
        let files = (!view.single || !view.terminal).then(|| rect(8, top, left_width, height));
        let terminal = (!view.single || view.terminal).then(|| {
            let x = if view.single { 8 } else { w * 2 / 5 + 4 };
            rect(x, top, w - x - 8, height)
        });
        let mut controls = Vec::new();
        controls.push((Focus::Panels, rect(w - cw * 10 - 8, 6, cw * 10, row)));
        if let Some(f) = files {
            let (x, y, fw, fh) = parts(f);
            controls.extend([
                (Focus::Refresh, rect(x + 6, y + row + 4, cw * 9, row)),
                (
                    Focus::NextPage,
                    rect(x + cw * 10 + 8, y + row + 4, cw * 6, row),
                ),
                (
                    Focus::Files,
                    rect(x + 6, y + row * 3, fw - 12, fh.saturating_sub(row * 5)),
                ),
                (Focus::FilesUp, rect(x + 6, y + fh - row - 4, cw * 7, row)),
                (
                    Focus::FilesDown,
                    rect(x + cw * 8 + 8, y + fh - row - 4, cw * 7, row),
                ),
            ]);
        }
        let mut output = rect(0, 0, 1, 1);
        let mut input = rect(0, 0, 1, 1);
        if let Some(t) = terminal {
            let (x, y, tw, th) = parts(t);
            output = rect(x + 8, y + row * 3, tw - 16, th.saturating_sub(row * 6 + 8));
            input = rect(x + 8, y + th - row * 2 - 4, tw - 16, row * 2);
            controls.extend([
                (Focus::PreviousResult, rect(x + 6, y + row + 4, cw * 6, row)),
                (
                    Focus::NextResult,
                    rect(x + cw * 7 + 6, y + row + 4, cw * 6, row),
                ),
                (
                    Focus::Details,
                    rect(x + cw * 14 + 6, y + row + 4, cw * 9, row),
                ),
                (
                    Focus::OutputUp,
                    rect(x + 6, y + th - row * 3 - 6, cw * 7, row),
                ),
                (
                    Focus::OutputDown,
                    rect(x + cw * 8 + 6, y + th - row * 3 - 6, cw * 7, row),
                ),
                (Focus::Input, input),
            ]);
        }
        let detail = (detail_height > 0)
            .then(|| rect(8, h - row - detail_height - 8, w - 16, detail_height));
        if let Some(d) = detail {
            let (x, y, dw, _) = parts(d);
            controls.extend([
                (
                    Focus::CloseDetails,
                    rect(x + dw - cw * 8 - 6, y + 4, cw * 8, row),
                ),
                (
                    Focus::DetailsUp,
                    rect(x + dw - cw * 25 - 6, y + 4, cw * 7, row),
                ),
                (
                    Focus::DetailsDown,
                    rect(x + dw - cw * 17 - 6, y + 4, cw * 8, row),
                ),
            ]);
        }
        Self {
            files,
            terminal,
            output,
            input,
            detail,
            controls,
            cw,
            row,
        }
    }
    pub fn hit(&self, point: Point) -> Option<Focus> {
        self.controls
            .iter()
            .rev()
            .find(|(_, r)| r.contains(point))
            .map(|(f, _)| *f)
    }
    pub fn file_rows(&self) -> usize {
        self.controls
            .iter()
            .find(|(f, _)| *f == Focus::Files)
            .map_or(1, |(_, r)| (r.size.height / self.row).max(1) as usize)
    }
    pub fn output_columns(&self) -> usize {
        (self.output.size.width / self.cw).max(1) as usize
    }
    pub fn detail_columns(&self) -> usize {
        self.detail
            .map_or(1, |r| ((r.size.width - 16) / self.cw).max(1)) as usize
    }
}
fn rect(x: u32, y: u32, w: u32, h: u32) -> Rectangle {
    Rectangle::new(Point::new(x as i32, y as i32), Size::new(w, h))
}
fn parts(r: Rectangle) -> (u32, u32, u32, u32) {
    (
        r.top_left.x as u32,
        r.top_left.y as u32,
        r.size.width,
        r.size.height,
    )
}
fn fill(s: &mut Surface<'_>, r: Rectangle, c: Rgb888) {
    s.rectangle(r.top_left, r.size, c);
}
fn label(s: &mut Surface<'_>, area: Rectangle, text: &str, c: Rgb888, cw: u32) {
    let font = if cw == 10 { &FONT_10X20 } else { &FONT_6X10 };
    // 每个控件使用自己的裁剪目标，长内容不能越过控件或覆盖其他面板。
    let mut target = s.clipped(&area);
    let display = text
        .chars()
        .map(|c| if (' '..='~').contains(&c) { c } else { '?' })
        .collect::<String>();
    let _ = Label::with_baseline(
        &display,
        area.top_left,
        MonoTextStyle::new(font, c),
        embedded_graphics::text::Baseline::Top,
    )
    .draw(&mut target);
}
fn frame(s: &mut Surface<'_>, r: Rectangle, focus: bool) {
    fill(s, r, if focus { style::FOCUS } else { style::BORDER });
    fill(
        s,
        Rectangle::new(r.top_left + Point::new(2, 2), r.size - Size::new(4, 4)),
        style::SURFACE,
    );
}
fn lines(s: &mut Surface<'_>, r: Rectangle, text: &Text, scroll: usize, l: &Layout) {
    for (i, line) in text
        .lines
        .iter()
        .skip(scroll)
        .take((r.size.height / l.row) as usize)
        .enumerate()
    {
        label(
            s,
            Rectangle::new(
                r.top_left + Point::new(0, (i as u32 * l.row) as i32),
                Size::new(r.size.width, l.row),
            ),
            line,
            style::TEXT,
            l.cw,
        );
    }
}
impl View {
    pub fn render(&mut self, s: &mut Surface<'_>) {
        let l = Layout::new(self);
        let w = self.geometry.width();
        let h = self.geometry.height();
        let _ = s.clear(style::BACKGROUND);
        label(
            s,
            rect(12, 8, w - l.cw * 12, l.row),
            "RadishRoot Workbench",
            style::TEXT,
            l.cw,
        );
        let volume = self.volume.map_or(String::from("Volume: loading"), |v| {
            format!(
                "/data  {:?}  {}  Power: {}",
                v.mount_state,
                if !matches!(self.power, services::power::State::Running) {
                    "WRITES BLOCKED"
                } else if v.read_only {
                    "READ ONLY"
                } else {
                    "WRITABLE"
                },
                self.power.label()
            )
        });
        label(
            s,
            rect(12, l.row + 12, w - 24, l.row),
            &volume,
            style::MUTED,
            l.cw,
        );
        let status = match self.notice {
            Notice::Ready => "Ready. Click / Tab: focus; Enter: activate; Esc: cancel.",
            Notice::Executing => "Executing bounded service request...",
            Notice::InputReset => "Input lost: draft cancelled. Release keys before continuing.",
            Notice::InputFull => "Input rejected: line limit. Enter / Esc clears this line.",
            Notice::InputUnavailable => "Input unavailable. See serial diagnostics.",
            Notice::IdentityExhausted => "FAILED: identity exhausted. No further command executed.",
        };
        let power_error = self.power_error.as_ref().map(|e| {
            format!(
                "Power FAILED {:?} at {:?}; writes blocked. Open original operation details.",
                e.code, e.stage
            )
        });
        label(
            s,
            rect(12, l.row * 2 + 12, w - 24, l.row),
            power_error.as_deref().unwrap_or(status),
            if power_error.is_none() && matches!(self.notice, Notice::Ready | Notice::Executing) {
                style::MUTED
            } else {
                style::ERROR
            },
            l.cw,
        );
        if let Some(f) = l.files {
            frame(
                s,
                f,
                matches!(
                    self.focus,
                    Focus::Files
                        | Focus::Refresh
                        | Focus::NextPage
                        | Focus::FilesUp
                        | Focus::FilesDown
                ),
            );
            let (x, y, fw, fh) = parts(f);
            label(
                s,
                rect(x + 8, y + 4, fw - 16, l.row),
                &format!("Files /data [{}]", self.update.label()),
                if matches!(
                    self.update,
                    UpdateState::Failed(_) | UpdateState::Unstable | UpdateState::ScanLimit
                ) {
                    style::ERROR
                } else {
                    style::TEXT
                },
                l.cw,
            );
            let body = l
                .controls
                .iter()
                .find(|(f, _)| *f == Focus::Files)
                .unwrap()
                .1;
            match &self.page {
                Err(e) => {
                    let mut t = Text::new((body.size.width / l.cw) as usize);
                    let _ = write!(t, "FAILED {e:?}\nUse Refresh to retry.");
                    lines(s, body, &t, 0, &l);
                }
                Ok(_) if self.list_operation.is_none() => {
                    label(s, body, "Loading...", style::MUTED, l.cw)
                }
                Ok(p) if p.entries.is_empty() => {
                    label(s, body, "Empty directory", style::MUTED, l.cw)
                }
                Ok(p) => {
                    for (i, e) in p
                        .entries
                        .iter()
                        .enumerate()
                        .skip(self.file_scroll)
                        .take(l.file_rows())
                    {
                        let r = Rectangle::new(
                            body.top_left
                                + Point::new(0, ((i - self.file_scroll) as u32 * l.row) as i32),
                            Size::new(body.size.width, l.row),
                        );
                        if i == self.file_row {
                            fill(s, r, style::BACKGROUND);
                        }
                        let columns = (r.size.width / l.cw) as usize;
                        let name_width = columns.saturating_sub(15);
                        let mut name = e.name.chars().take(name_width).collect::<String>();
                        if e.name.chars().count() > name_width {
                            name.pop();
                            name.push('~');
                        }
                        let size = e.size_bytes.map_or(String::from("-"), |n| format!("{n}"));
                        label(
                            s,
                            r,
                            &format!(
                                "{name:<name_width$} {} {size}",
                                if e.kind == FileKind::Directory {
                                    "dir"
                                } else {
                                    "file"
                                }
                            ),
                            style::TEXT,
                            l.cw,
                        );
                    }
                }
            }
            label(
                s,
                rect(x + 8, y + fh - l.row * 2 - 4, fw - 16, l.row),
                &format!(
                    "Snapshot gen={}  more={}",
                    self.list_generation,
                    self.page.as_ref().is_ok_and(|p| p.next.is_some())
                ),
                style::MUTED,
                l.cw,
            );
        }
        if let Some(t) = l.terminal {
            frame(
                s,
                t,
                matches!(
                    self.focus,
                    Focus::Input
                        | Focus::PreviousResult
                        | Focus::NextResult
                        | Focus::OutputUp
                        | Focus::OutputDown
                        | Focus::Details
                ),
            );
            let (x, y, tw, _) = parts(t);
            label(
                s,
                rect(x + 8, y + 4, tw - 16, l.row),
                &format!(
                    "Terminal {}/{}  evicted={}",
                    if self.results.is_empty() {
                        0
                    } else {
                        self.selected + 1
                    },
                    self.results.len(),
                    self.evicted
                ),
                style::TEXT,
                l.cw,
            );
            let text = self.output_text();
            lines(s, l.output, &text, self.output_scroll, &l);
            let prefix = &self.editor.text()[..self.editor.cursor()];
            let cursor = prefix.chars().count();
            let width = (l.input.size.width / l.cw).saturating_sub(3) as usize;
            let start = cursor.saturating_sub(width.saturating_sub(1));
            let display = self
                .editor
                .text()
                .chars()
                .skip(start)
                .take(width)
                .collect::<String>();
            fill(s, l.input, style::BACKGROUND);
            label(s, l.input, &format!("> {display}"), style::TEXT, l.cw);
            if self.focus == Focus::Input {
                fill(
                    s,
                    rect(
                        l.input.top_left.x as u32 + (2 + cursor - start) as u32 * l.cw,
                        l.input.top_left.y as u32 + l.row - 3,
                        l.cw,
                        2,
                    ),
                    style::FOCUS,
                );
            }
            label(
                s,
                Rectangle::new(
                    l.input.top_left + Point::new(0, l.row as i32),
                    Size::new(l.input.size.width, l.row),
                ),
                &format!(
                    "out:{}{} glyph:{} history-drop:{}",
                    self.output_scroll,
                    if text.truncated { " DISPLAY LIMIT" } else { "" },
                    if text.missing_glyphs { "?" } else { "ASCII" },
                    self.editor.evicted
                ),
                style::MUTED,
                l.cw,
            );
        }
        if let Some(d) = l.detail {
            frame(s, d, true);
            let (x, y, dw, dh) = parts(d);
            label(
                s,
                rect(x + 8, y + 4, dw.saturating_sub(l.cw * 27), l.row),
                "Operation details",
                style::TEXT,
                l.cw,
            );
            let text = self.detail_text();
            lines(
                s,
                rect(x + 8, y + l.row + 8, dw - 16, dh - l.row - 12),
                &text,
                self.detail_scroll,
                &l,
            );
        }
        for (focus, r) in &l.controls {
            if matches!(focus, Focus::Files | Focus::Input) {
                continue;
            }
            fill(
                s,
                *r,
                if *focus == self.focus {
                    style::FOCUS
                } else {
                    style::BACKGROUND
                },
            );
            let name = match focus {
                Focus::Refresh => "Refresh",
                Focus::NextPage => "Next",
                Focus::FilesUp | Focus::OutputUp | Focus::DetailsUp => "Up",
                Focus::FilesDown | Focus::OutputDown | Focus::DetailsDown => "Down",
                Focus::PreviousResult => "Prev",
                Focus::NextResult => "Next",
                Focus::Details => "Details",
                Focus::Panels => "Panels",
                Focus::CloseDetails => "Close",
                _ => "",
            };
            label(
                s,
                *r,
                name,
                if *focus == self.focus {
                    style::SURFACE
                } else {
                    style::TEXT
                },
                l.cw,
            );
        }
        let update = match self.update {
            UpdateState::Idle => String::from(
                "Names: ASCII glyphs; '?' preserves original UTF-8. Lists follow service changes.",
            ),
            UpdateState::Refreshing(UpdateReason::MissedEvents)
            | UpdateState::Refreshed(UpdateReason::MissedEvents) => String::from(
                "Change events missed: directory re-queried; unknown targets are not inferred.",
            ),
            UpdateState::Refreshing(UpdateReason::NewService)
            | UpdateState::Refreshed(UpdateReason::NewService) => String::from(
                "Service identity changed: directory re-queried; old results keep their original IDs.",
            ),
            UpdateState::Refreshing(_) => String::from(
                "Refreshing directory in bounded pages. You can continue editing the command.",
            ),
            UpdateState::Refreshed(UpdateReason::Operation(id)) => format!(
                "List source {:032x}:{}; Enter in Files opens that operation.",
                u128::from_be_bytes(id.boot.0),
                id.sequence
            ),
            UpdateState::Refreshed(_) => {
                String::from("Directory re-queried from the shared service.")
            }
            UpdateState::Failed(_) => String::from(
                "Directory refresh FAILED. Original command result is unchanged; see operation details.",
            ),
            UpdateState::Unstable => String::from(
                "Directory keeps changing: automatic refresh stopped after 4 restarts. Use Refresh.",
            ),
            UpdateState::ScanLimit => String::from(
                "Directory scan limit reached: target not located. Use Refresh; no entry was invented.",
            ),
        };
        label(
            s,
            rect(12, h - l.row - 4, w - 24, l.row),
            &update,
            style::MUTED,
            l.cw,
        );
        for y in 0..12 {
            for x in 0..=y / 2 {
                s.pixel(self.pointer + Point::new(x, y), style::TEXT);
            }
        }
        self.dirty = false;
    }
}
