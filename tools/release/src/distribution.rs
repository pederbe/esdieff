//! Package retained release archives and prepare their distribution metadata.
use crate::Result;
use sha2::{Digest, Sha256, Sha512};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const URL: &str = "https://packages.esdieff.com";
const RELEASES: &str = "https://github.com/pederbe/esdieff/releases/download";
const DOCS: &[&str] = &[
    "README.md",
    "CHANGELOG.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "THIRD-PARTY-LICENSES.md",
];
const TARGETS: &[(&str, &str, &str)] = &[
    ("x86_64-unknown-linux-gnu", "amd64", "x86_64"),
    ("aarch64-unknown-linux-gnu", "arm64", "aarch64"),
];

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("package-linux") => package_linux(&args[1..]),
        Some("repository") => repository(&args[1..]),
        Some("homebrew") => homebrew(&args[1..]),
        Some("winget") => winget(&args[1..]),
        _ => Err("expected package-linux, repository, homebrew or winget".into()),
    }
}

fn stable(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

fn output(command: &mut Command) -> Result<Vec<u8>> {
    let result = command.output()?;
    if !result.status.success() {
        return Err(format!(
            "{:?} failed with {}: {}",
            command.get_program(),
            result.status,
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(result.stdout)
}

fn text(command: &mut Command) -> Result<String> {
    Ok(String::from_utf8(output(command)?)?)
}

fn hash(path: &Path, algorithm: &str) -> Result<String> {
    let bytes = fs::read(path)?;
    let digest: Vec<u8> = match algorithm {
        "sha256" => Sha256::digest(&bytes).to_vec(),
        "sha512" => Sha512::digest(&bytes).to_vec(),
        _ => return Err("unsupported file digest".into()),
    };
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

struct Work(PathBuf);
static NEXT_WORK: AtomicU64 = AtomicU64::new(0);
impl Work {
    fn new(parent: &Path) -> Result<Self> {
        fs::create_dir_all(parent)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sequence = NEXT_WORK.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".esdieff-work-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn keep(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        if fs::read(from)? != fs::read(to)? {
            return Err(format!("{} differs from the retained file", to.display()).into());
        }
    } else {
        fs::copy(from, to)?;
    }
    Ok(())
}

fn compare_package(extracted: &Path, packaged: &Path) -> Result<()> {
    for (from, to) in std::iter::once((extracted.join("esdieff"), packaged.join("usr/bin/esdieff")))
        .chain(DOCS.iter().map(|name| {
            (
                extracted.join(name),
                packaged.join("usr/share/doc/esdieff").join(name),
            )
        }))
    {
        if fs::read(&from)? != fs::read(&to)? {
            return Err(format!("packaging changed {}", from.display()).into());
        }
    }
    Ok(())
}

fn extract_archive(archive: &Path, prefix: &str, extracted: &Path) -> Result<()> {
    fs::create_dir(extracted)?;
    let listing = text(Command::new("tar").arg("-tf").arg(archive))?;
    let mut entries = BTreeSet::new();
    for name in listing.lines() {
        let name = name.strip_prefix("./").unwrap_or(name);
        if name.trim_end_matches('/') == prefix {
            continue;
        }
        let relative = name
            .strip_prefix(&format!("{prefix}/"))
            .ok_or("unexpected archive root")?;
        if !(relative == "esdieff" || DOCS.contains(&relative)) || !entries.insert(relative) {
            return Err(format!("unexpected or repeated archive entry: {name}").into());
        }
    }
    output(
        Command::new("tar")
            .args(["-xf"])
            .arg(archive)
            .args(["--strip-components=1", "--no-same-owner", "-C"])
            .arg(extracted),
    )?;
    for name in std::iter::once("esdieff").chain(DOCS.iter().copied()) {
        if !fs::symlink_metadata(extracted.join(name))?.is_file() {
            return Err(format!("archive has no regular {name}").into());
        }
    }
    Ok(())
}

fn package_linux(args: &[String]) -> Result<()> {
    let [archive, target, version, out] = args else {
        return Err("usage: esdieff-release package-linux ARCHIVE TARGET VERSION OUT".into());
    };
    if !stable(version) {
        return Err("expected a stable x.y.z version".into());
    }
    let (_, deb_arch, rpm_arch) = TARGETS
        .iter()
        .find(|(name, _, _)| *name == target)
        .ok_or("unsupported Linux package target")?;
    let archive = fs::canonicalize(archive)?;
    let prefix = format!("esdieff-{target}");
    if archive.file_name().and_then(|n| n.to_str()) != Some(&format!("{prefix}.tar.xz")) {
        return Err("archive name does not match its target".into());
    }
    fs::create_dir_all(out)?;
    let out = fs::canonicalize(out)?;
    let work = Work::new(&out)?;
    let extracted = work.0.join("archive");
    extract_archive(&archive, &prefix, &extracted)?;
    let deb_root = work.0.join("deb");
    fs::create_dir_all(deb_root.join("usr/bin"))?;
    fs::create_dir_all(deb_root.join("usr/share/doc/esdieff"))?;
    fs::create_dir(deb_root.join("DEBIAN"))?;
    fs::copy(extracted.join("esdieff"), deb_root.join("usr/bin/esdieff"))?;
    for name in DOCS {
        fs::copy(
            extracted.join(name),
            deb_root.join("usr/share/doc/esdieff").join(name),
        )?;
    }
    fs::write(
        deb_root.join("DEBIAN/control"),
        format!(
            "Package: esdieff\nVersion: {version}\nArchitecture: {deb_arch}\nMaintainer: Peder Bergan <16326438+pederbe@users.noreply.github.com>\nDepends: libc6 (>= 2.35), libgcc-s1\nSection: devel\nPriority: optional\nHomepage: https://esdieff.com\nDescription: An SDF toolkit for connected-device models\n Inspect, check, compare and test connected-device models.\n"
        ),
    )?;
    let deb = out.join(format!("esdieff_{version}_{deb_arch}.deb"));
    output(
        Command::new("dpkg-deb")
            .args(["--root-owner-group", "-Zxz", "--build"])
            .arg(&deb_root)
            .arg(&deb),
    )?;
    let deb_check = work.0.join("deb-check");
    output(Command::new("dpkg-deb").arg("-x").arg(&deb).arg(&deb_check))?;
    compare_package(&extracted, &deb_check)?;

    let rpm_root = work.0.join("rpm");
    for directory in ["BUILD", "RPMS", "SOURCES", "SPECS", "SRPMS"] {
        fs::create_dir_all(rpm_root.join(directory))?;
    }
    output(
        Command::new("cp")
            .arg("-a")
            .arg(deb_root.join("usr"))
            .arg(rpm_root.join("SOURCES/usr")),
    )?;
    let spec = rpm_root.join("SPECS/esdieff.spec");
    fs::write(
        &spec,
        format!(
            "%global debug_package %{{nil}}\n%global __os_install_post %{{nil}}\n%global _build_id_links none\nName: esdieff\nVersion: {version}\nRelease: 1\nSummary: An SDF toolkit for connected-device models\nLicense: MIT OR Apache-2.0\nURL: https://esdieff.com\nAutoReqProv: no\nRequires: glibc >= 2.35\nRequires: libgcc\n%description\nInspect, check, compare and test connected-device models.\n%install\nmkdir -p %{{buildroot}}/usr\ncp -a %{{_sourcedir}}/usr/. %{{buildroot}}/usr/\n%files\n/usr/bin/esdieff\n/usr/share/doc/esdieff\n"
        ),
    )?;
    output(
        Command::new("rpmbuild")
            .args(["--quiet", "-bb", "--target", rpm_arch, "--define"])
            .arg(format!("_topdir {}", rpm_root.display()))
            .arg(&spec),
    )?;
    let rpm_name = format!("esdieff-{version}-1.{rpm_arch}.rpm");
    let rpm = out.join(&rpm_name);
    fs::copy(rpm_root.join("RPMS").join(rpm_arch).join(&rpm_name), &rpm)?;
    let rpm_check = work.0.join("rpm-check");
    fs::create_dir(&rpm_check)?;
    let mut producer = Command::new("rpm2cpio")
        .arg(&rpm)
        .stdout(Stdio::piped())
        .spawn()?;
    let consumer = Command::new("cpio")
        .args(["-idm", "--quiet"])
        .current_dir(&rpm_check)
        .stdin(producer.stdout.take().ok_or("rpm2cpio has no output")?)
        .status()?;
    let producer_status = producer.wait()?;
    if !consumer.success() || !producer_status.success() {
        return Err("RPM extraction failed".into());
    }
    compare_package(&extracted, &rpm_check)?;
    for path in [&deb, &rpm] {
        let name = path
            .file_name()
            .ok_or("package has no name")?
            .to_string_lossy();
        fs::write(
            path.with_file_name(format!("{name}.sha256")),
            format!("{}  {name}\n", hash(path, "sha256")?),
        )?;
        println!("{}", path.display());
    }
    Ok(())
}

struct Signer {
    fingerprint: String,
    options: Vec<String>,
}
impl Signer {
    fn new(public: &Path, work: &Path) -> Result<Self> {
        let listing = text(
            Command::new("gpg")
                .args(["--batch", "--show-keys", "--with-colons"])
                .arg(public),
        )?;
        let mut primary = false;
        let mut found = Vec::new();
        for line in listing.lines() {
            let fields: Vec<_> = line.split(':').collect();
            match fields[0] {
                "pub" => primary = true,
                "fpr" if primary => {
                    found.push(fields.get(9).copied().unwrap_or_default().to_owned());
                    primary = false;
                }
                "sub" => primary = false,
                _ => {}
            }
        }
        let [fingerprint] = found.as_slice() else {
            return Err("public key must hold one primary key".into());
        };
        output(Command::new("gpg").args(["--batch", "--list-secret-keys", fingerprint]))?;
        let mut options = vec![
            "--batch".into(),
            "--pinentry-mode".into(),
            "loopback".into(),
        ];
        let path = work.join("passphrase");
        let mut open = fs::OpenOptions::new();
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
        open.create_new(true).write(true).open(&path)?.write_all(
            std::env::var("PACKAGE_SIGNING_PASSPHRASE")
                .unwrap_or_default()
                .as_bytes(),
        )?;
        options.extend(["--passphrase-file".into(), path.display().to_string()]);
        Ok(Self {
            fingerprint: fingerprint.clone(),
            options,
        })
    }
    fn sign(&self, input: &Path, signature: &Path, clear: bool) -> Result<()> {
        output(
            Command::new("gpg")
                .args(&self.options)
                .args([
                    "--yes",
                    "--local-user",
                    &self.fingerprint,
                    "--digest-algo",
                    "SHA512",
                    "--output",
                ])
                .arg(signature)
                .args(if clear {
                    vec!["--clearsign"]
                } else {
                    vec!["--armor", "--detach-sign"]
                })
                .arg(input),
        )?;
        Ok(())
    }
}

fn package_identity(name: &str) -> Option<(&str, &str, bool)> {
    for (_, deb_arch, rpm_arch) in TARGETS {
        if let Some(version) = name
            .strip_prefix("esdieff_")
            .and_then(|s| s.strip_suffix(&format!("_{deb_arch}.deb")))
        {
            return stable(version).then_some((version, deb_arch, false));
        }
        if let Some(version) = name
            .strip_prefix("esdieff-")
            .and_then(|s| s.strip_suffix(&format!("-1.{rpm_arch}.rpm")))
        {
            return stable(version).then_some((version, rpm_arch, true));
        }
    }
    None
}

fn rpm_identity(path: &Path) -> Result<String> {
    let tags = text(Command::new("rpm").arg("--querytags"))?;
    let payload = if tags.lines().any(|line| line == "PAYLOADSHA256") {
        "PAYLOADSHA256"
    } else {
        "PAYLOADDIGEST"
    };
    text(
        Command::new("rpm")
            .args(["-qp", "--nosignature", "--qf"])
            .arg(format!("%{{SHA256HEADER}} %{{{payload}}}"))
            .arg(path),
    )
}

fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            result.extend(files(&entry.path())?);
        } else if kind.is_file() {
            result.push(entry.path());
        } else {
            return Err(format!(
                "unexpected non-regular repository entry: {}",
                entry.path().display()
            )
            .into());
        }
    }
    result.sort();
    Ok(result)
}

fn repository(args: &[String]) -> Result<()> {
    let [root, public, packages @ ..] = args else {
        return Err("usage: esdieff-release repository ROOT PUBLIC_KEY PACKAGE...".into());
    };
    if packages.is_empty() {
        return Err("expected at least one release package".into());
    }
    fs::create_dir_all(root)?;
    let root = fs::canonicalize(root)?;
    let public = fs::canonicalize(public)?;
    let work = Work::new(root.parent().ok_or("repository has no parent")?)?;
    let signer = Signer::new(&public, &work.0)?;
    let rpmdb = work.0.join("rpmdb");
    fs::create_dir(&rpmdb)?;
    output(
        Command::new("rpmkeys")
            .arg("--dbpath")
            .arg(&rpmdb)
            .arg("--import")
            .arg(&public),
    )?;
    fs::create_dir_all(root.join("apt/pool/main/e/esdieff"))?;
    fs::create_dir_all(root.join("rpm"))?;
    let rpm_version = text(Command::new("rpm").arg("--version"))?;
    let rpm6 = rpm_version
        .split_whitespace()
        .last()
        .and_then(|v| v.split('.').next())
        .and_then(|v| v.parse::<u32>().ok())
        .is_some_and(|v| v >= 6);
    for path in packages {
        let path = Path::new(path);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("package has no name")?;
        let (_, _, is_rpm) = package_identity(name).ok_or("expected an Esdieff .deb or .rpm")?;
        if is_rpm {
            let copy = work.0.join(name);
            fs::copy(path, &copy)?;
            let identity = rpm_identity(&copy)?;
            let mut command = Command::new("rpmsign");
            command.arg("--addsign");
            if rpm6 {
                command.arg("--rpmv4");
            }
            output(
                command
                    .arg("--define")
                    .arg(format!("_gpg_name {}", signer.fingerprint))
                    .arg("--define")
                    .arg(format!(
                        "_gpg_sign_cmd_extra_args {}",
                        signer.options.join(" ")
                    ))
                    .arg(&copy),
            )?;
            if rpm_identity(&copy)? != identity {
                return Err("RPM signing changed its header or payload".into());
            }
            let destination = root.join("rpm").join(name);
            if destination.exists() {
                if rpm_identity(&destination)? != identity {
                    return Err(format!("{name} differs from its retained package").into());
                }
            } else {
                fs::copy(copy, destination)?;
            }
        } else {
            keep(path, &root.join("apt/pool/main/e/esdieff").join(name))?;
        }
    }
    let mut revision = 0;
    for path in files(&root)? {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let Some((version, arch, is_rpm)) = package_identity(name) else {
            if path
                .extension()
                .is_some_and(|extension| extension == "deb" || extension == "rpm")
            {
                return Err(format!("unexpected package: {}", path.display()).into());
            }
            continue;
        };
        let expected = if is_rpm {
            root.join("rpm").join(name)
        } else {
            root.join("apt/pool/main/e/esdieff").join(name)
        };
        if path != expected {
            return Err(format!("unexpected package location: {}", path.display()).into());
        }
        let actual = if is_rpm {
            let signature = text(
                Command::new("rpmkeys")
                    .arg("--dbpath")
                    .arg(&rpmdb)
                    .arg("--checksig")
                    .arg(&path),
            )?;
            if !signature.trim_end().ends_with(": digests signatures OK") {
                return Err("RPM signature is not trusted".into());
            }
            let built = text(
                Command::new("rpm")
                    .args(["-qp", "--nosignature", "--qf", "%{BUILDTIME}"])
                    .arg(&path),
            )?
            .parse::<u64>()?;
            revision = revision.max(built);
            fs::File::open(&path)?.set_times(
                fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(built)),
            )?;
            text(
                Command::new("rpm")
                    .args(["-qp", "--nosignature", "--qf", "%{NAME} %{VERSION} %{ARCH}"])
                    .arg(&path),
            )?
        } else {
            let name = text(Command::new("dpkg-deb").arg("-f").arg(&path).arg("Package"))?;
            let version = text(Command::new("dpkg-deb").arg("-f").arg(&path).arg("Version"))?;
            let arch = text(
                Command::new("dpkg-deb")
                    .arg("-f")
                    .arg(&path)
                    .arg("Architecture"),
            )?;
            format!("{} {} {}", name.trim(), version.trim(), arch.trim())
        };
        if actual != format!("esdieff {version} {arch}") {
            return Err(format!("{} does not match its package metadata", path.display()).into());
        }
    }
    let apt = root.join("apt");
    let dists = apt.join("dists/stable");
    for (_, arch, _) in TARGETS {
        let index = dists.join(format!("main/binary-{arch}"));
        fs::create_dir_all(&index)?;
        let packages = output(
            Command::new("apt-ftparchive")
                .args(["--arch", arch, "packages", "pool"])
                .current_dir(&apt),
        )?;
        fs::write(index.join("Packages"), packages)?;
        let compressed = output(
            Command::new("gzip")
                .args(["-nc", "--"])
                .arg(index.join("Packages")),
        )?;
        fs::write(index.join("Packages.gz"), compressed)?;
        for (algorithm, directory) in [("sha256", "SHA256"), ("sha512", "SHA512")] {
            let by_hash = index.join("by-hash").join(directory);
            fs::create_dir_all(&by_hash)?;
            for name in ["Packages", "Packages.gz"] {
                let source = index.join(name);
                keep(&source, &by_hash.join(hash(&source, algorithm)?))?;
            }
        }
    }
    for name in ["Release", "Release.gpg", "InRelease"] {
        if dists.join(name).exists() {
            fs::remove_file(dists.join(name))?;
        }
    }
    let mut command = Command::new("apt-ftparchive");
    for (field, value) in [
        ("Origin", "Esdieff"),
        ("Label", "Esdieff"),
        ("Suite", "stable"),
        ("Codename", "stable"),
        ("Architectures", "amd64 arm64"),
        ("Components", "main"),
        ("Acquire-By-Hash", "yes"),
    ] {
        command
            .arg("-o")
            .arg(format!("APT::FTPArchive::Release::{field}={value}"));
    }
    fs::write(
        dists.join("Release"),
        output(command.arg("release").arg(&dists))?,
    )?;
    signer.sign(&dists.join("Release"), &dists.join("InRelease"), true)?;
    signer.sign(&dists.join("Release"), &dists.join("Release.gpg"), false)?;
    let generated = work.0.join("rpm");
    fs::create_dir(&generated)?;
    output(
        Command::new("createrepo_c")
            .args(["--quiet", "--set-timestamp-to-revision", "--revision"])
            .arg(revision.to_string())
            .args(["--location-prefix", "../../", "--outputdir"])
            .arg(&generated)
            .arg(root.join("rpm")),
    )?;
    let metadata = generated.join("repodata/repomd.xml");
    signer.sign(&metadata, &metadata.with_extension("xml.asc"), false)?;
    let generation = hash(&metadata, "sha256")?;
    let snapshot = root
        .join("rpm/snapshots")
        .join(&generation)
        .join("repodata");
    fs::create_dir_all(&snapshot)?;
    for path in files(&generated.join("repodata"))? {
        let destination = snapshot.join(path.file_name().ok_or("metadata has no name")?);
        if destination.exists() && destination.extension().is_some_and(|e| e == "asc") {
            continue;
        }
        keep(&path, &destination)?;
    }
    fs::write(
        root.join("rpm/mirrorlist"),
        format!("{URL}/rpm/snapshots/{generation}/\n"),
    )?;
    fs::copy(&public, root.join("esdieff.asc"))?;
    let keyring = root.join("esdieff.gpg");
    output(
        Command::new("gpg")
            .args(["--batch", "--yes", "--dearmor", "--output"])
            .arg(&keyring)
            .arg(&public),
    )?;
    fs::write(
        root.join("esdieff.repo"),
        format!(
            "[esdieff]\nname=Esdieff\nmirrorlist={URL}/rpm/mirrorlist\nenabled=1\ngpgcheck=1\nrepo_gpgcheck=1\ngpgkey={URL}/esdieff.asc\n"
        ),
    )?;
    for pair in [
        vec![dists.join("InRelease")],
        vec![dists.join("Release.gpg"), dists.join("Release")],
        vec![snapshot.join("repomd.xml.asc"), snapshot.join("repomd.xml")],
    ] {
        output(
            Command::new("gpgv")
                .arg("--keyring")
                .arg(&keyring)
                .args(pair),
        )?;
    }
    println!("{}", signer.fingerprint);
    Ok(())
}

