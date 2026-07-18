#[cfg(windows)]
fn main() {
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icons/seex.ico");
    resource
        .compile()
        .expect("failed to embed Windows application icon");
}

#[cfg(not(windows))]
fn main() {}
