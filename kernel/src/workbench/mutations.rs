//! 文件区的修改通过同一个服务请求入口执行。
use super::*;

pub(super) enum FileDialog {
    NewDirectory,
    Delete(FileRef),
    Rename(FileRef),
}

impl FileDialog {
    pub(super) fn accepts_path(&self) -> bool {
        matches!(self, Self::NewDirectory | Self::Rename(_))
    }
}

pub(super) enum FileMessage {
    Created(Path),
    Removed(Path),
    Renamed(Path),
    Failed(shell::Error),
}

impl View {
    pub(super) fn show_dialog(&mut self, dialog: FileDialog) {
        self.focus = match dialog {
            FileDialog::NewDirectory | FileDialog::Rename(_) => Focus::DialogInput,
            FileDialog::Delete(_) => Focus::DialogCancel,
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
            Some(Intent::CreateDirectory(_) | Intent::Remove(_) | Intent::Rename { .. })
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
            None => (),
        }
    }

    pub(super) fn mutate<B: Backend>(&mut self, files: &mut FileService<B>, request: Request<'_>) {
        self.close_dialog();
        self.file_message = Some(match files.execute(&self.directory, request) {
            Ok(reply) => {
                let message = match reply.result {
                    Ok(Output::Created(reference)) => FileMessage::Created(reference.path),
                    Ok(Output::Removed(path)) => FileMessage::Removed(path),
                    Ok(Output::Renamed { target, .. }) => FileMessage::Renamed(target.path),
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