fn homebrew(args: &[String]) -> Result<()> {
    let [directory, version] = args else {
        return Err("usage: esdieff-release homebrew DISTDIR VERSION".into());
    };
    if !stable(version) {
        return Err("expected a stable x.y.z version".into());
    }
    let directory = Path::new(directory);
    let mut hashes = Vec::new();
    for target in [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ] {
        hashes.push(hash(
            &directory.join(format!("esdieff-{target}.tar.xz")),
            "sha256",
        )?);
    }
    fs::write(directory.join("esdieff.rb"), formula(version, &hashes))?;
    Ok(())
}

fn formula(version: &str, hashes: &[String]) -> String {
    format!(
        "class Esdieff < Formula\n  desc \"An SDF toolkit for connected-device models\"\n  homepage \"https://esdieff.com\"\n  version \"{version}\"\n  license any_of: [\"MIT\", \"Apache-2.0\"]\n\n  on_macos do\n    depends_on macos: :sequoia\n    depends_on arch: :arm64\n    url \"{RELEASES}/v{version}/esdieff-aarch64-apple-darwin.tar.xz\"\n    sha256 \"{}\"\n  end\n\n  on_linux do\n    on_intel do\n      url \"{RELEASES}/v{version}/esdieff-x86_64-unknown-linux-gnu.tar.xz\"\n      sha256 \"{}\"\n    end\n    on_arm do\n      url \"{RELEASES}/v{version}/esdieff-aarch64-unknown-linux-gnu.tar.xz\"\n      sha256 \"{}\"\n    end\n  end\n\n  def install\n    bin.install \"esdieff\"\n    doc.install \"README.md\", \"CHANGELOG.md\", \"LICENSE-MIT\", \"LICENSE-APACHE\", \"THIRD-PARTY-LICENSES.md\"\n  end\n\n  test do\n    assert_equal \"esdieff #{{version}}\\n\", shell_output(\"#{{bin}}/esdieff --version\")\n    (testpath/\"model.sdf.json\").write '{{\"sdfObject\":{{\"Thermostat\":{{\"sdfProperty\":{{\"temperature\":{{\"type\":\"number\"}}}}}}}}}}'\n    system bin/\"esdieff\", \"check\", \"model.sdf.json\"\n  end\nend\n",
        hashes[0], hashes[1], hashes[2]
    )
}

