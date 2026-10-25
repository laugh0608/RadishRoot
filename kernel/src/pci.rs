use crate::platform_memory;
use core::mem::ManuallyDrop;
use radishroot_kernel::{
    boot_contract::Region,
    pci_contract::{self, Bar},
};
use virtio_drivers::transport::{
    DeviceStatus, DeviceType, Transport,
    pci::{
        PciTransport,
        bus::{BarInfo, ConfigurationAccess, DeviceFunction, PciRoot},
    },
};
use x86_64::instructions::port::Port;
fn address(bdf: DeviceFunction, offset: u8) -> u32 {
    assert!(bdf.valid() && offset.is_multiple_of(4));
    0x80000000
        | u32::from(bdf.bus) << 16
        | u32::from(bdf.device) << 11
        | u32::from(bdf.function) << 8
        | u32::from(offset)
}
fn read(bdf: DeviceFunction, offset: u8) -> u32 {
    // SAFETY: q35 配置机制 1 的 CF8/CFC，唯一 BSP / IF=0，无并发 PCI 访问；offset 已对齐。
    unsafe {
        Port::<u32>::new(0xcf8).write(address(bdf, offset));
        Port::<u32>::new(0xcfc).read()
    }
}
fn write(bdf: DeviceFunction, offset: u8, value: u32) {
    // SAFETY: 同上，调用点仅访问已选定设备的 command / BAR，不访问其他设备。
    unsafe {
        Port::<u32>::new(0xcf8).write(address(bdf, offset));
        Port::<u32>::new(0xcfc).write(value);
    }
}
struct Config {
    device: DeviceFunction,
    readonly: bool,
}
impl ConfigurationAccess for Config {
    fn read_word(&self, bdf: DeviceFunction, offset: u8) -> u32 {
        assert_eq!(bdf, self.device);
        read(bdf, offset)
    }
    fn write_word(&mut self, bdf: DeviceFunction, offset: u8, value: u32) {
        assert_eq!(bdf, self.device);
        assert!(!self.readonly);
        assert!(offset == 4 || (0x10..=0x24).contains(&offset));
        if offset == 4 {
            assert_eq!(value & 4, 0, "T03 bus master must remain disabled");
        }
        write(bdf, offset, value);
    }
    unsafe fn unsafe_clone(&self) -> Self {
        Self {
            device: self.device,
            readonly: true,
        }
    }
}
pub struct Device {
    pub transport: ManuallyDrop<PciTransport>,
    pub bdf: DeviceFunction,
    pub bars: [Bar; 6],
    pub capabilities: usize,
    pub features: u64,
    pub queue_size: u32,
    pub bus_master: bool,
}
pub fn discover(regions: &[Region], bad_capability: bool) -> Result<Device, &'static str> {
    // 固定配置没有 PCI bridge / 多 bus；只扫描 root bus 0，不能声称通用 PCI 拓扑发现。
    let mut found = None;
    for device in 0..32 {
        for function in 0..8 {
            let bdf = DeviceFunction {
                bus: 0,
                device,
                function,
            };
            if read(bdf, 0) == 0x10421af4 && found.replace(bdf).is_some() {
                return Err("PCI_MULTIPLE");
            }
        }
    }
    let bdf = found.ok_or("PCI_NOT_FOUND")?;
    let original_command = read(bdf, 4) as u16;
    // T03 不提交 IO，先禁 bus master 和地址解码；写低16位，避免清除 W1C status。
    write(bdf, 4, u32::from(original_command & !7));
    if read(bdf, 4) & 7 != 0 {
        return Err("PCI_DISABLE");
    }
    let mut snapshot = [0u8; 256];
    for offset in (0..256).step_by(4) {
        snapshot[offset..offset + 4].copy_from_slice(&read(bdf, offset as u8).to_le_bytes());
    }
    // 在调用库的 BAR 探测前拒绝非法类型，避免其早退留下全1 BAR。
    let mut index = 0;
    while index < 6 {
        let value = read(bdf, 0x10 + index * 4);
        if value != 0 && value & 1 != 0 {
            return Err("PCI_IO_BAR");
        }
        match value & 6 {
            0 => index += 1,
            4 if index < 5 => index += 2,
            _ => return Err("PCI_BAR_TYPE"),
        }
    }
    let mut root = PciRoot::new(Config {
        device: bdf,
        readonly: false,
    });
    let mut bars = [Bar::default(); 6];
    for (index, bar) in root
        .bars(bdf)
        .map_err(|_| "PCI_BAR_PROBE")?
        .iter()
        .enumerate()
    {
        if let Some(BarInfo::Memory { address, size, .. }) = bar {
            bars[index] = Bar {
                base: *address,
                length: *size,
            };
        }
        if read(bdf, 0x10 + index as u8 * 4).to_le_bytes()
            != snapshot[0x10 + index * 4..0x14 + index * 4]
        {
            return Err("PCI_BAR_RESTORE");
        }
    }
    // capability 的绝对范围计算前先验证完整 BAR，不能等到建立映射时才拒绝溢出。
    pci_contract::bars(&bars, regions).map_err(|_| "PCI_BAR_RANGE")?;
    if bad_capability {
        snapshot[0x34] = 0x40;
        snapshot[0x40] = 1;
        snapshot[0x41] = 0x40;
    }
    let capabilities =
        pci_contract::capabilities(&snapshot, &bars).map_err(|_| "PCI_CAPABILITY")?;
    platform_memory::map_bars(bars, regions)?;
    // 只恢复 memory decode，仍禁 bus master / INTx；直到 T04 建立请求与超时所有权再开启 DMA。
    write(bdf, 4, u32::from((original_command & !5) | 2 | 0x400));
    let mut transport = ManuallyDrop::new(
        PciTransport::new::<platform_memory::VirtioHal, _>(&mut root, bdf)
            .map_err(|_| "PCI_TRANSPORT")?,
    );
    if transport.device_type() != DeviceType::Block {
        return Err("PCI_DEVICE_TYPE");
    }
    // 读取真实寄存器，不建立队列或 DRIVER_OK；设备不能访问 RAM。
    let features = transport.read_device_features();
    let queue_size = transport.max_queue_size(0);
    if features & (1 << 32) == 0 || queue_size == 0 {
        return Err("PCI_VIRTIO_CONFIG");
    }
    let bus_master = read(bdf, 4) & 4 != 0;
    if bus_master {
        return Err("PCI_BUS_MASTER");
    }
    // 上游 Drop 使用无界复位等待；这里拥有明确的有界复位，故不执行该 Drop。
    // 映射 / claims 保留到 halt；未建立 DMA 队列，无待释放的设备缓冲区。
    transport.set_status(DeviceStatus::empty());
    let mut stopped = false;
    for _ in 0..100_000 {
        if transport.get_status().is_empty() {
            stopped = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !stopped {
        return Err("PCI_RESET_TIMEOUT");
    }
    Ok(Device {
        transport,
        bdf,
        bars,
        capabilities,
        features,
        queue_size,
        bus_master,
    })
}

pub fn bus_master(bdf: DeviceFunction, enabled: bool) -> Result<(), &'static str> {
    if read(bdf, 0) != 0x10421af4 {
        return Err("PCI_IDENTITY_CHANGED");
    }
    let command = read(bdf, 4) as u16;
    let value = if enabled { command | 4 } else { command & !4 };
    write(bdf, 4, u32::from(value));
    if (read(bdf, 4) & 4 != 0) != enabled {
        return Err("PCI_BUS_MASTER");
    }
    Ok(())
}
