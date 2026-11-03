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
fn settle<B: Backend>(v: &mut View, files: &mut FileService<B>) {
    for _ in 0..130 {
        if !v.pending() {
            return;
        }
        v.execute_pending(files);
    }
    panic!("bounded workbench refresh did not settle");
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
    // 写入与重新查询分轮执行；T11 无需手动刷新便显示服务返回的新项。
    assert!(v.page.as_ref().unwrap().entries.is_empty());
    settle(&mut v, &mut files);
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
    let old = v.page.as_ref().unwrap().next.clone().unwrap();
    command(&mut v, &mut files, "write new x");
    settle(&mut v, &mut files);
    assert!(
        v.page
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.name == "new")
    );
    // 自动查找使用新游标；原服务的旧游标拒绝契约仍保留，不修改代次冒充有效。
    assert!(matches!(
        files
            .execute(
                &Path::root(),
                Request::List {
                    path: "/data",
                    cursor: Some(&old)
                }
            )
            .unwrap()
            .result,
        Err(SystemError {
            code: Code::StaleReference,
            ..
        })
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

#[test]
fn automatic_refresh_locates_new_file_across_pages_without_rewriting_history_or_draft() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    for i in 0..201 {
        files
            .execute(
                &Path::root(),
                Request::CreateText {
                    path: &format!("p{i:03}.txt"),
                    text: "seed",
                },
            )
            .unwrap()
            .result
            .unwrap();
    }
    let mut v = view();
    v.execute_pending(&mut files);
    command(&mut v, &mut files, "ls");
    let mut snapshot = String::new();
    shell::render::reply(&mut snapshot, &v.results[0].reply).unwrap();
    command(&mut v, &mut files, "write newest.txt payload");
    let id = v.results.back().unwrap().reply.operation_id.unwrap();
    assert!(matches!(v.update,UpdateState::Refreshing(UpdateReason::Operation(op))if op==id));
    v.event(Event::Text('h'));
    assert_eq!(v.editor.text(), "h");
    let mut pages = 0;
    while v.pending() {
        v.execute_pending(&mut files);
        pages += 1;
        assert!(pages <= 7);
    }
    assert_eq!(pages, 7);
    assert_eq!(
        v.page.as_ref().unwrap().entries[v.file_row].name,
        "newest.txt"
    );
    assert_eq!(v.change_operation, Some(id));
    assert_eq!(v.editor.text(), "h");
    let mut after = String::new();
    shell::render::reply(&mut after, &v.results[0].reply).unwrap();
    assert_eq!(snapshot, after);
    v.activate(Focus::Files);
    v.execute_pending(&mut files);
    let Ok(shell::Output::Explanation { record, .. }) = &v.details.as_ref().unwrap().result else {
        panic!()
    };
    assert_eq!(record.id, id);
    assert_eq!(record.status, Status::Succeeded);
}

#[test]
fn unrelated_directory_change_and_pure_rejections_do_not_reload_root() {
    use crate::storage::{
        Partition,
        stream::{State, Stream},
    };
    let (mut disk, layout) = formatted();
    {
        let state = State::default();
        let mut stream = Stream::new(Partition::open(&mut disk, &layout).unwrap(), &state);
        let fs = fatfs::FileSystem::new(&mut stream, fatfs::FsOptions::new()).unwrap();
        fs.root_dir().create_dir("folder").unwrap();
        fs.unmount().unwrap();
    }
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    let initial = v.list_operation;
    files
        .execute(
            &Path::root(),
            Request::CreateText {
                path: "folder/inside.txt",
                text: "nested",
            },
        )
        .unwrap()
        .result
        .unwrap();
    assert!(v.observe_changes(&files));
    assert!(!v.pending());
    assert_eq!(v.list_operation, initial);
    command(&mut v, &mut files, "write folder/inside.txt again");
    assert!(!v.pending());
    assert_eq!(v.list_operation, initial);
    assert!(matches!(
        &v.results.back().unwrap().reply.result,
        Err(shell::Error::Service(SystemError {
            code: Code::AlreadyExists,
            ..
        }))
    ));
    command(&mut v, &mut files, "write missing");
    assert!(!v.pending());
    assert_eq!(v.list_operation, initial);
}

#[test]
fn event_gap_is_visible_and_consumed_event_eviction_is_not_a_gap() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    for i in 0..40 {
        files
            .execute(
                &Path::root(),
                Request::CreateText {
                    path: &format!("lost{i:02}"),
                    text: "x",
                },
            )
            .unwrap()
            .result
            .unwrap();
    }
    v.observe_changes(&files);
    assert_eq!(
        v.update,
        UpdateState::Refreshing(UpdateReason::MissedEvents)
    );
    settle(&mut v, &mut files);
    assert_eq!(v.update, UpdateState::Refreshed(UpdateReason::MissedEvents));
    assert!(v.change_operation.is_none());
    for i in 0..40 {
        let r = files
            .execute(
                &Path::root(),
                Request::CreateText {
                    path: &format!("seen{i:02}"),
                    text: "x",
                },
            )
            .unwrap();
        r.result.unwrap();
        v.observe_changes(&files);
        settle(&mut v, &mut files);
        assert_eq!(
            v.update,
            UpdateState::Refreshed(UpdateReason::Operation(r.operation_id))
        );
    }
    assert!(files.changes.evicted > 0);
}

