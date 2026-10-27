#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::fs::symlink,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Installer(PathBuf);

impl Installer {
    fn new(manager: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "esdieff-installer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let this = Self(root);
        fs::create_dir(this.0.join("bin")).unwrap();
        let compile = Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "-Dunsafe_code"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/installer-command.rs"
            ))
            .arg("-o")
            .arg(this.0.join("command"))
            .output()
            .unwrap();
        assert!(compile.status.success(), "{compile:?}");
        for name in ["id", "uname", "curl", "sudo", "install", "rpm", manager] {
            symlink(this.0.join("command"), this.0.join("bin").join(name)).unwrap();
        }
        for name in ["cat", "mktemp", "rm", "sha256sum"] {
            symlink(format!("/usr/bin/{name}"), this.0.join("bin").join(name)).unwrap();
        }
        this
    }

    fn run(&self, uid: &str, failure: Option<&str>) -> Output {
        let mut command = Command::new("/bin/sh");
        command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/install.sh"
            ))
            .env("PATH", self.0.join("bin"))
            .env("INSTALL_TEST_ROOT", &self.0)
            .env("INSTALL_TEST_UID", uid)
            .env("TMPDIR", &self.0);
        if let Some(failure) = failure {
            command.env(failure, "1");
        }
        command.output().unwrap()
    }

    fn commands(&self) -> String {
        fs::read_to_string(self.0.join("commands")).unwrap_or_default()
    }

    fn installed(&self, path: &str) -> String {
        fs::read_to_string(self.0.join("installed").join(path)).unwrap()
    }
}

impl Drop for Installer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn apt_setup_verifies_the_key_and_installs_through_sudo() {
    let installer = Installer::new("apt-get");
    let output = installer.run("1000", None);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        installer.installed("usr/share/keyrings/esdieff.asc"),
        include_str!("../../../packaging/esdieff.asc")
    );
    assert_eq!(
        installer.installed("etc/apt/sources.list.d/esdieff.list"),
        "deb [signed-by=/usr/share/keyrings/esdieff.asc] https://packages.esdieff.com/apt stable main\n"
    );
    let commands = installer.commands();
    assert!(commands.contains("sudo apt-get update\n"));
    assert!(commands.contains("sudo apt-get install -y esdieff\n"));
    assert!(commands.find("apt-get update").unwrap() < commands.find("apt-get install").unwrap());
}

#[test]
fn fedora_imports_the_verified_key_and_keeps_signature_checks_enabled() {
    let installer = Installer::new("dnf");
    let output = installer.run("0", None);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        installer.installed("etc/pki/rpm-gpg/RPM-GPG-KEY-esdieff"),
        include_str!("../../../packaging/esdieff.asc")
    );
    let repository = installer.installed("etc/yum.repos.d/esdieff.repo");
    assert!(repository.contains("mirrorlist=https://packages.esdieff.com/rpm/mirrorlist\n"));
    assert!(repository.contains("gpgcheck=1\nrepo_gpgcheck=1\n"));
    assert!(repository.contains("gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-esdieff\n"));
    let commands = installer.commands();
    assert!(commands.contains("rpm --import /etc/pki/rpm-gpg/RPM-GPG-KEY-esdieff\n"));
    assert!(commands.contains("dnf install -y --refresh esdieff\n"));
    assert!(!commands.contains("sudo"));
}

#[test]
fn wrong_key_and_failed_download_make_no_system_changes() {
    for failure in ["INSTALL_TEST_WRONG_KEY", "INSTALL_TEST_DOWNLOAD_FAILURE"] {
        let installer = Installer::new("apt-get");
        let output = installer.run("1000", Some(failure));
        assert!(!output.status.success(), "{output:?}");
        assert_eq!(installer.commands(), "");
        assert!(!installer.0.join("installed").exists());
    }
}

#[test]
fn failed_repository_refresh_does_not_install_or_report_success() {
    let installer = Installer::new("apt-get");
    let output = installer.run("0", Some("INSTALL_TEST_UPDATE_FAILURE"));
    assert!(!output.status.success(), "{output:?}");
    assert!(!installer.commands().contains("apt-get install"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Esdieff is installed"));
}
