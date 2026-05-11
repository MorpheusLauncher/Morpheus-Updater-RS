fn main() -> std::io::Result<()> {
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("morpheus.ico")
            .compile()?;
    }
    Ok(())
}