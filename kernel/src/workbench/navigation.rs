//! 目录意图共享文件列表查询，失败时保留已提交的目录与快照。
use super::*;

pub(super) fn list<B: Backend>(
    files: &mut FileService<B>,
    cwd: &Path,
    path: &str,
    cursor: Option<&ListCursor>,
) -> (Option<OperationId>, Result<ListPage, shell::Error>) {
    match files.execute(cwd, Request::List { path, cursor }) {
        Ok(reply) => (
            Some(reply.operation_id),
            reply
                .result
                .map(|output| match output {
                    Output::List(page) => page,
                    _ => unreachable!("list service reply"),
                })
                .map_err(shell::Error::Service),
        ),
        Err(code) => (None, Err(shell::Error::ServiceIdentity(code))),
    }
}

impl View {
    pub(super) fn install_page(
        &mut self,
        operation: Option<OperationId>,
        page: Result<ListPage, shell::Error>,
        generation: u64,
    ) {
        self.list_operation = operation;
        self.page = page;
        self.list_generation = generation;
        self.file_row = 0;
        self.file_scroll = 0;
        // 按下后若快照更新，释放时不能激活替换后的对象。
        if self.pressed.is_some_and(|(focus, _)| {
            matches!(
                focus,
                Focus::Files
                    | Focus::OpenDirectory
                    | Focus::PreviewFile
                    | Focus::DeleteEntry
                    | Focus::RenameEntry
                    | Focus::CopyEntry
                    | Focus::FileDetails
            )
        }) {
            self.pressed = None;
        }
        if (self.focus == Focus::OpenDirectory && self.selected_directory().is_none())
            || (self.focus == Focus::DeleteEntry && self.selected_entry().is_none())
            || (matches!(
                self.focus,
                Focus::RenameEntry | Focus::CopyEntry | Focus::PreviewFile
            ) && self.selected_file().is_none())
            || (self.focus == Focus::FileDetails && self.file_detail_operation().is_none())
        {
            self.focus = Focus::Files;
        }
    }

    pub(super) fn navigate<B: Backend>(&mut self, files: &mut FileService<B>, path: &str) {
        let (operation, page) = list(files, &self.directory, path, None);
        self.file_message = match page {
            Ok(page) => {
                self.directory = page.directory.clone();
                self.refresh = None;
                self.update = UpdateState::Idle;
                self.change_operation = None;
                self.install_page(operation, Ok(page), files.generation());
                self.focus = Focus::Files;
                None
            }
            Err(error) => Some((operation, FileMessage::Failed(error))),
        };
    }

    pub(super) fn open_terminal<B: Backend>(&mut self, files: &mut FileService<B>) {
        let (operation, page) = list(files, &self.directory, ".", None);
        self.file_message = match page {
            Ok(page) => {
                self.shell.open_directory(page);
                if self.single {
                    self.terminal = true;
                }
                self.focus = Focus::Input;
                None
            }
            Err(error) => Some((operation, FileMessage::Failed(error))),
        };
    }

    pub(super) fn selected_directory(&self) -> Option<&Path> {
        let entry = self.selected_entry()?;
        (entry.kind == FileKind::Directory).then_some(&entry.reference.path)
    }

    pub(super) fn selected_entry(&self) -> Option<&FileEntry> {
        self.page.as_ref().ok()?.entries.get(self.file_row)
    }

    pub(super) fn selected_file(&self) -> Option<&FileEntry> {
        self.selected_entry()
            .filter(|entry| entry.kind == FileKind::File)
    }

    pub(super) fn open_file_details(&mut self) {
        if let Some(id) = self.file_detail_operation() {
            self.schedule(Intent::Details(id));
        }
    }

    pub(super) fn file_detail_operation(&self) -> Option<OperationId> {
        self.file_message.as_ref().map_or_else(
            || self.change_operation.or(self.list_operation),
            |(id, _)| *id,
        )
    }

    pub(super) fn reset_path_input(&mut self) {
        let _ = self.path_editor.apply(Edit::Cancel);
        for c in self.directory.as_str().chars() {
            // Path 已校验长度与字符。
            self.path_editor
                .apply(Edit::Insert(c))
                .expect("validated path");
        }
        self.path_rejected = false;
    }

    pub(super) fn focus_control(&mut self, focus: Focus) {
        if focus == Focus::FilePath && self.focus != focus {
            self.reset_path_input();
        } else if self.focus == Focus::FilePath && focus != self.focus && self.path_rejected {
            // 离开路径栏时丢弃已拒绝的路径草稿。
            self.path_rejected = false;
            if self.notice == Notice::InputFull {
                self.notice = Notice::Ready;
            }
        }
        self.focus = focus;
    }
}