#[test]
fn partial_write_triggers_failed_refresh_and_details_keep_original_write() {
    let (mut disk, layout) = formatted();
    let mut backend = FatBackend::new(&mut disk, layout);
    backend.inject(Stage::FileSync);
    let mut files = FileService::new(backend, BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    command(&mut v, &mut files, "write failed.txt payload");
    let id = v.results.back().unwrap().reply.operation_id.unwrap();
    settle(&mut v, &mut files);
    assert_eq!(v.update, UpdateState::Failed(UpdateReason::Operation(id)));
    assert!(matches!(
        &v.page,
        Err(shell::Error::Service(SystemError {
            code: Code::Offline,
            ..
        }))
    ));
    v.activate(Focus::Files);
    v.execute_pending(&mut files);
    let Ok(shell::Output::Explanation { record, .. }) = &v.details.as_ref().unwrap().result else {
        panic!()
    };
    assert_eq!(
        (record.id, record.status, record.stage),
        (id, Status::Failed, Stage::FileSync)
    );
    assert!(!record.partial_effect.persistence_confirmed);
    assert!(matches!(
        &v.results.back().unwrap().reply.result,
        Err(shell::Error::Service(SystemError {
            stage: Stage::FileSync,
            ..
        }))
    ));
}

#[test]
fn continuous_changes_stop_refresh_and_explicit_reload_recovers() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    for i in 0..6 {
        files
            .execute(
                &Path::root(),
                Request::CreateText {
                    path: &format!("burst{i}"),
                    text: "x",
                },
            )
            .unwrap()
            .result
            .unwrap();
        v.observe_changes(&files);
    }
    assert_eq!(v.update, UpdateState::Unstable);
    assert!(!v.pending());
    v.activate(Focus::Refresh);
    v.execute_pending(&mut files);
    assert_eq!(v.page.as_ref().unwrap().entries.len(), 6);
    assert_eq!(v.update, UpdateState::Idle);
    command(&mut v, &mut files, "write after.txt yes");
    settle(&mut v, &mut files);
    assert_eq!(
        v.page.as_ref().unwrap().entries[v.file_row].name,
        "after.txt"
    );
}

#[test]
fn new_service_reloads_list_but_old_results_do_not_alias_new_records() {
    let (mut disk, layout) = formatted();
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    command(&mut v, &mut files, "write previous.txt old");
    settle(&mut v, &mut files);
    let old = v.results.back().unwrap().reply.operation_id;
    let (mut disk2, layout2) = formatted();
    let mut next = FileService::new(FatBackend::new(&mut disk2, layout2), BootId([2; 16])).unwrap();
    v.observe_changes(&next);
    settle(&mut v, &mut next);
    assert_eq!(v.update, UpdateState::Refreshed(UpdateReason::NewService));
    assert!(v.page.as_ref().unwrap().entries.is_empty());
    assert_eq!(v.results.back().unwrap().reply.operation_id, old);
    v.activate(Focus::Details);
    v.execute_pending(&mut next);
    assert!(matches!(
        v.details.as_ref().unwrap().result,
        Err(shell::Error::ForeignBoot)
    ));
}

#[test]
fn failed_list_keeps_original_io_cause_instead_of_replacing_it_with_offline() {
    struct FailSecondList<B: Backend> {
        inner: B,
        lists: usize,
    }
    impl<B: Backend> Backend for FailSecondList<B> {
        fn volume(&self) -> VolumeInfo {
            self.inner.volume()
        }
        fn run(
            &mut self,
            path: &Path,
            request: BackendRequest<'_>,
            progress: &mut Progress,
        ) -> Result<Data, Cause> {
            if matches!(request, BackendRequest::List { .. }) {
                self.lists += 1;
                if self.lists == 2 {
                    progress.enter(Stage::List, Status::Running);
                    return Err(Cause::Contract(Code::Io));
                }
            }
            self.inner.run(path, request, progress)
        }
    }
    let (mut disk, layout) = formatted();
    let backend = FailSecondList {
        inner: FatBackend::new(&mut disk, layout),
        lists: 0,
    };
    let mut files = FileService::new(backend, BootId([1; 16])).unwrap();
    let mut v = view();
    v.execute_pending(&mut files);
    command(&mut v, &mut files, "write created.txt payload");
    settle(&mut v, &mut files);
    assert!(matches!(
        &v.page,
        Err(shell::Error::Service(SystemError {
            code: Code::Io,
            stage: Stage::List,
            ..
        }))
    ));
    assert_eq!(v.volume.unwrap().mount_state, MountState::Offline);
    let query = v.list_operation;
    for _ in 0..5 {
        v.observe_changes(&files);
        v.execute_pending(&mut files);
    }
    assert!(!v.pending());
    assert_eq!(v.list_operation, query);
    assert!(matches!(
        &v.page,
        Err(shell::Error::Service(SystemError {
            code: Code::Io,
            stage: Stage::List,
            ..
        }))
    ));
}
