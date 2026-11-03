//! 宿主只读磁盘适配；工作台没有宿主路径 / 文件内容的旁路。
use crate::{Result, save_with_probe};
use radishroot_kernel::{
    services::{BootId, FileService, fat::FatBackend},
    storage::{
        block::{self, BlockDevice},
        gpt,
    },
    surface::Geometry,
    workbench::{View, replay},
};
use std::{
    fs::{self, File},
    os::unix::fs::FileExt,
    path::Path,
};
struct Disk {
    file: File,
    sectors: u64,
}
impl BlockDevice for Disk {
    fn info(&self) -> block::Info {
        block::Info {
            sectors: self.sectors,
            readonly: true,
            flush: true,
        }
    }
    fn read_blocks(&mut self, lba: u64, bytes: &mut [u8]) -> std::result::Result<(), block::Error> {
        block::validate(self.info(), lba, bytes.len(), false)?;
        self.file
            .read_exact_at(bytes, lba * 512)
            .map_err(|_| block::Error::new(block::Kind::Io, block::Stage::Read, lba))
    }
    fn write_blocks(&mut self, lba: u64, _: &[u8]) -> std::result::Result<(), block::Error> {
        Err(block::Error::new(
            block::Kind::ReadOnly,
            block::Stage::Write,
            lba,
        ))
    }
    fn flush(&mut self) -> std::result::Result<(), block::Error> {
        self.file
            .sync_all()
            .map_err(|_| block::Error::new(block::Kind::Io, block::Stage::Flush, 0))
    }
}
pub fn run(output: &Path, args: &[String]) -> Result<()> {
    let [flag, disk, id_flag, id, rest @ ..] = args else {
        return Err(
            "expected --workbench-disk IMAGE --boot-id HEX [--replay STEPS | --gui]".into(),
        );
    };
    if flag != "--workbench-disk"
        || id_flag != "--boot-id"
        || id.len() != 32
        || !id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid workbench disk / identity arguments".into());
    }
    let mut boot = [0; 16];
    for (i, b) in boot.iter_mut().enumerate() {
        *b = u8::from_str_radix(&id[i * 2..i * 2 + 2], 16)?;
    }
    if boot == [0; 16] {
        return Err("zero boot identity".into());
    }
    let path = Path::new(disk).canonicalize()?;
    let file = File::open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.len() > 1024 * 1024 * 1024
        || metadata.len() < 512 * 100
        || metadata.len() % 512 != 0
    {
        return Err("expected bounded ordinary disk image".into());
    }
    let mut disk = Disk {
        file,
        sectors: metadata.len() / 512,
    };
    let layout = gpt::inspect(&mut disk).map_err(|e| format!("GPT: {e:?}"))?;
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), BootId(boot))
        .map_err(|e| format!("service: {e:?}"))?;
    // 默认 guest 在图形初始化前执行同一个探测，保持操作编号顺序。
    let _ = files
        .execute(
            &radishroot_kernel::services::path::Path::root(),
            radishroot_kernel::services::Request::List {
                path: "/data",
                cursor: None,
            },
        )
        .map_err(|e| format!("probe: {e:?}"))?;
    let mut view = View::new(Geometry::new(1024, 768).map_err(|e| e.code())?);
    match rest {
        [] => view.execute_pending(&mut files),
        [flag, path] if flag == "--replay" => {
            let path = Path::new(path);
            if fs::metadata(path)?.len() > 1024 * 1024 {
                return Err("replay exceeds 1 MiB".into());
            }
            for line in fs::read_to_string(path)?.lines() {
                if line == "execute" {
                    view.execute_pending(&mut files);
                } else if line == "observe" {
                    view.observe_changes(&files);
                } else {
                    view.event(replay::parse(line).ok_or("invalid workbench replay event")?);
                }
            }
        }
        [flag] if flag == "--gui" => {
            #[cfg(feature = "gui")]
            {
                return crate::gui::run_view(output, view, |v| v.execute_pending(&mut files));
            }
            #[cfg(not(feature = "gui"))]
            {
                return Err("GUI feature was not built".into());
            }
        }
        _ => return Err("invalid workbench arguments".into()),
    }
    save_with_probe(&mut view, output, "frame", false)?;
    println!(
        "host-view=workbench readonly=1 source={} results={} events={}",
        path.display(),
        view.results.len(),
        view.events
    );
    Ok(())
}