fn winget(args: &[String]) -> Result<()> {
    let [directory, version] = args else {
        return Err("usage: esdieff-release winget DISTDIR VERSION".into());
    };
    if !stable(version) {
        return Err("expected a stable x.y.z version".into());
    }
    let directory = Path::new(directory);
    let archive = "esdieff-x86_64-pc-windows-msvc.zip";
    let checksum = hash(&directory.join(archive), "sha256")?;
    let out = directory
        .join("winget/manifests/p/PederBergan/Esdieff")
        .join(version);
    fs::create_dir_all(&out)?;
    // The community repository currently accepts the 1.12 manifest schema.
    let schema = "1.12.0";
    let identity = format!("PackageIdentifier: PederBergan.Esdieff\nPackageVersion: {version}\n");
    let header = |kind| {
        format!(
            "# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.{schema}.schema.json\n\n"
        )
    };
    fs::write(
        out.join("PederBergan.Esdieff.yaml"),
        format!(
            "{}{identity}DefaultLocale: en-US\nManifestType: version\nManifestVersion: {schema}\n",
            header("version")
        ),
    )?;
    fs::write(
        out.join("PederBergan.Esdieff.installer.yaml"),
        format!(
            "{}{identity}InstallerType: zip\nNestedInstallerType: portable\nNestedInstallerFiles:\n- RelativeFilePath: esdieff.exe\n  PortableCommandAlias: esdieff\nMinimumOSVersion: 10.0.22000.0\nInstallers:\n- Architecture: x64\n  InstallerUrl: {RELEASES}/v{version}/{archive}\n  InstallerSha256: {}\nManifestType: installer\nManifestVersion: {schema}\n",
            header("installer"),
            checksum.to_ascii_uppercase()
        ),
    )?;
    fs::write(
        out.join("PederBergan.Esdieff.locale.en-US.yaml"),
        format!(
            "{}{identity}PackageLocale: en-US\nPublisher: Peder Bergan\nPublisherUrl: https://github.com/pederbe\nPublisherSupportUrl: https://github.com/pederbe/esdieff/issues\nPackageName: Esdieff\nPackageUrl: https://esdieff.com\nLicense: MIT OR Apache-2.0\nLicenseUrl: https://github.com/pederbe/esdieff/blob/v{version}/README.md#license\nShortDescription: An SDF toolkit for connected-device models\nMoniker: esdieff\nReleaseNotesUrl: https://github.com/pederbe/esdieff/releases/tag/v{version}\nManifestType: defaultLocale\nManifestVersion: {schema}\n",
            header("defaultLocale")
        ),
    )?;
    println!("{}", out.display());
    Ok(())
}

