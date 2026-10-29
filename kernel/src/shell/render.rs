//! 显式 JSON 展示投影。领域结果保持类型，字节输出不推断文件编码或完整性。
use super::{Error, Output, Reply, parser::COMMANDS};
use crate::services::*;
use core::fmt::{self, Write};
pub fn string(w: &mut impl Write, text: &str) -> fmt::Result {
    w.write_char('"')?;
    for c in text.chars() {
        match c {
            '"' => w.write_str("\\\"")?,
            '\\' => w.write_str("\\\\")?,
            '\n' => w.write_str("\\n")?,
            '\r' => w.write_str("\\r")?,
            '\t' => w.write_str("\\t")?,
            c if c.is_control() => write!(w, "\\u{:04x}", c as u32)?,
            c => w.write_char(c)?,
        }
    }
    w.write_char('"')
}
pub fn id(w: &mut impl Write, id: OperationId) -> fmt::Result {
    write!(
        w,
        "\"{:032x}:{}\"",
        u128::from_be_bytes(id.boot.0),
        id.sequence
    )
}
fn effect(w: &mut impl Write, e: PartialEffect) -> fmt::Result {
    write!(
        w,
        "{{\"created\":{},\"written_bytes\":{},\"may_have_changed\":{},\"persistence_confirmed\":{}}}",
        e.created, e.written_bytes, e.may_have_changed, e.persistence_confirmed
    )
}
fn service_error(w: &mut impl Write, e: &SystemError) -> fmt::Result {
    write!(
        w,
        "{{\"code\":\"{:?}\",\"stage\":\"{:?}\",\"object\":",
        e.code, e.stage
    )?;
    if let Some(path) = &e.object {
        string(w, path.as_str())?
    } else {
        w.write_str("null")?
    }
    w.write_str(",\"cause\":")?;
    string(w, &alloc::format!("{:?}", e.cause))?;
    w.write_str(",\"partial_effect\":")?;
    effect(w, e.partial_effect)?;
    w.write_char('}')
}
fn record(w: &mut impl Write, r: &OperationRecord) -> fmt::Result {
    w.write_str("{\"operation_id\":")?;
    id(w, r.id)?;
    write!(
        w,
        ",\"request_kind\":\"{:?}\",\"stage\":\"{:?}\",\"status\":\"{:?}\",\"last_completed\":",
        r.request_kind, r.stage, r.status
    )?;
    if let Some(stage) = r.last_completed {
        write!(w, "\"{stage:?}\"")?
    } else {
        w.write_str("null")?
    }
    w.write_str(",\"target\":")?;
    if let Some(p) = &r.target {
        string(w, p.as_str())?
    } else {
        w.write_str("null")?
    }
    w.write_str(",\"partial_effect\":")?;
    effect(w, r.partial_effect)?;
    w.write_str(",\"error\":")?;
    if let Some(e) = &r.error {
        service_error(w, e)?
    } else {
        w.write_str("null")?
    }
    w.write_char('}')
}
fn reference(w: &mut impl Write, r: &FileRef) -> fmt::Result {
    write!(
        w,
        "{{\"boot_id\":\"{:032x}\",\"volume\":\"{:032x}\",\"generation\":{},\"path\":",
        u128::from_be_bytes(r.boot.0),
        u128::from_be_bytes(r.volume.0),
        r.generation
    )?;
    string(w, r.path.as_str())?;
    w.write_char('}')
}
pub fn reply(w: &mut impl Write, r: &Reply) -> fmt::Result {
    write!(
        w,
        "{{\"schema\":\"radishroot.command.v1\",\"command_id\":{},\"operation_id\":",
        r.command_id
    )?;
    if let Some(operation) = r.operation_id {
        id(w, operation)?
    } else {
        w.write_str("null")?
    }
    match &r.result {
        Err(e) => {
            w.write_str(",\"ok\":false,\"error\":")?;
            match e {
                Error::Service(e) => {
                    w.write_str("{\"kind\":\"service\",\"detail\":")?;
                    service_error(w, e)?;
                    w.write_char('}')?
                }
                Error::Parse(e) => write!(w, "{{\"kind\":\"parse\",\"code\":\"{e:?}\"}}")?,
                Error::ServiceIdentity(e) => {
                    write!(w, "{{\"kind\":\"identity\",\"code\":\"{e:?}\"}}")?
                }
                Error::NoNextPage => w.write_str("{\"kind\":\"NoNextPage\"}")?,
                Error::ForeignBoot => w.write_str("{\"kind\":\"ForeignBoot\"}")?,
                Error::OperationUnavailable { evicted } => write!(
                    w,
                    "{{\"kind\":\"OperationUnavailable\",\"evicted\":{evicted}}}"
                )?,
            }
        }
        Ok(output) => {
            w.write_str(",\"ok\":true,\"output\":")?;
            match output {
                Output::Help => {
                    w.write_str("{\"kind\":\"help\",\"commands\":[")?;
                    for (i, c) in COMMANDS.iter().enumerate() {
                        if i != 0 {
                            w.write_char(',')?
                        }
                        w.write_str("{\"name\":")?;
                        string(w, c.name)?;
                        w.write_str(",\"usage\":")?;
                        string(w, c.usage)?;
                        write!(w, ",\"modifies\":{},\"result\":", c.modifies)?;
                        string(w, c.result)?;
                        w.write_char('}')?
                    }
                    w.write_str("]}")?;
                }
                Output::Files(page) => {
                    write!(
                        w,
                        "{{\"kind\":\"files\",\"more\":{},\"entries\":[",
                        page.next.is_some()
                    )?;
                    for (i, e) in page.entries.iter().enumerate() {
                        if i != 0 {
                            w.write_char(',')?
                        }
                        w.write_str("{\"name\":")?;
                        string(w, &e.name)?;
                        write!(w, ",\"kind\":\"{:?}\",\"size_bytes\":", e.kind)?;
                        if let Some(n) = e.size_bytes {
                            write!(w, "{n}")?
                        } else {
                            w.write_str("null")?
                        }
                        w.write_str(",\"reference\":")?;
                        reference(w, &e.reference)?;
                        w.write_char('}')?
                    }
                    w.write_str("]}")?;
                }
                Output::Bytes { bytes, eof, offset } => {
                    write!(
                        w,
                        "{{\"kind\":\"bytes\",\"offset\":{offset},\"length\":{},\"eof\":{eof},\"hex\":\"",
                        bytes.len()
                    )?;
                    for b in bytes {
                        write!(w, "{b:02x}")?
                    }
                    w.write_str("\",\"text\":")?;
                    if let Ok(s) = core::str::from_utf8(bytes) {
                        string(w, s)?
                    } else {
                        w.write_str("null")?
                    }
                    w.write_char('}')?;
                }
                Output::Created(r) => {
                    w.write_str("{\"kind\":\"created\",\"reference\":")?;
                    reference(w, r)?;
                    w.write_char('}')?;
                }
                Output::Operations { records, evicted } => {
                    write!(
                        w,
                        "{{\"kind\":\"operations\",\"evicted\":{evicted},\"records\":["
                    )?;
                    for (i, r) in records.iter().enumerate() {
                        if i != 0 {
                            w.write_char(',')?
                        }
                        record(w, r)?
                    }
                    w.write_str("]}")?;
                }
                Output::Explanation {
                    record: r,
                    events,
                    events_evicted,
                } => {
                    write!(
                        w,
                        "{{\"kind\":\"explanation\",\"events_evicted\":{events_evicted},\"record\":"
                    )?;
                    record(w, r)?;
                    w.write_str(",\"events\":[")?;
                    for (i, e) in events.iter().enumerate() {
                        if i != 0 {
                            w.write_char(',')?
                        }
                        write!(
                            w,
                            "{{\"seq\":{},\"stage\":\"{:?}\",\"status\":\"{:?}\"}}",
                            e.seq, e.stage, e.status
                        )?
                    }
                    w.write_str("]}")?;
                }
            }
        }
    }
    w.write_char('}')
}
