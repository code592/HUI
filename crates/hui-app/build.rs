use std::{env, path::PathBuf, process::Command};

fn windows_icon() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }
    let icon = PathBuf::from("../../packaging/windows/hui.ico")
        .canonicalize()
        .expect("missing generated Windows icon");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-env-changed=RC");
    println!("cargo:rerun-if-env-changed=WindowsSdkDir");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let rc = out.join("hui-icon.rc");
    std::fs::write(
        &rc,
        format!("1 ICON \"{}\"\n", icon.to_string_lossy().replace('\\', "/")),
    )
    .unwrap();
    let msvc = target.contains("msvc");
    let output = out.join(if msvc { "hui-icon.res" } else { "hui-icon.o" });
    let compiler = env::var_os("RC").map(PathBuf::from).unwrap_or_else(|| {
        if !msvc {
            return PathBuf::from("windres");
        }
        let sdk = env::var_os("WindowsSdkDir")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    env::var_os("ProgramFiles(x86)")
                        .unwrap_or_else(|| "C:/Program Files (x86)".into()),
                )
                .join("Windows Kits/10")
            });
        let mut versions: Vec<_> = std::fs::read_dir(sdk.join("bin"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        versions.sort();
        let host = if env::consts::ARCH == "aarch64" {
            "arm64"
        } else {
            "x64"
        };
        versions
            .into_iter()
            .rev()
            .map(|p| p.join(host).join("rc.exe"))
            .find(|p| p.is_file())
            .unwrap_or_else(|| "rc.exe".into())
    });
    let mut command = Command::new(&compiler);
    if msvc {
        command.arg("/nologo").arg("/fo").arg(&output).arg(&rc);
    } else {
        command
            .arg("--input")
            .arg(&rc)
            .arg("--output-format=coff")
            .arg("--output")
            .arg(&output);
    }
    let status = command.status().expect(
        "Windows icon compilation needs Windows SDK rc.exe or RC pointing to a resource compiler",
    );
    assert!(
        status.success(),
        "Windows application icon resource compilation failed"
    );
    println!("cargo:rustc-link-arg-bin=hui={}", output.display());
}

fn main() {
    windows_icon();
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new().with_style("fluent".into()),
    )
    .unwrap();
}
