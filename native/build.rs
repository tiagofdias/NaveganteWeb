fn main() {
    println!("cargo:rerun-if-changed=../desktop-assets/app.ico");
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../desktop-assets/app.ico")
            .set("ProductName", "Navegante Desktop")
            .set("FileDescription", "Navegante Desktop — aplicação nativa")
            .set_manifest_file("app.manifest")
            .compile()
            .expect("Não foi possível compilar os recursos Windows");
    }
}
