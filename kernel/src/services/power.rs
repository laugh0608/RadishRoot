//! 同一服务的电源阶段；平台只消费已同步请求，不向 UI 暴露设备。
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Sync,
    Reboot,
    Shutdown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Preparing(Action, OperationId),
    Prepared(Action, OperationId),
    Stopped(Action, OperationId),
    Failed(Action, OperationId),
    Requested(Action, OperationId),
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Preparing(..) => "draining",
            Self::Prepared(..) => "prepared",
            Self::Stopped(..) => "stopped",
            Self::Failed(..) => "failed",
            Self::Requested(..) => "requested",
        }
    }
}
pub(super) fn request_power(request: &Request<'_>) -> Action {
    match request {
        Request::Power(a) => *a,
        _ => unreachable!("power request"),
    }
}
impl<B: Backend> FileService<B> {
    pub fn power(&self) -> State {
        self.power
    }
    pub fn prepared_power(&self) -> Option<(Action, OperationId)> {
        match self.power {
            State::Prepared(a, id) => Some((a, id)),
            _ => None,
        }
    }
    fn power_transition(&mut self, stage: Stage, status: Status, error: Option<SystemError>) {
        let id = match self.power {
            State::Prepared(_, id)
            | State::Stopped(_, id)
            | State::Requested(_, id)
            | State::Failed(_, id) => id,
            _ => return,
        };
        if let Some(r) = self.operations.entries.iter_mut().find(|r| r.id == id) {
            r.stage = stage;
            r.status = status;
            if status != Status::Failed {
                r.last_completed = Some(stage);
            }
            r.error = error;
        }
        // execute 在 IO 前预留 32 个序号；后续电源阶段至多三个，其他请求仍按同一预留门禁拒绝。
        self.events.push(OperationEvent {
            operation_id: id,
            seq: self.next_event,
            stage,
            status,
        });
        self.next_event = self
            .next_event
            .checked_add(1)
            .expect("reserved power event identity");
    }
    pub fn fail_power(&mut self, stage: Stage, cause: Cause) {
        let (a, id) = match self.power {
            State::Prepared(a, id) | State::Stopped(a, id) | State::Requested(a, id) => (a, id),
            _ => return,
        };
        self.power = State::Failed(a, id);
        self.power_transition(
            stage,
            Status::Failed,
            Some(SystemError {
                code: cause.code(),
                stage,
                object: Some(Path::root()),
                cause,
                partial_effect: PartialEffect {
                    persistence_confirmed: true,
                    ..PartialEffect::default()
                },
            }),
        );
    }
    /// 已同步且设备停止后，在执行寄存器请求前记录；不是成功关机终态。
    pub fn power_requested(&mut self) {
        if let State::Stopped(a, id) = self.power {
            self.power = State::Requested(a, id);
            self.power_transition(Stage::Platform, Status::Requested, None);
        }
    }
}
impl<D: crate::storage::block::Quiesce> FileService<fat::FatBackend<'_, D>> {
    pub fn quiesce_for_power(&mut self) -> Result<(), Cause> {
        if self.prepared_power().is_none() {
            return Err(Cause::Contract(Code::InvalidArgument));
        }
        match self.backend.device.stop() {
            Ok(()) => {
                let State::Prepared(a, id) = self.power else {
                    unreachable!()
                };
                self.power = State::Stopped(a, id);
                self.volume.mount_state = MountState::Offline;
                self.power_transition(Stage::DeviceStop, Status::Running, None);
                Ok(())
            }
            Err(e) => {
                let cause = Cause::Block(e);
                self.volume.mount_state = MountState::Offline;
                self.fail_power(Stage::DeviceStop, cause);
                Err(cause)
            }
        }
    }
}
#[cfg(test)]
mod tests;
