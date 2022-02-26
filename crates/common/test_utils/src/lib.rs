use assert_cmd::prelude::*;
use std::{process::Command, thread, time};
pub fn hello_world() {
    println!("Hello World")
}

pub fn tedge_command<I, S>(args: I) -> Result<assert_cmd::Command, Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let path: &str = "tedge";
    let mut cmd = assert_cmd::Command::cargo_bin(path)?;
    cmd.args(args);
    Ok(cmd)
}

pub fn tedge_mapper_command<I, S>(
    args: I,
) -> Result<assert_cmd::Command, Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let path: &str = "tedge_mapper";
    let mut cmd = assert_cmd::Command::cargo_bin(path)?;
    cmd.args(args);
    Ok(cmd)
}

pub fn tedge_agent_command<I, S>(args: I) -> Result<assert_cmd::Command, Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let path: &str = "tedge_agent";
    let mut cmd = assert_cmd::Command::cargo_bin(path)?;
    cmd.args(args);
    Ok(cmd)
}

macro_rules! tedge_connect_c8y {
    ($config_dir:literal) => {{
        use crate::*;
        let mapper_cmd: assert_cmd::Command =
            tedge_mapper_command(["--config-dir", $config_dir]).unwrap();
        mapper_cmd.spawn();
    }};
}

#[macro_export]
macro_rules! tedge {
    (config list all --config-dir $dir:expr) => {{
        use crate::*;
        tedge_command(["--config-dir", $dir, "config", "list", "--all"]).unwrap()
    }};
    (config get $key:expr) => {{
        use crate::*;
        tedge_command(["config", "get", $key]).unwrap()
    }};
    (config set $key:expr,$value:expr) => {{
        use crate::*;
        tedge_command(["config", "set", $key, $value]).unwrap()
    }};
    (config set $a:expr, $b:expr ) => {{}};
}
fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

macro_rules! tedge_config_get {
    ($key:literal) => {{
        use crate::*;
        tedge_command(["config", "get", $key]).unwrap()
    }};

    ($key:literal $config_dir:ident) => {{
        use crate::*;
        let mut cmd = tedge_command(["--config-dir", $config_dir, "config", "get", $key]).unwrap();
        String::from_utf8(cmd.output().unwrap().stdout).unwrap()
    }};
}

macro_rules! tedge_config_set {
    ($key:literal $value:ident $config_dir:ident) => {{
        use crate::*;
        let _cmd = tedge_command([
            "--config-dir",
            $config_dir,
            "config",
            "set",
            $key,
            $value, //$value.to_str().unwrap(),
        ])
        .unwrap()
        .output();
    }};
}

struct TedgeConfigCredentials {
    pub c8y_url: String,
    pub device_id: String,
    pub c8y_user: String,
}

impl Default for TedgeConfigCredentials {
    fn default() -> Self {
        Self {
            c8y_url: "initard.basic.stage.c8y.io".into(),
            device_id: "testuser".into(),
            c8y_user: "initard-test".into(),
        }
    }
}

fn tedge_cert_create(directory_path: &str) {
    let path: &str = "tedge";

    let c8y_url = "initard.basic.stage.c8y.io";
    let () = tedge_config_set!("c8y.url" c8y_url directory_path);
    // $ sudo tedge config set c8y.root.cert.path /etc/ssl/certs
    let cert = "/etc/ssl/certs";
    let () = tedge_config_set!("c8y.root.cert.path" cert directory_path);

    let mut cmd = Command::cargo_bin(path).unwrap();
    //cmd.env("C8YPASS", "CsiqG9s25z//");
    //cmd.env("C8YUSERNAME", "octocat");
    cmd.args([
        "--config-dir",
        directory_path,
        "cert",
        "create",
        "--device-id",
        "testuser",
    ]);

    let _ = cmd.spawn();

    let ten_millis = time::Duration::from_secs(1);
    thread::sleep(ten_millis);

    let mut cmd = Command::cargo_bin(path).unwrap();
    //cmd.env("C8YPASS", "CsiqG9s25z//");
    //cmd.env("C8YUSERNAME", "octocat");
    cmd.args(["--config-dir", directory_path, "config", "list", "--all"]);

    let _ = cmd.spawn();

    let mut cmd = Command::cargo_bin(path).unwrap();
    //cmd.env("C8YPASS", "CsiqG9s25z//");
    //cmd.env("C8YUSERNAME", "octocat");
    cmd.args([
        "--config-dir",
        directory_path,
        "cert",
        "upload",
        "c8y",
        "--user",
        "initard-test",
    ]);
    dbg!("here");
    let _ = cmd.spawn();
}

macro_rules! create_tedge_certificate {
    () => {{
        let tmpdir = tempfile::TempDir::new().unwrap();
        let dir_path = tmpdir.path().join("device-certs");
        println!("{:?}", tmpdir);

        std::fs::create_dir(dir_path).unwrap();

        let dir_path = tmpdir.path().join("sm-plugins");
        std::fs::create_dir(dir_path).unwrap();

        let directory_path = tmpdir.into_path();
        let directory_path = directory_path.to_str().unwrap();
        String::from(directory_path)
    }};
}

