//! 当前目录的有界变更订阅与分页刷新；历史结果保持原快照。
use super::*;
const RESTARTS: u8 = 4;
const PAGES: u32 = services::MAX_ENTRIES / services::PAGE_SIZE as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateReason {
    Operation(OperationId),
    CheckPathAlias,
    MissedEvents,
    NewService,
    ServiceOffline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Refreshing(UpdateReason),
    Refreshed(UpdateReason),
    Failed(UpdateReason),
    Unstable,
    ScanLimit,
}
impl UpdateState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Refreshing(UpdateReason::CheckPathAlias) => "checking-path",
            Self::Refreshing(_) => "refreshing",
            Self::Refreshed(_) => "refreshed",
            Self::Failed(_) => "failed",
            Self::Unstable => "unstable",
            Self::ScanLimit => "scan-limit",
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct Subscription {
    boot: BootId,
    volume: VolumeId,
    generation: u64,
    state: MountState,
}
pub(super) struct Refresh {
    target: Option<Path>,
    cursor: Option<ListCursor>,
    reason: UpdateReason,
    pages: u32,
    restarts: u8,
}
impl View {
    /// 只检查最多 32 条变更事实；返回是否观察到新状态，不执行 IO。
    pub fn observe_changes<B: Backend>(&mut self, files: &FileService<B>) -> bool {
        let power_changed = self.power != files.power();
        if power_changed {
            self.power = files.power();
            self.power_error = match self.power {
                services::power::State::Failed(_, id) => files
                    .operations
                    .iter()
                    .find(|r| r.id == id)
                    .and_then(|r| r.error.clone()),
                _ => None,
            };
            self.dirty = true;
        }
        let now = Subscription {
            boot: files.boot_id(),
            volume: files.volume().id,
            generation: files.generation(),
            state: files.volume().mount_state,
        };
        let Some(old) = self.subscription.replace(now) else {
            return power_changed;
        };
        let identity = old.boot != now.boot || old.volume != now.volume;
        if !identity && old.generation == now.generation && old.state == now.state {
            return power_changed;
        }
        self.volume = Some(files.volume());
        self.dirty |= identity || old.state != now.state;
        let mut latest = None;
        let mut check_alias = false;
        let mut expected = old.generation.saturating_add(1);
        let mut gap = false;
        if !identity && old.generation != now.generation {
            for event in files
                .changes
                .iter()
                .filter(|e| e.generation > old.generation)
            {
                if event.generation != expected
                    || event.volume != now.volume
                    || event.operation_id.boot != now.boot
                {
                    gap = true;
                }
                expected = event.generation.saturating_add(1);
                let [target, parent] = &event.paths;
                let removing = event.request_kind == RequestKind::Remove;
                if parent == &self.directory || (removing && target == &self.directory) {
                    latest = Some(event);
                    check_alias = false;
                } else if !self.directory.is_root()
                    && ((!parent.is_root()
                        && parent.relative().split('/').count()
                            == self.directory.relative().split('/').count())
                        || (removing
                            && target.relative().split('/').count()
                                == self.directory.relative().split('/').count()))
                {
                    // FAT 长短名和大小写别名可能指向同一目录。未核实归属时只刷新当前快照。
                    check_alias = true;
                }
            }
            gap |= expected != now.generation.saturating_add(1);
        }
        let reason = if identity {
            Some(UpdateReason::NewService)
        } else if gap {
            Some(UpdateReason::MissedEvents)
        } else if check_alias {
            Some(UpdateReason::CheckPathAlias)
        } else if let Some(event) = latest {
            Some(UpdateReason::Operation(event.operation_id))
        } else if old.state != now.state && now.state == MountState::Offline && self.page.is_ok() {
            Some(UpdateReason::ServiceOffline)
        } else {
            None
        };
        if reason.is_none() && self.refresh.is_none() {
            return true;
        }
        if !identity && matches!(self.update, UpdateState::Unstable | UpdateState::ScanLimit) {
            return true;
        }
        let previous = self.refresh.take();
        let reason = reason.unwrap_or_else(|| previous.as_ref().expect("active refresh").reason);
        if let UpdateReason::Operation(id) = reason
            && self
                .file_message
                .as_ref()
                .is_some_and(|(previous, message)| {
                    *previous != Some(id)
                        && matches!(message, FileMessage::Created(_) | FileMessage::Removed(_))
                })
        {
            self.file_message = None;
        }
        self.change_operation = match reason {
            UpdateReason::Operation(id) => Some(id),
            _ => None,
        };
        self.dirty = true;
        if self.list_generation == now.generation
            && self.page.is_ok()
            && previous.is_none()
            && !identity
            && now.state != MountState::Offline
        {
            self.update = UpdateState::Refreshed(reason);
            return true;
        }
        let restarts = if identity {
            0
        } else {
            previous.as_ref().map_or(0, |r| r.restarts + 1)
        };
        if restarts > RESTARTS {
            self.update = UpdateState::Unstable;
            return true;
        }
        let target = match reason {
            UpdateReason::Operation(_) => match latest {
                Some(e) if e.request_kind != RequestKind::Remove => Some(e.paths[0].clone()),
                Some(_) => None,
                None => previous.as_ref().and_then(|r| r.target.clone()),
            },
            _ => None,
        };
        self.refresh = Some(Refresh {
            target,
            cursor: None,
            reason,
            pages: 0,
            restarts,
        });
        self.update = UpdateState::Refreshing(reason);
        true
    }
    /// 一轮最多读取一页；命令和详情由父模块先处理，不阻断草稿编辑。
    pub(super) fn refresh_step<B: Backend>(&mut self, files: &mut FileService<B>) {
        let Some(mut job) = self.refresh.take() else {
            return;
        };
        let (operation, page) = navigation::list(files, &self.directory, ".", job.cursor.as_ref());
        job.pages += 1;
        self.install_page(operation, page, files.generation());
        self.update = match &self.page {
            Err(_) => UpdateState::Failed(job.reason),
            Ok(page) => {
                let at = job.target.as_ref().and_then(|target| {
                    page.entries
                        .iter()
                        .position(|e| e.reference.path == *target)
                });
                if let Some(at) = at {
                    self.file_row = at;
                    self.file_scroll =
                        at.saturating_sub(Layout::new(self).file_rows().saturating_sub(1));
                    UpdateState::Refreshed(job.reason)
                } else if job.target.is_some() && page.next.is_some() {
                    if job.pages >= PAGES {
                        UpdateState::ScanLimit
                    } else {
                        job.cursor = page.next.clone();
                        let reason = job.reason;
                        self.refresh = Some(job);
                        UpdateState::Refreshing(reason)
                    }
                } else {
                    UpdateState::Refreshed(job.reason)
                }
            }
        };
        self.volume = Some(files.volume());
        self.dirty = true;
    }
}
