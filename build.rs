use serde_json::Value;
use std::process::{Command, Output};

fn get_cargo_metadata() -> Value {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("Failed to run cargo metadata");
    serde_json::from_slice(&output.stdout).expect("Failed to parse cargo metadata JSON")
}

fn get_package_version<'a>(packages: &'a [Value], name: &str) -> &'a str {
    packages
        .iter()
        .find(|pkg| pkg.get("name").and_then(|v| v.as_str()) == Some(name))
        .and_then(|pkg| pkg.get("version").and_then(|v| v.as_str()))
        .unwrap_or("unknown")
}

fn print_command_output(label: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        println!("cargo:warning=[{} stdout] {}", label, line);
    }

    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        println!("cargo:warning=[{} stderr] {}", label, line);
    }
}

fn run_npm_build() {
    // 检查 npm 是否可用
    let npm_check = Command::new("npm").arg("--version").output();

    if npm_check.is_err() {
        println!("cargo:warning=未检测到 npm，跳过前端构建");
        return;
    }

    let install_result = Command::new("npm")
        .arg("install")
        .current_dir("web")
        .output();

    match install_result {
        Ok(output) => {
            if !output.status.success() {
                println!("cargo:warning=npm install 失败");
                print_command_output("npm install", &output);
                println!(
                    "cargo:warning=npm install 失败，退出码: {:?}",
                    output.status.code()
                );
                return;
            }
        }
        Err(e) => {
            println!("cargo:warning=npm install 失败: {}", e);
            return;
        }
    }

    let build_result = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("web")
        .output();

    match build_result {
        Ok(output) => {
            if !output.status.success() {
                println!("cargo:warning=前端构建失败");
                print_command_output("npm run build", &output);
                println!(
                    "cargo:warning=前端构建失败，退出码: {:?}",
                    output.status.code()
                );
            }
        }
        Err(e) => {
            println!("cargo:warning=npm run build 失败: {}", e);
        }
    }
}

fn main() {
    // 判断是否为 release 构建
    let is_release = std::env::var("PROFILE").unwrap_or_default() == "release";

    // 只获取一次 metadata
    let metadata = get_cargo_metadata();

    if !is_release {
        // 构建静态文件
        run_npm_build();
    } else {
        println!("cargo:warning=Release 构建，跳过 npm 构建");
    }

    // 补充版本号
    let app_name = env!("CARGO_PKG_NAME");
    let wp_parse_pkg_name = "wp-engine";

    let packages = metadata
        .get("packages")
        .and_then(|v| v.as_array())
        .expect("No packages found in cargo metadata");

    let wp_station = get_package_version(packages, app_name);
    let wp_parse = get_package_version(packages, wp_parse_pkg_name);

    println!("cargo:rustc-env=WP_STATION_VERSION={}", wp_station);
    println!("cargo:rustc-env=WP_PARSE_VERSION={}", wp_parse);
}