macro_rules! spawn_tedge_mapper {
    ($directory_path:ident) => {{
        let path: &str = "tedge_mapper";
        let mut cmd = Command::cargo_bin(path).unwrap();
        cmd.args(["--config-dir", $directory_path, "c8y"]);
        cmd.spawn().unwrap()
    }};
}

macro_rules! spawn_tedge_agent {
    ($directory_path:ident) => {{
        let path: &str = "tedge_agent";
        let mut cmd = Command::cargo_bin(path).unwrap();
        cmd.args(["--config-dir", $directory_path]);
        cmd.spawn().unwrap()
    }};
}

#[cfg(test)]
mod tests {
    use assert_cmd::prelude::*;

    use std::{path::PathBuf, process::Command};

    use crate::{tedge_cert_create, tedge_command, TedgeConfigCredentials};

    #[test]
    fn cmd_command() {
        use std::{thread, time};

        let credentials = TedgeConfigCredentials::default();
        let c8y_url = credentials.c8y_url.as_str();

        let directory_path = create_tedge_certificate!();
        let directory_path = &directory_path;
        let () = tedge_config_set!("c8y.url" c8y_url directory_path);
        let () = tedge_cert_create(&directory_path);

        let log_path = PathBuf::from(&directory_path).join("logs");
        let () = std::fs::create_dir(&log_path).unwrap();
        let log_path = log_path.to_str().unwrap();

        let () = tedge_config_set!("logs.path" log_path directory_path);

        thread::sleep(time::Duration::from_secs(1));

        let mut tedge_mapper_child = spawn_tedge_mapper!(directory_path);
        thread::sleep(time::Duration::from_secs(2));

        let mut tedge_agent_child = spawn_tedge_agent!(directory_path);
        thread::sleep(time::Duration::from_secs(2));

        let _cmd = tedge_command([
            "--config-dir",
            directory_path,
            "mqtt",
            "pub",
            "-t",
            "tedge/commands/req/software/list",
            "-m",
            r#"{"id":"r17FTqVxrKTeG6zDZpU4w"}"#,
        ])
        .unwrap()
        .output();

        thread::sleep(time::Duration::from_secs(10));

        let () = tedge_agent_child.kill().unwrap();
        let () = tedge_mapper_child.kill().unwrap();
    }

    #[test]
    fn it_works() {
        let tmpdir = tempfile::TempDir::new().unwrap();
        let directory_path = tmpdir.into_path();
        let directory_path = directory_path.to_str().unwrap();

        let device_id = tedge_config_get!("logs.path" directory_path);
        println!("{:?}", device_id);
    }

    //#[test]
    //fn tedge_config_set_test() {
    //    let tmpdir = tempfile::TempDir::new().unwrap();
    //    let directory_path = tmpdir.into_path();
    //    let directory_path = directory_path.to_str().unwrap();

    //    let log_path = PathBuf::from("/tmp");

    //    let () = tedge_config_set!("logs.path" log_path directory_path);
    //    let asset_path = tedge_config_get!("logs.path" directory_path);
    //}

    /// test:
    /// spawn_tedge_agent!(--config-dir /some/tempfile::Dir)
    /// spawn_tedge_mapper!(--config-dir /some/tempfile::Dir) -- no macro needed
    ///
    /// tedge!(config set device.id hello) -> String
    /// tedge_config_set!("device.id" "hi","/path/to/config") -> String
    ///
    ///
    /// tedge_command(["config", "set", "", "", "", ""])
    /// tedge_command(["config", "list", "all"])
    ///
    ///
    /// $ sudo tedge config set logs.path /some/var/log/path --config-dir /some/tempfile::Dir
    /// & sudo tedge_agent --config-dir /some/tempfile::Dir
    /// & sudo tedge_mapper --config-dir /some/tempfile::Dir
    /// check if software list is in /some/var/log/path

    #[test]
    fn config_list_all() {
        let tmpdir = tempfile::TempDir::new().unwrap();
        let directory_path = tmpdir.into_path();
        let directory_path = directory_path.to_str().unwrap();
        let mut cmd = tedge!(config list all --config-dir directory_path);
        cmd.assert().success();
    }

    #[test]
    fn tmp() {
        let mut cmd = tedge!(config set "logs.path", "/tmp" );
    }

    #[test]
    fn config_get() {
        let mut cmd = tedge!(config get "device.id");
        cmd.assert()
            .success()
            .stdout(predicates::str::contains("user"));
    }

    #[ignore]
    #[test]
    fn config_set() {
        let mut cmd = tedge!(config set "tmp.path", "/tmp");
        cmd.assert().success();
    }
}