#[cfg(test)]
#[path = "distribution_checks.rs"]
mod checks;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_hashes_match_standard_digest_vectors() {
        let work = Work::new(&std::env::temp_dir()).unwrap();
        let file = work.0.join("abc");
        fs::write(&file, b"abc").unwrap();
        assert_eq!(
            hash(&file, "sha256").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hash(&file, "sha512").unwrap(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }

    #[test]
    fn package_names_must_match_a_supported_target_and_stable_version() {
        assert_eq!(
            package_identity("esdieff_0.1.0_amd64.deb"),
            Some(("0.1.0", "amd64", false))
        );
        assert_eq!(
            package_identity("esdieff-1.2.3-1.aarch64.rpm"),
            Some(("1.2.3", "aarch64", true))
        );
        for name in [
            "esdieff_0.1_amd64.deb",
            "esdieff_01.0.0_amd64.deb",
            "esdieff_0.1.0_i386.deb",
            "other_0.1.0_amd64.deb",
            "esdieff-0.1.0-2.x86_64.rpm",
        ] {
            assert!(package_identity(name).is_none(), "{name}");
        }
    }

    #[test]
    fn formula_uses_the_retained_archives_and_macos_minimum() {
        let formula = formula("0.1.0", &["a".repeat(64), "b".repeat(64), "c".repeat(64)]);
        assert!(formula.contains("depends_on macos: :sequoia"));
        assert!(formula.contains("depends_on arch: :arm64"));
        assert!(!formula.contains("x86_64-apple-darwin"));
        assert_eq!(formula.matches("sha256").count(), 3);
        assert!(formula.contains("system bin/\"esdieff\", \"check\", \"model.sdf.json\""));
    }

    #[test]
    fn winget_uses_community_schema_headers_and_the_actual_archive_digest() {
        let work = Work::new(&std::env::temp_dir()).unwrap();
        fs::write(work.0.join("esdieff-x86_64-pc-windows-msvc.zip"), b"abc").unwrap();
        winget(&[work.0.to_str().unwrap().to_owned(), "0.1.0".to_owned()]).unwrap();
        let out = work.0.join("winget/manifests/p/PederBergan/Esdieff/0.1.0");
        for (name, kind) in [
            ("PederBergan.Esdieff.yaml", "version"),
            ("PederBergan.Esdieff.installer.yaml", "installer"),
            ("PederBergan.Esdieff.locale.en-US.yaml", "defaultLocale"),
        ] {
            let text = fs::read_to_string(out.join(name)).unwrap();
            assert_eq!(
                text.lines().next().unwrap(),
                format!(
                    "# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.1.12.0.schema.json"
                )
            );
            assert!(text.contains(&format!("ManifestType: {kind}\nManifestVersion: 1.12.0\n")));
            if kind == "installer" {
                assert!(text.contains("InstallerSha256: BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD\n"));
                assert!(text.contains("NestedInstallerType: portable\n"));
                assert!(text.contains("RelativeFilePath: esdieff.exe\n"));
                assert!(text.contains("PortableCommandAlias: esdieff\n"));
            }
        }
    }
}
