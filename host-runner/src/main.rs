#[cfg(feature = "gui")]
mod gui;
mod snapshot;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay};
use radishroot_kernel::{
    input_view::View,
    surface::{Geometry, Surface},
};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn main() {
    if let Err(error) = run() {
        eprintln!("host-view: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let [output, rest @ ..] = args.as_slice() else {
        return Err("expected output-directory [--state serial-frame-file | --gui]".into());
    };
    let output = Path::new(output);
    if !output.is_dir() {
        return Err("output directory must already exist".into());
    }
    let (mut view, clipping) = match rest {
        [] => (
            View::new(Geometry::new(1024, 768).map_err(|e| e.code())?),
            false,
        ),
        [flag, path] if flag == "--state" => snapshot::read(Path::new(path))?,
        [flag] if flag == "--gui" => {
            #[cfg(feature = "gui")]
            {
                return gui::run(output);
            }
            #[cfg(not(feature = "gui"))]
            {
                return Err("GUI feature was not built".into());
            }
        }
        _ => return Err("invalid host-view arguments".into()),
    };
    save_with_probe(&mut view, output, "frame", clipping)?;
    println!("host-view=headless output={}", output.display());
    Ok(())
}
fn render(view: &mut View) -> Result<(Geometry, Vec<u32>)> {
    let geometry = view.geometry();
    let mut pixels = vec![0; geometry.pixels()];
    view.render(&mut Surface::new(geometry, &mut pixels).map_err(|e| e.code())?);
    Ok((geometry, pixels))
}
#[cfg(feature = "gui")]
fn save(view: &mut View, output: &Path, name: &str) -> Result<()> {
    save_with_probe(view, output, name, false)
}
fn save_with_probe(view: &mut View, output: &Path, name: &str, clipping: bool) -> Result<()> {
    let (geometry, mut pixels) = render(view)?;
    if clipping {
        radishroot_kernel::surface::boundary_probe(
            &mut Surface::new(geometry, &mut pixels).map_err(|e| e.code())?,
        );
    }
    let mut ppm = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join(format!("{name}.ppm")))?;
    write!(ppm, "P6\n{} {}\n255\n", geometry.width(), geometry.height())?;
    let rgb = pixels
        .iter()
        .flat_map(|v| [(v >> 16) as u8, (v >> 8) as u8, *v as u8])
        .collect::<Vec<_>>();
    ppm.write_all(&rgb)?;
    let mut display = SimulatorDisplay::<Rgb888>::new(geometry.size());
    display
        .draw_iter(pixels.iter().enumerate().map(|(i, p)| {
            Pixel(
                Point::new(
                    (i % geometry.width() as usize) as i32,
                    (i / geometry.width() as usize) as i32,
                ),
                Rgb888::new((p >> 16) as u8, (p >> 8) as u8, *p as u8),
            )
        }))
        .unwrap();
    let png = output.join(format!("{name}.png"));
    // 先独占保留输出名，不能覆盖另一运行的材料。
    let mut file = OpenOptions::new().write(true).create_new(true).open(png)?;
    let settings = OutputSettingsBuilder::new()
        .scale(1)
        .pixel_spacing(0)
        .build();
    // PNG 编码由批准的 simulator 完成；临时文件也只在本任务独占目录内。
    let temporary = output.join(format!("{name}.encoding.png"));
    if temporary.exists() {
        return Err("PNG temporary path already exists".into());
    }
    display
        .to_rgb_output_image(&settings)
        .save_png(&temporary)?;
    file.write_all(&fs::read(&temporary)?)?;
    fs::remove_file(temporary)?;
    Ok(())
}
