use super::*;
use crate::{
    services::{fat::FatBackend, tests::formatted},
    surface::Surface,
};
use alloc::{vec, vec::Vec};
fn key(view: &mut View, code: KeyCode) {
    view.event(Event::Key {
        code,
        phase: Phase::Press,
        modifiers: Modifiers::default(),
    });
    view.event(Event::Key {
        code,
        phase: Phase::Release,
        modifiers: Modifiers::default(),
    });
}
fn command<B: Backend>(view: &mut View, files: &mut FileService<B>, line: &str) {
    view.focus = Focus::Input;
    for c in line.chars() {
        view.event(Event::Text(c));
    }
    key(view, KeyCode::Return);
    assert_eq!(view.notice, Notice::Executing);
    view.execute_pending(files);
}
fn view() -> View {
    View::new(Geometry::new(1024, 768).unwrap())
}
#[test]
fn real_fat_list_and_graphical_commands_share_operation_and_conflict() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    assert!(v.page.as_ref().unwrap().entries.is_empty());
    command(&mut v, &mut files, "write 'note one.txt' payload");
    let id = v.results.back().unwrap().reply.operation_id.unwrap();
    assert!(matches!(
        v.results.back().unwrap().reply.result,
        Ok(shell::Output::Created(_))
    ));
    // T10 保留旧列表快照；显式刷新使用真实服务，而非手工填入新建项。
    assert!(v.page.as_ref().unwrap().entries.is_empty());
    v.activate(Focus::Refresh);
    v.execute_pending(&mut files);
    assert_eq!(v.page.as_ref().unwrap().entries[0].name, "note one.txt");
    v.activate(Focus::Details);
    v.execute_pending(&mut files);
    let Ok(shell::Output::Explanation { record, .. }) = &v.details.as_ref().unwrap().result else {
        panic!()
    };
    assert_eq!((record.id, record.status), (id, Status::Succeeded));
    assert!(record.partial_effect.persistence_confirmed);
    command(&mut v, &mut files, "write 'note one.txt' different");
    assert!(matches!(
        &v.results.back().unwrap().reply.result,
        Err(shell::Error::Service(SystemError {
            code: Code::AlreadyExists,
            ..
        }))
    ));
    command(&mut v, &mut files, "cat 'note one.txt'");
    assert!(
        matches!(&v.results.back().unwrap().reply.result,Ok(shell::Output::Bytes {bytes,..}) if bytes==b"payload")
    );
    let count = files.operations.iter().count();
    command(&mut v, &mut files, "write missing");
    assert!(v.results.back().unwrap().reply.operation_id.is_none());
    assert_eq!(files.operations.iter().count(), count);
}
#[test]
fn pages_remain_accessible_and_stale_cursor_is_visible() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    for i in 0..35 {
        files
            .execute(
                &Path::root(),
                Request::CreateText {
                    path: &format!("f{i:02}"),
                    text: "x",
                },
            )
            .unwrap()
            .result
            .unwrap();
    }
    let mut v = view();
    v.execute_pending(&mut files);
    assert_eq!(v.page.as_ref().unwrap().entries.len(), 32);
    for _ in 0..40 {
        v.activate(Focus::FilesDown);
    }
    assert_eq!(v.file_row, 31);
    assert!(v.file_scroll > 0);
    v.activate(Focus::NextPage);
    v.execute_pending(&mut files);
    assert_eq!(v.page.as_ref().unwrap().entries.len(), 3);
    assert!(v.page.as_ref().unwrap().next.is_none());
    v.activate(Focus::Refresh);
    v.execute_pending(&mut files);
    command(&mut v, &mut files, "write new x");
    v.activate(Focus::NextPage);
    v.execute_pending(&mut files);
    assert!(matches!(
        v.page,
        Err(shell::Error::Service(SystemError {
            code: Code::StaleReference,
            ..
        }))
    ));
    v.activate(Focus::Refresh);
    v.execute_pending(&mut files);
    assert!(v.page.is_ok());
}
#[test]
fn input_reset_cancels_pending_command_and_clicks_require_matching_release() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    for c in "write accidental x".chars() {
        v.event(Event::Text(c));
    }
    key(&mut v, KeyCode::Return);
    v.event(Event::InputReset);
    v.execute_pending(&mut files);
    assert!(v.results.is_empty());
    assert!(v.editor.text().is_empty());
    assert!(!v.pending());
    assert_eq!(v.modifiers, Modifiers::default());
    let r = Layout::new(&v)
        .controls
        .iter()
        .find(|(f, _)| *f == Focus::Refresh)
        .unwrap()
        .1;
    v.pointer = r.top_left;
    v.event(Event::PointerButton { down: true });
    v.pointer = Point::new(0, 0);
    v.event(Event::PointerButton { down: false });
    assert!(!v.pending());
    v.pointer = r.top_left;
    v.event(Event::PointerButton { down: true });
    v.event(Event::PointerButton { down: false });
    assert!(v.pending());
}
#[test]
fn rendering_handles_minimum_single_panels_and_long_original_bytes() {
    for (w, h) in [(320, 240), (800, 600), (1024, 768)] {
        let mut v = View::new(Geometry::new(w, h).unwrap());
        let mut pixels = vec![0; v.geometry.pixels()];
        v.render(&mut Surface::new(v.geometry, &mut pixels).unwrap());
        v.details = Some(shell::Reply {
            command_id: 1,
            operation_id: None,
            result: Ok(shell::Output::Bytes {
                bytes: vec![0xff; 4096],
                eof: false,
                offset: 0,
            }),
        });
        v.render(&mut Surface::new(v.geometry, &mut pixels).unwrap());
        assert!(!v.dirty());
        for _ in 0..FOCUS.len() * 2 {
            key(&mut v, KeyCode::Tab);
            assert!(v.visible(v.focus));
        }
    }
    let mut t = Text::new(10);
    let _ = write!(t, "萝卜\n{}", "x".repeat(100_000));
    assert!(t.missing_glyphs && t.truncated);
    assert_eq!(t.lines[0], "??");
    assert!(t.lines.len() <= text::ROWS);
    let bytes: Vec<u8> = vec![0, 0xff, 0xe4, 0xb8, 0xad];
    let mut v = view();
    v.results.push_back(CommandResult {
        line: String::from("cat raw"),
        reply: shell::Reply {
            command_id: 1,
            operation_id: None,
            result: Ok(shell::Output::Bytes {
                bytes: bytes.clone(),
                eof: true,
                offset: 0,
            }),
        },
    });
    assert!(v.output_text().lines.join("\n").contains("00 ff e4 b8 ad"));
    assert!(matches!(&v.results[0].reply.result,Ok(shell::Output::Bytes{bytes:b,..})if *b==bytes));
}

