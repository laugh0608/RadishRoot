use super::*;
use alloc::{collections::BTreeMap, format, string::ToString, vec, vec::Vec};
use editor::{Edit, Editor};
use input::{Action, Input, InputError};
use parser::{Command, ParseError, parse};
#[test]
fn quoted_arguments_and_control_syntax_are_explicit() {
    assert_eq!(
        parse("write 'a b.txt' \"two words\\n\\\"quoted\\\"\""),
        Ok(Command::Write {
            path: "a b.txt".into(),
            text: "two words\n\"quoted\"".into()
        })
    );
    assert_eq!(
        parse("write empty.txt ''"),
        Ok(Command::Write {
            path: "empty.txt".into(),
            text: String::new()
        })
    );
    assert_eq!(
        parse("write '萝卜.txt' 'literal\\n'"),
        Ok(Command::Write {
            path: "萝卜.txt".into(),
            text: "literal\\n".into()
        })
    );
    for (line, error) in [
        ("", ParseError::Empty),
        ("write a \"bad", ParseError::Quote),
        ("write a \\q", ParseError::Escape),
        ("write a x;ls", ParseError::Operator),
        ("ls | cat", ParseError::Operator),
        ("help extra", ParseError::Arguments),
        ("external", ParseError::UnknownCommand),
        ("cat a -1", ParseError::Number),
        ("cat a 18446744073709551616", ParseError::Number),
        ("explain 1", ParseError::Identity),
        ("help\nwrite a x", ParseError::Control),
    ] {
        assert_eq!(parse(line), Err(error), "{line}");
    }
    assert_eq!(parse(&"x".repeat(1025)), Err(ParseError::Length));
    assert_eq!(parse("help a b c d e f g h"), Err(ParseError::Arguments));
}
#[test]
fn editor_uses_unicode_boundaries_and_restores_draft_with_history_limits() {
    let mut e = Editor::new();
    for c in "a萝b".chars() {
        e.apply(Edit::Insert(c)).unwrap();
    }
    e.apply(Edit::Left).unwrap();
    e.apply(Edit::Backspace).unwrap();
    assert_eq!((e.text(), e.cursor()), ("ab", 1));
    e.apply(Edit::Insert('卜')).unwrap();
    e.apply(Edit::Home).unwrap();
    e.apply(Edit::Delete).unwrap();
    assert_eq!(e.apply(Edit::Submit).unwrap().unwrap(), "卜b");
    for c in "draft".chars() {
        e.apply(Edit::Insert(c)).unwrap();
    }
    e.apply(Edit::Previous).unwrap();
    assert_eq!(e.text(), "卜b");
    e.apply(Edit::Next).unwrap();
    assert_eq!(e.text(), "draft");
    e.apply(Edit::Cancel).unwrap();
    for _ in 0..17 {
        e.apply(Edit::Insert('x')).unwrap();
        e.apply(Edit::Submit).unwrap();
    }
    assert_eq!((e.history_len(), e.evicted), (16, 2));
    for _ in 0..LINE_BYTES {
        e.apply(Edit::Insert('a')).unwrap();
    }
    assert_eq!(e.apply(Edit::Insert('萝')), Err(editor::Error::Capacity));
    assert_eq!(e.text().len(), LINE_BYTES);
}
#[test]
fn uart_rejects_damaged_lines_and_recovers_without_submitting_suffix() {
    let mut input = Input::new();
    let mut submitted = Vec::new();
    for b in b"write a x\xffbad\r\nhelp\r\n" {
        if let Action::Submitted(s) = input.receive(*b) {
            submitted.push(s)
        }
    }
    assert_eq!(submitted, vec!["help"]);
    for b in "write '萝卜.txt' hi".bytes() {
        input.receive(b);
    }
    assert!(matches!(input.receive(b'\n'),Action::Submitted(s) if s=="write '萝卜.txt' hi"));
    for b in b"helx\x1b[D\x1b[3~p\r" {
        if let Action::Submitted(s) = input.receive(*b) {
            assert_eq!(s, "help");
        }
    }
    for b in b"bad\x1b" {
        input.receive(*b);
    }
    input.escape_timeout();
    assert!(input.editor.text().is_empty());
    input.receive(0xe0);
    input.receive(0x80);
    assert!(matches!(
        input.receive(0x80),
        Action::Rejected(InputError::Utf8)
    ));
    assert!(!matches!(input.receive(b'\n'), Action::Submitted(_)));
    for _ in 0..LINE_BYTES {
        input.receive(b'a');
    }
    assert!(matches!(
        input.receive(b'b'),
        Action::Rejected(InputError::Capacity)
    ));
    assert!(!matches!(input.receive(b'\n'), Action::Submitted(_)));
    input.receive(b'x');
    input.transport_error();
    assert!(!matches!(input.receive(b'\n'), Action::Submitted(_)));
}
struct Memory {
    data: BTreeMap<String, Vec<u8>>,
    readonly: bool,
    fail: bool,
    calls: usize,
}
impl Backend for Memory {
    fn volume(&self) -> VolumeInfo {
        VolumeInfo {
            id: VolumeId([2; 16]),
            capacity_bytes: 1024 * 1024,
            read_only: self.readonly,
            flush_support: true,
            mount_state: MountState::Unchecked,
        }
    }
    fn run(
        &mut self,
        path: &Path,
        request: BackendRequest<'_>,
        p: &mut Progress,
    ) -> Result<Data, Cause> {
        self.calls += 1;
        p.enter(Stage::Mount, Status::Running);
        p.mounted = true;
        p.completed();
        match request {
            BackendRequest::List { offset } => {
                p.enter(Stage::List, Status::Running);
                let entries = self
                    .data
                    .iter()
                    .skip(offset as usize)
                    .take(PAGE_SIZE)
                    .map(|(name, b)| RawEntry {
                        name: name.clone(),
                        kind: FileKind::File,
                        size_bytes: Some(b.len() as u64),
                    })
                    .collect();
                Ok(Data::List {
                    entries,
                    next: if self.data.len() > offset as usize + PAGE_SIZE {
                        Some(offset + PAGE_SIZE as u32)
                    } else {
                        None
                    },
                })
            }
            BackendRequest::Read { offset, limit } => {
                p.enter(Stage::Read, Status::Running);
                let bytes = self
                    .data
                    .get(path.relative())
                    .ok_or(Cause::Contract(Code::NotFound))?;
                let n = usize::try_from(offset).unwrap();
                if n > bytes.len() {
                    return Err(Cause::Contract(Code::InvalidArgument));
                }
                let end = (n + limit).min(bytes.len());
                Ok(Data::Read {
                    bytes: bytes[n..end].to_vec(),
                    eof: end == bytes.len(),
                })
            }
            BackendRequest::CreateText(text) => {
                p.enter(Stage::Create, Status::Running);
                if self.data.contains_key(path.relative()) {
                    return Err(Cause::Contract(Code::AlreadyExists));
                }
                self.data
                    .insert(path.relative().into(), text.as_bytes().to_vec());
                p.record.partial_effect = PartialEffect {
                    created: true,
                    written_bytes: text.len(),
                    may_have_changed: true,
                    persistence_confirmed: false,
                };
                p.completed();
                p.enter(Stage::Write, Status::Running);
                p.completed();
                for stage in [
                    Stage::FileSync,
                    Stage::VolumeSync,
                    Stage::Unmount,
                    Stage::DeviceFlush,
                ] {
                    p.enter(stage, Status::Flushing);
                    if self.fail && stage == Stage::FileSync {
                        return Err(Cause::Contract(Code::Io));
                    }
                    p.completed();
                }
                p.record.partial_effect.persistence_confirmed = true;
                Ok(Data::Created)
            }
        }
    }
}
fn service(readonly: bool, fail: bool) -> FileService<Memory> {
    FileService::new(
        Memory {
            data: BTreeMap::new(),
            readonly,
            fail,
            calls: 0,
        },
        BootId([1; 16]),
    )
    .unwrap()
}
#[test]
fn bad_commands_have_no_service_operations_and_conflict_details_share_the_id() {
    let mut files = service(false, false);
    let mut shell = Shell::new();
    for line in [
        "write path",
        "write a x | ls",
        "write a \"bad",
        "cat a invalid",
    ] {
        assert!(matches!(
            shell.execute(line, &mut files).unwrap().result,
            Err(Error::Parse(_))
        ));
    }
    assert_eq!(files.operations.iter().count(), 0);
    let r = shell
        .execute("write 'a b.txt' 'hello world'", &mut files)
        .unwrap();
    let id = r.operation_id.unwrap();
    assert!(matches!(r.result, Ok(Output::Created(_))));
    assert_eq!(id.sequence, 1);
    let r = shell.execute("write 'a b.txt' again", &mut files).unwrap();
    let conflict = r.operation_id.unwrap();
    assert!(matches!(
        r.result,
        Err(Error::Service(SystemError {
            code: Code::AlreadyExists,
            ..
        }))
    ));
    for (op, status) in [(id, Status::Succeeded), (conflict, Status::Failed)] {
        let r = shell
            .execute(
                &format!(
                    "explain {:032x}:{}",
                    u128::from_be_bytes(op.boot.0),
                    op.sequence
                ),
                &mut files,
            )
            .unwrap();
        assert!(r.operation_id.is_none());
        let Output::Explanation { record, events, .. } = r.result.unwrap() else {
            panic!()
        };
        assert_eq!(record.id, op);
        assert_eq!(record.status, status);
        assert_eq!(events.last().unwrap().status, status);
    }
    assert_eq!(files.operations.iter().count(), 2);
    let read = shell.execute("cat 'a b.txt'", &mut files).unwrap();
    assert!(
        matches!(read.result,Ok(Output::Bytes{bytes,eof:true,offset:0})if bytes==b"hello world")
    );
}
#[test]
fn partial_failure_explanation_keeps_original_effect_and_stale_pages_are_rejected() {
    let mut files = service(false, true);
    let mut shell = Shell::new();
    let r = shell.execute("write a payload", &mut files).unwrap();
    let id = r.operation_id.unwrap();
    let r = shell
        .execute(
            &format!(
                "explain {:032x}:{}",
                u128::from_be_bytes(id.boot.0),
                id.sequence
            ),
            &mut files,
        )
        .unwrap();
    let Output::Explanation { record, .. } = r.result.unwrap() else {
        panic!()
    };
    assert_eq!(record.stage, Stage::FileSync);
    assert_eq!(record.partial_effect.written_bytes, 7);
    assert!(!record.partial_effect.persistence_confirmed);
    assert_eq!(record.error.unwrap().code, Code::Io);
    let mut files = service(false, false);
    let mut shell = Shell::new();
    for i in 0..34 {
        shell
            .execute(&format!("write f{i:02} data"), &mut files)
            .unwrap();
    }
    let r = shell.execute("ls", &mut files).unwrap();
    assert!(
        matches!(r.result,Ok(Output::Files(ListPage{entries,next:Some(_)}))if entries.len()==32)
    );
    shell.execute("write new data", &mut files).unwrap();
    assert!(matches!(
        shell.execute("ls --next", &mut files).unwrap().result,
        Err(Error::Service(SystemError {
            code: Code::StaleReference,
            ..
        }))
    ));
    shell.execute("ls", &mut files).unwrap();
    assert!(
        matches!(shell.execute("ls --next",&mut files).unwrap().result,Ok(Output::Files(ListPage{entries,next:None}))if entries.len()==3)
    );
    let r = shell.execute("operations", &mut files).unwrap();
    assert!(
        matches!(r.result,Ok(Output::Operations{records,evicted})if records.len()==32 && evicted>0)
    );
    let prefix = format!("{:032x}", u128::from_be_bytes([1; 16]));
    assert!(
        matches!(shell.execute(&format!("explain {prefix}:1"),&mut files).unwrap().result,Err(Error::OperationUnavailable{evicted})if evicted>0)
    );
    assert_eq!(
        shell
            .execute("explain 02020202020202020202020202020202:1", &mut files)
            .unwrap()
            .result
            .err(),
        Some(Error::ForeignBoot)
    );
}
#[test]
fn rendered_results_escape_untrusted_text_and_keep_byte_types() {
    let r = Reply {
        command_id: 7,
        operation_id: None,
        result: Ok(Output::Bytes {
            bytes: b"\"\nRR:CONSOLE:REPLY \\".to_vec(),
            eof: false,
            offset: 4,
        }),
    };
    let mut s = String::new();
    render::reply(&mut s, &r).unwrap();
    assert_eq!(s,"{\"schema\":\"radishroot.command.v1\",\"command_id\":7,\"operation_id\":null,\"ok\":true,\"output\":{\"kind\":\"bytes\",\"offset\":4,\"length\":20,\"eof\":false,\"hex\":\"220a52523a434f4e534f4c453a5245504c59205c\",\"text\":\"\\\"\\nRR:CONSOLE:REPLY \\\\\"}}".to_string());
}

