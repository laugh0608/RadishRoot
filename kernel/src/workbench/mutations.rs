//! 文件区的新增与删除通过同一个服务请求入口执行。
use super::*;

pub(super) enum FileDialog {
    NewDirectory,
    Delete(FileRef),
}

pub(super) enum FileMessage {
    Created(Path),
    Removed(Path),
    Failed(shell::Error),
}

impl View {
    pub(super) fn show_dialog(&mut self, dialog: FileDialog) {
        self.focus = match dialog {
            FileDialog::NewDirectory => Focus::DialogInput,
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
            Some(Intent::CreateDirectory(_) | Intent::Remove(_))
        ) {
            self.pending = None;
        }
        self.notice = Notice::Ready;
    }

    pub(super) fn confirm_dialog(&mut self) {
        match self.dialog.as_ref() {
            Some(FileDialog::NewDirectory) if self.path_rejected => {
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
