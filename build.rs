use std::fs;
use std::path::PathBuf;

fn render_png(tree: &resvg::usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size, size).expect("failed to allocate SeEx logo raster");
    // seex.svg uses a 1024 x 1024 viewBox. Keep the source coordinate system
    // explicit so the whole mark is rendered into every generated icon size.
    let scale = size as f32 / 1024.0;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("failed to encode SeEx PNG logo")
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/seex.svg");

    let svg = fs::read_to_string("assets/icons/seex.svg").expect("failed to read SeEx SVG logo");
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&svg, &options).expect("failed to parse SeEx SVG logo");
    let png = render_png(&tree, 256);
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let png_path = out_dir.join("seex.png");
    fs::write(&png_path, &png).expect("failed to write SeEx PNG logo");

    #[cfg(windows)]
    {
        let images = [16u32, 32, 48, 256]
            .into_iter()
            .map(|size| (size, render_png(&tree, size)))
            .collect::<Vec<_>>();
        let header_size = 6 + images.len() * 16;
        let payload_size = images.iter().map(|(_, data)| data.len()).sum::<usize>();
        let mut ico = Vec::with_capacity(header_size + payload_size);
        ico.extend_from_slice(&0u16.to_le_bytes());
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&(images.len() as u16).to_le_bytes());
        let mut offset = header_size as u32;
        for (size, data) in &images {
            let dimension = if *size >= 256 { 0 } else { *size as u8 };
            ico.extend_from_slice(&[dimension, dimension, 0, 0]);
            ico.extend_from_slice(&1u16.to_le_bytes());
            ico.extend_from_slice(&32u16.to_le_bytes());
            ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
            ico.extend_from_slice(&offset.to_le_bytes());
            offset += data.len() as u32;
        }
        for (_, data) in &images {
            ico.extend_from_slice(data);
        }
        let ico_path = out_dir.join("seex.ico");
        fs::write(&ico_path, ico).expect("failed to write SeEx ICO logo");

        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(ico_path.to_str().expect("ICO path must be valid UTF-8"));
        resource
            .compile()
            .expect("failed to embed Windows application icon");
    }
}