#[test]
fn host_equivalence_trace_preserves_success_conflict_and_detail() {
    extern crate std;
    let (mut disk, layout) = crate::services::tests::formatted();
    let mut files = FileService::new(
        crate::services::fat::FatBackend::new(&mut disk, layout),
        BootId([1; 16]),
    )
    .unwrap();
    let mut shell = Shell::new();
    // 与真实 guest 使用相同的前置服务操作顺序；预期内容在两侧另行断言。
    shell.execute("ls", &mut files).unwrap();
    shell
        .execute("write /data/../escape x", &mut files)
        .unwrap();
    let explain = format!("explain {:032x}:3", u128::from_be_bytes([1; 16]));
    for (label, command) in [
        (
            "create",
            "write \"note one.txt\" \"hello \\\"quoted\\\"\\n\"",
        ),
        ("conflict", "write \"note one.txt\" again"),
        ("cat", "cat \"note one.txt\""),
        ("explain", explain.as_str()),
    ] {
        let r = shell.execute(command, &mut files).unwrap();
        if label == "conflict" {
            assert!(matches!(
                r.result,
                Err(Error::Service(SystemError {
                    code: Code::AlreadyExists,
                    ..
                }))
            ));
        } else {
            assert!(r.result.is_ok());
        }
        let mut json = String::new();
        render::reply(&mut json, &r).unwrap();
        std::println!("RR:HOST:{label} {json}");
    }
}