#[test]
fn readonly_and_failed_write_remain_failed_in_details_and_later_queries() {
    let (mut disk, mut layout) = formatted();
    layout.data_readonly = true;
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    command(&mut v, &mut files, "write denied x");
    assert!(matches!(
        &v.results.back().unwrap().reply.result,
        Err(shell::Error::Service(SystemError {
            code: Code::ReadOnly,
            ..
        }))
    ));
    v.activate(Focus::Details);
    v.execute_pending(&mut files);
    let Ok(shell::Output::Explanation { record, .. }) = &v.details.as_ref().unwrap().result else {
        panic!()
    };
    assert_eq!(record.status, Status::Failed);
    command(&mut v, &mut files, "ls");
    assert!(v.results.back().unwrap().reply.result.is_ok());
    // 在新服务直接提交写入；注入仍由既有 FAT 适配器消费，UI 不伪造失败。
    let (mut disk, layout) = formatted();
    let mut backend = FatBackend::new(&mut disk, layout);
    backend.inject(Stage::FileSync);
    let mut files = FileService::new(backend, BootId([1; 16])).unwrap();
    let mut v = view();
    v.pending = None;
    command(&mut v, &mut files, "write partial payload");
    let id = v.results.back().unwrap().reply.operation_id.unwrap();
    v.activate(Focus::Details);
    v.execute_pending(&mut files);
    let Ok(shell::Output::Explanation { record, .. }) = &v.details.as_ref().unwrap().result else {
        panic!()
    };
    assert_eq!(record.id, id);
    assert_eq!(record.status, Status::Failed);
    assert!(!record.partial_effect.persistence_confirmed);
    assert!(record.partial_effect.may_have_changed);
    v.details = None;
    v.activate(Focus::Refresh);
    v.execute_pending(&mut files);
    assert!(matches!(
        v.page,
        Err(shell::Error::Service(SystemError {
            code: Code::Offline,
            ..
        }))
    ));
}

#[test]
fn overflow_is_not_submitted_and_output_history_is_bounded() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    for c in "write shortened "
        .chars()
        .chain(core::iter::repeat_n('x', 1024))
    {
        v.event(Event::Text(c));
    }
    let count = files.operations.iter().count();
    key(&mut v, KeyCode::Return);
    v.execute_pending(&mut files);
    assert_eq!(files.operations.iter().count(), count);
    assert!(v.results.is_empty());
    for _ in 0..20 {
        command(&mut v, &mut files, "help");
    }
    assert_eq!((v.results.len(), v.evicted), (16, 4));
    assert!(v.results.iter().map(result_bytes).sum::<usize>() <= RESULT_BYTES);
    let before = v.selected;
    v.activate(Focus::PreviousResult);
    assert_eq!(v.selected, before - 1);
}
