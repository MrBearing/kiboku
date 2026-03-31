use std::fs::{create_dir_all, write};
use std::process::Command;

use tempfile::tempdir;

fn write_min_workspace(base: &std::path::Path) {
    let src = base.join("src");
    let pkg_dir = src.join("my_pkg");
    create_dir_all(pkg_dir.join("src")).expect("mkdir");

    write(
        pkg_dir.join("package.xml"),
        r#"<package format="2"><name>my_pkg</name><version>0.1.0</version><depend>roscpp</depend></package>"#,
    )
    .expect("write package.xml");

    write(
        pkg_dir.join("CMakeLists.txt"),
        "cmake_minimum_required(VERSION 3.0.2)\nfind_package(catkin REQUIRED COMPONENTS roscpp)\n",
    )
    .expect("write CMakeLists.txt");

    write(
        pkg_dir.join("src").join("main.cpp"),
        "#include <ros/ros.h>\nint main() { return 0; }\n",
    )
    .expect("write main.cpp");
}

#[test]
fn run_subcommand_generates_report_bundle() {
    let td = tempdir().expect("tempdir");
    let base = td.path();

    write_min_workspace(base);

    let out_dir = base.join("results");
    let cfg_path = base.join("report.toml");
    write(&cfg_path, "title = \"Run Test\"\n").expect("write report.toml");

    let exe = env!("CARGO_BIN_EXE_bok");
    let output = Command::new(exe)
        .args([
            "run",
            base.to_str().unwrap(),
            "--platform",
            "ros1",
            "--output",
            out_dir.to_str().unwrap(),
            "--config",
            cfg_path.to_str().unwrap(),
        ])
        .output()
        .expect("run bok run");

    assert!(output.status.success(), "bok run failed: {output:?}");

    assert!(out_dir.join("index.html").exists(), "index.html missing");
    assert!(
        out_dir.join("assets").join("app.js").exists(),
        "assets/app.js missing"
    );
    assert!(
        out_dir.join("assets").join("style.css").exists(),
        "assets/style.css missing"
    );
}

#[test]
fn run_shorthand_generates_report_bundle() {
    let td = tempdir().expect("tempdir");
    let base = td.path();

    write_min_workspace(base);

    let out_dir = base.join("results");

    let exe = env!("CARGO_BIN_EXE_bok");
    let output = Command::new(exe)
        .args([
            base.to_str().unwrap(),
            "--platform",
            "ros1",
            "--output",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run bok <WORKSPACE>");

    assert!(
        output.status.success(),
        "bok WORKSPACE shorthand failed: {output:?}"
    );

    assert!(out_dir.join("index.html").exists(), "index.html missing");
    assert!(
        out_dir.join("assets").join("app.js").exists(),
        "assets/app.js missing"
    );
    assert!(
        out_dir.join("assets").join("style.css").exists(),
        "assets/style.css missing"
    );
}
