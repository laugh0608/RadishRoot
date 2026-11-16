//! 文件区的修改通过同一个服务请求入口执行。
use super::*;

pub(super) enum FileDialog {
    NewDirectory,
    Delete(FileRef),
    Rename(FileRef),
    Copy(FileRef),
    OverwriteCopy { source: FileRef, target: FileRef },
}

impl FileDialog {
    pub(super) fn accepts_path(&self) -> bool {
        matches!(self, Self::NewDirectory | Self::Rename(_) | Self::Copy(_))
    }
}

pub(super) enum FileMessage {
    Created(Path),
    Removed(Path),
    Renamed(Path),
    Copied {
        target: Path,
        bytes: usize,
        overwritten: bool,
    },
    Failed(shell::Error),
}

impl View {
    pub(super) fn show_dialog(&mut self, dialog: FileDialog) {
        self.focus = if dialog.accepts_path() {
            Focus::DialogInput
        } else {
            Focus::DialogCancel
        };
        self.dialog = Some(dialog);
        self.pressed = None;
        let _ = self.path_editor.apply(Edit::Cancel);
        self.path_rejected = false;
        self.notice = Notice::Ready;
    }

    pub(super) fn close_dialog(&mut self) {
        self.dialog = None;
        self.focus = Focus::Files;
        self.pressed = None;
        let _ = self.path_editor.apply(Edit::Cancel);
        self.path_rejected = false;
        if matches!(
            self.pending,
            Some(
                Intent::CreateDirectory(_)
                    | Intent::Remove(_)
                    | Intent::Rename { .. }
                    | Intent::Copy { .. }
                    | Intent::LookupCopyTarget { .. }
                    | Intent::OverwriteCopy { .. }
            )
        ) {
            self.pending = None;
        }
        self.notice = Notice::Ready;
    }

    pub(super) fn confirm_dialog(&mut self) {
        match self.dialog.as_ref() {
            Some(dialog) if dialog.accepts_path() && self.path_rejected => {
                let _ = self.path_editor.apply(Edit::Cancel);
                self.path_rejected = false;
                self.notice = Notice::Ready;
                self.focus = Focus::DialogInput;
            }
            Some(FileDialog::NewDirectory) => {
                self.schedule(Intent::CreateDirectory(String::from(
                    self.path_editor.text(),
                )));
            }
            Some(FileDialog::Delete(reference)) => self.schedule(Intent::Remove(reference.clone())),
            Some(FileDialog::Rename(reference)) => self.schedule(Intent::Rename {
                source: reference.clone(),
                destination: String::from(self.path_editor.text()),
            }),
            Some(FileDialog::Copy(reference)) => self.schedule(Intent::Copy {
                source: reference.clone(),
                destination: String::from(self.path_editor.text()),
            }),
            Some(FileDialog::OverwriteCopy { source, target }) => {
                self.schedule(Intent::OverwriteCopy {
                    source: source.clone(),
                    target: target.clone(),
                })
            }
            None => (),
        }
    }

    pub(super) fn mutate<B: Backend>(&mut self, files: &mut FileService<B>, request: Request<'_>) {
        self.finish_mutation(files.execute(&self.directory, request));
    }

    pub(super) fn copy_file<B: Backend>(
        &mut self,
        files: &mut FileService<B>,
        source: FileRef,
        destination: String,
    ) {
        let reply = files.execute(
            &self.directory,
            Request::Copy {
                source: source.path.as_str(),
                destination: &destination,
                reference: Some(&source),
                overwrite: Overwrite::Deny,
            },
        );
        if let Ok(reply) = &reply
            && let Err(error) = &reply.result
            && error.code == Code::AlreadyExists
            && error.stage == Stage::Create
        {
            // 默认复制已明确拒绝。下一轮只读取目标引用，再显示覆盖确认。
            self.file_message = Some((
                Some(reply.operation_id),
                FileMessage::Failed(shell::Error::Service(error.clone())),
            ));
            self.schedule(Intent::LookupCopyTarget {
                source,
                destination,
            });
        } else {
            self.finish_mutation(reply);
        }
    }

    pub(super) fn lookup_copy_target<B: Backend>(
        &mut self,
        files: &mut FileService<B>,
        source: FileRef,
        destination: &str,
    ) {
        let reply = files.execute(&self.directory, Request::LookupFile { path: destination });
        match reply {
            Ok(Reply {
                result: Ok(Output::File(target)),
                ..
            }) => {
                self.show_dialog(FileDialog::OverwriteCopy { source, target });
            }
            reply => self.finish_mutation(reply),
        }
    }

    fn finish_mutation(&mut self, reply: Result<Reply, Code>) {
        self.close_dialog();
        self.file_message = Some(match reply {
            Ok(reply) => {
                let message = match reply.result {
                    Ok(Output::Created(reference)) => FileMessage::Created(reference.path),
                    Ok(Output::Removed(path)) => FileMessage::Removed(path),
                    Ok(Output::Renamed { target, .. }) => FileMessage::Renamed(target.path),
                    Ok(Output::Copied {
                        target,
                        bytes,
                        overwritten,
                        ..
                    }) => FileMessage::Copied {
                        target: target.path,
                        bytes,
                        overwritten,
                    },
                    Err(error) => FileMessage::Failed(shell::Error::Service(error)),
                    Ok(_) => unreachable!("mutation reply"),
                };
                (Some(reply.operation_id), message)
            }
            Err(code) => (
                None,
                FileMessage::Failed(shell::Error::ServiceIdentity(code)),
            ),
        });
    }
}
