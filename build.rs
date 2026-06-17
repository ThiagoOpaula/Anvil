fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows"
        && let Err(e) = winres::WindowsResource::new()
            .set_manifest(
                r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
        <requestedPrivileges>
            <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
        </requestedPrivileges>
    </security>
</trustInfo>
</assembly>"#,
            )
            .compile()
        {
            // winres::compile may fail if the Windows SDK is not installed.
            // In that case, print a warning but don't stop the build.
            println!("cargo:warning=failed to embed Windows manifest: {}", e);
        }
}
