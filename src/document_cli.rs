//! Shared explicit document-output policies follow ADR 0017, Reference-preserving sdfThing
//! authoring, and ADR 0034, Authored-quality editing with source substitution.
use crate::cli_style::ColorMode;
use esdieff::{Assessment, Catalogue, HumanStyle, Outcome, SourceInput};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) type Error = (u8, String);
pub(super) fn usage(message: impl Into<String>) -> Error {
    (2, message.into())
}
fn host(message: impl Into<String>) -> Error {
    (4, message.into())
}

/// One explicit document operation, formed from a plan over the supplied catalogue.
pub(super) trait Operation: Sized {
    /// The subcommand name used in usage messages.
    const NAME: &'static str;
    /// Whether delivery refuses any protected input whose identity changed after validation.
    const UNCHANGED_INPUTS: bool;
    type Result: OperationResult;
    fn from_plan(catalogue: Catalogue, plan: &[u8]) -> Result<Self, &'static str>;
    fn strict_warnings(self, strict: bool) -> Self;
    fn checking(self, assessment: Assessment) -> Result<Self, &'static str>;
    fn perform(self) -> Result<Self::Result, &'static str>;
}
/// The generated document and report of a performed document operation.
pub(super) trait OperationResult {
    fn document_deliverable(&self) -> bool;
    fn generated_document(&self) -> Option<&[u8]>;
    fn to_json(&self) -> String;
    fn to_human_with_style(&self, style: HumanStyle) -> String;
    fn outcome(&self) -> &Outcome;
}
pub(super) fn run<O: Operation>(args: &[OsString], color: &mut ColorMode) -> Result<u8, Error> {
    let Some(options) = parse(args, color, O::NAME)? else {
        return crate::emit_help(*color);
    };
    let mut delivery = Delivery::prepare(&options)?;
    if O::UNCHANGED_INPUTS {
        delivery = delivery.require_unchanged_inputs();
    }
    let bytes = read_plan(options.plan, O::NAME)?;
    let catalogue = Catalogue::take_inputs(options.inputs()).map_err(usage)?;
    let mut request = O::from_plan(catalogue, &bytes)
        .map_err(usage)?
        .strict_warnings(options.strict);
    if let Some(check) = options.check {
        request = request.checking(check).map_err(usage)?;
    }
    let result = request.perform().map_err(usage)?;
    let document = result
        .document_deliverable()
        .then(|| result.generated_document().expect("deliverable document"));
    let rendered = if options.json {
        result.to_json()
    } else {
        result.to_human_with_style(delivery.report_style(*color))
    };
    delivery.emit(document, rendered)?;
    Ok(result.outcome().disposition().exit_code())
}
pub(super) struct Options<'a> {
    pub(super) plan: &'a OsStr,
    pub(super) document: &'a OsStr,
    pub(super) report: &'a OsStr,
    pub(super) sources: Vec<(&'a str, &'a OsStr)>,
    pub(super) json: bool,
    pub(super) check: Option<Assessment>,
    pub(super) strict: bool,
    pub(super) replace_document: bool,
    pub(super) replace_report: bool,
}
impl Options<'_> {
    pub(super) fn inputs(&self) -> Vec<SourceInput<'_>> {
        self.sources
            .iter()
            .map(|(key, path)| {
                if *path == OsStr::new("-") {
                    SourceInput::stdin(key)
                } else {
                    SourceInput::file(key, Path::new(path))
                }
            })
            .collect()
    }
}
pub(super) fn parse<'a>(
    args: &'a [OsString],
    color: &mut ColorMode,
    operation: &str,
) -> Result<Option<Options<'a>>, Error> {
    let mut plan = None;
    let mut document = None;
    let mut report = None;
    let mut sources = Vec::new();
    let mut json = false;
    let mut check = None;
    let mut strict = false;
    let mut replace_document = false;
    let mut replace_report = false;
    let mut index = 1;
    let mut metadata = 0usize;
    let mut help = false;
    while let Some(flag) = args.get(index) {
        index += 1;
        if color.consume(flag, args, &mut index)? {
            continue;
        }
        match flag.to_str() {
            Some("--help" | "-h") => help = true,
            Some("--strict-warnings") if !strict => strict = true,
            Some("--replace-document") if !replace_document => replace_document = true,
            Some("--replace-report") if !replace_report => replace_report = true,
            Some("--source") => {
                if sources.len() >= 255 {
                    return Err(usage(
                        "at most 255 supplied sources with one generated source",
                    ));
                }
                let key = args
                    .get(index)
                    .and_then(|v| v.to_str())
                    .ok_or_else(|| usage("--source requires a UTF-8 key and explicit path"))?;
                let path = args
                    .get(index + 1)
                    .ok_or_else(|| usage("--source requires key and path"))?;
                index += 2;
                if key.len() > 256 || sources.iter().any(|(previous, _)| *previous == key) {
                    return Err(usage("source keys must be bounded and unique"));
                }
                metadata = metadata
                    .checked_add(key.len() + path.as_encoded_bytes().len())
                    .ok_or_else(|| usage("input metadata overflow"))?;
                sources.push((key, path.as_os_str()));
            }
            Some(
                flag
                @ ("--plan" | "--document-output" | "--report-output" | "--check" | "--format"),
            ) => {
                let value = args
                    .get(index)
                    .ok_or_else(|| usage("option requires a value"))?;
                index += 1;
                metadata = metadata
                    .checked_add(value.as_encoded_bytes().len())
                    .ok_or_else(|| usage("input metadata overflow"))?;
                match flag {
                    "--plan" if plan.is_none() => plan = Some(value.as_os_str()),
                    "--document-output" if document.is_none() => document = Some(value.as_os_str()),
                    "--report-output" if report.is_none() => report = Some(value.as_os_str()),
                    "--check" if check.is_none() => {
                        check = Some(match value.to_str() {
                            Some("initial-v1") => Assessment::Initial,
                            Some("full-baseline") => Assessment::FullBaseline,
                            _ => {
                                return Err(usage(format!(
                                    "{operation} check must be initial-v1 or full-baseline"
                                )));
                            }
                        })
                    }
                    "--format" => {
                        json = match value.to_str() {
                            Some("json") => true,
                            Some("human") => false,
                            _ => return Err(usage("format must be human or json")),
                        }
                    }
                    _ => return Err(usage(format!("duplicate {operation} option"))),
                }
            }
            _ => return Err(usage(format!("unknown {operation} option"))),
        }
        if metadata > 1024 * 1024 {
            return Err(usage(format!(
                "{operation} CLI metadata exceeds whole-request budget"
            )));
        }
    }
    if help {
        return Ok(None);
    }
    let options = Options {
        plan: plan.ok_or_else(|| usage(format!("{operation} requires --plan")))?,
        document: document
            .ok_or_else(|| usage(format!("{operation} requires --document-output")))?,
        report: report.unwrap_or_else(|| OsStr::new("-")),
        sources,
        json,
        check,
        strict,
        replace_document,
        replace_report,
    };
    if options.sources.is_empty() {
        return Err(usage(format!(
            "{operation} requires explicit --source KEY PATH"
        )));
    }
    if options
        .sources
        .iter()
        .map(|(_, path)| *path)
        .chain(std::iter::once(options.plan))
        .filter(|path| *path == OsStr::new("-"))
        .count()
        > 1
    {
        return Err(usage("only one plan or source may use standard input"));
    }
    if options.document == OsStr::new("-") && options.report == OsStr::new("-") {
        return Err(usage("document and report cannot both use standard output"));
    }
    Ok(Some(options))
}

#[derive(Debug, Eq, PartialEq)]
enum Identity {
    Existing(same_file::Handle),
    #[cfg(unix)]
    Fifo {
        device: u64,
        inode: u64,
    },
    Absent {
        parent: same_file::Handle,
        name: OsString,
    },
}
fn identity(path: &Path) -> Result<Identity, Error> {
    match fs::metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::{FileTypeExt, MetadataExt};
                // Retaining a FIFO identity must not acquire its stream before input reading.
                if metadata.file_type().is_fifo() {
                    return Ok(Identity::Fifo {
                        device: metadata.dev(),
                        inode: metadata.ino(),
                    });
                }
            }
            #[cfg(not(unix))]
            let _ = metadata;
            same_file::Handle::from_path(path)
                .map(Identity::Existing)
                .map_err(|error| usage(format!("cannot retain file identity: {error}")))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = fs::canonicalize(parent)
                .map_err(|error| usage(format!("output/input parent unavailable: {error}")))?;
            let parent = same_file::Handle::from_path(parent)
                .map_err(|error| usage(format!("cannot retain parent identity: {error}")))?;
            let name = path
                .file_name()
                .ok_or_else(|| usage("explicit file basename required"))?
                .to_owned();
            Ok(Identity::Absent { parent, name })
        }
        Err(error) => Err(usage(format!("cannot establish file identity: {error}"))),
    }
}
struct Destination {
    path: Option<PathBuf>,
    identity: Option<Identity>,
    replace: bool,
}
/// Acquisition reuses native identity preflight without authoring replacement policy.
pub(super) struct NativeOutput(Destination, Option<cap_std::fs::Dir>);
impl NativeOutput {
    fn pin(destination: Destination) -> Result<Self, Error> {
        let Some(path) = &destination.path else {
            return Ok(Self(destination, None));
        };
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let directory = cap_std::fs::Dir::open_ambient_dir(parent, cap_std::ambient_authority())
            .map_err(|error| usage(format!("cannot pin acquisition output parent: {error}")))?;
        let opened = directory
            .try_clone()
            .and_then(|directory| same_file::Handle::from_file(directory.into_std_file()))
            .map_err(|error| usage(format!("cannot retain acquisition output parent: {error}")))?;
        if !matches!(&destination.identity, Some(Identity::Absent { parent, .. }) if *parent == opened)
        {
            return Err(usage("acquisition output parent changed during validation"));
        }
        Ok(Self(destination, Some(directory)))
    }
    pub(super) fn path(&self) -> Option<&Path> {
        self.0.path.as_deref()
    }
    pub(super) fn parent(&self) -> &cap_std::fs::Dir {
        self.1.as_ref().expect("named acquisition output")
    }
    pub(super) fn basename(&self) -> &OsStr {
        self.path()
            .and_then(Path::file_name)
            .expect("named acquisition output")
    }
    pub(super) fn confirm_parent_entry(&self, name: &OsStr, file: &File) -> std::io::Result<()> {
        let metadata = self.parent().symlink_metadata(name)?;
        if !metadata.is_file() || metadata.is_symlink() {
            return Err(std::io::Error::other(
                "entry is not the verified regular file",
            ));
        }
        #[cfg(unix)]
        let matches = {
            use cap_std::fs::MetadataExt;
            let verified = file.metadata()?;
            metadata.dev() == std::os::unix::fs::MetadataExt::dev(&verified)
                && metadata.ino() == std::os::unix::fs::MetadataExt::ino(&verified)
        };
        #[cfg(not(unix))]
        let matches = {
            let named = same_file::Handle::from_file(self.parent().open(name)?.into_std())?;
            let verified = same_file::Handle::from_file(file.try_clone()?)?;
            named == verified
        };
        if !matches {
            return Err(std::io::Error::other(
                "entry no longer names the verified staging inode",
            ));
        }
        Ok(())
    }
    pub(super) fn confirm(&self, file: &File) -> Result<(), Error> {
        let path = self.path().expect("named acquisition output");
        let actual = same_file::Handle::from_file(
            file.try_clone()
                .map_err(|error| host(format!("cannot retain delivered output: {error}")))?,
        )
        .map(Identity::Existing)
        .map_err(|error| host(format!("cannot retain delivered output: {error}")))?;
        if identity(path).map_err(|(_, message)| host(message))? != actual {
            return Err(host("delivered output identity changed"));
        }
        if fs::symlink_metadata(path)
            .map_err(|error| host(format!("cannot inspect delivered output: {error}")))?
            .file_type()
            .is_symlink()
        {
            return Err(host("delivered output became a symlink"));
        }
        Ok(())
    }
    pub(super) fn recheck(&self, other: &Self) -> Result<(), Error> {
        self.0.recheck(&[], &other.0, InputPolicy::AliasesOnly)
    }
}
pub(super) fn prepare_native_outputs(
    document: &OsStr,
    report: &OsStr,
) -> Result<(NativeOutput, NativeOutput), Error> {
    if document == OsStr::new("-") {
        return Err(usage("acquisition requires a named document destination"));
    }
    let document = Destination::validate(document, false, &[])?;
    let report = Destination::validate(report, false, &[])?;
    if document.identity.is_some() && document.identity == report.identity {
        return Err(usage("document and receipt destinations alias"));
    }
    Ok((NativeOutput::pin(document)?, NativeOutput::pin(report)?))
}
struct DeliveryFailure {
    message: String,
    committed: bool,
}
impl DeliveryFailure {
    fn before_commit((_, message): Error) -> Self {
        Self {
            message,
            committed: false,
        }
    }
}
struct ProtectedInput {
    path: PathBuf,
    identity: Identity,
}
/// A named sidecar file using the same replacement and input protections as document output.
pub(super) struct FileOutput {
    destination: Destination,
    protected: Vec<ProtectedInput>,
}
impl FileOutput {
    pub(super) fn prepare<'a>(
        path: &OsStr,
        replace: bool,
        inputs: impl Iterator<Item = &'a Path>,
    ) -> Result<Self, Error> {
        if path == OsStr::new("-") {
            return Err(usage("acknowledgements output requires a named file"));
        }
        let protected = inputs
            .map(|path| {
                Ok(ProtectedInput {
                    path: path.to_owned(),
                    identity: identity(path)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self {
            destination: Destination::validate(path, replace, &protected)?,
            protected,
        })
    }
    pub(super) fn emit(&self, bytes: &[u8]) -> Result<(), Error> {
        let stdout = Destination::validate(OsStr::new("-"), false, &[])?;
        self.destination
            .emit(bytes, &self.protected, &stdout, InputPolicy::Unchanged)
            .map_err(|error| {
                host(format!(
                    "{}; acknowledgements file {}",
                    error.message,
                    if error.committed {
                        "already committed"
                    } else {
                        "not committed"
                    },
                ))
            })
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum InputPolicy {
    AliasesOnly,
    Unchanged,
}
impl Destination {
    fn validate(path: &OsStr, replace: bool, protected: &[ProtectedInput]) -> Result<Self, Error> {
        if path == OsStr::new("-") {
            if replace {
                return Err(usage(
                    "replacement option does not apply to standard output",
                ));
            }
            return Ok(Self {
                path: None,
                identity: None,
                replace,
            });
        }
        let path = PathBuf::from(path);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !replace || !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(usage(
                        "existing output requires explicit replacement of a regular non-symlink file",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(usage(format!("cannot validate destination: {error}"))),
        }
        let target = identity(&path)?;
        if protected.iter().any(|input| input.identity == target) {
            return Err(usage("output aliases a protected source or plan input"));
        }
        Ok(Self {
            path: Some(path),
            identity: Some(target),
            replace,
        })
    }
    fn recheck(
        &self,
        protected: &[ProtectedInput],
        other: &Self,
        input_policy: InputPolicy,
    ) -> Result<(), Error> {
        let current = self
            .path
            .as_ref()
            .map(|path| identity(path).map_err(|(_, reason)| host(reason)))
            .transpose()?;
        if current.as_ref() != self.identity.as_ref() {
            return Err(host("output identity changed after validation"));
        }
        if current.is_none() && input_policy == InputPolicy::AliasesOnly {
            return Ok(());
        }
        for input in protected {
            let latest = identity(&input.path).map_err(|(_, reason)| host(reason))?;
            if input_policy == InputPolicy::Unchanged && latest != input.identity {
                return Err(host(format!(
                    "protected input identity changed after validation: {}",
                    input.path.display()
                )));
            }
            if current
                .as_ref()
                .is_some_and(|current| *current == input.identity || *current == latest)
            {
                return Err(host("output aliases a protected source or plan input"));
            }
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(other) = &other.path
            && current == Some(identity(other).map_err(|(_, reason)| host(reason))?)
        {
            return Err(host("output destinations now alias"));
        }
        if let Ok(metadata) = fs::symlink_metadata(path)
            && (!metadata.is_file() || metadata.file_type().is_symlink())
        {
            return Err(host("output is no longer a regular non-symlink file"));
        }
        Ok(())
    }
    fn emit(
        &self,
        bytes: &[u8],
        protected: &[ProtectedInput],
        other: &Self,
        input_policy: InputPolicy,
    ) -> Result<(), DeliveryFailure> {
        self.recheck(protected, other, input_policy)
            .map_err(DeliveryFailure::before_commit)?;
        let Some(path) = &self.path else {
            return std::io::stdout().lock().write_all(bytes).map_err(|error| {
                DeliveryFailure::before_commit(host(format!(
                    "cannot emit standard output: {error}"
                )))
            });
        };
        let mut staging = Staging::new(path).map_err(DeliveryFailure::before_commit)?;
        let staged = staging
            .file
            .write_all(bytes)
            .and_then(|_| staging.file.sync_all())
            .map_err(|error| host(format!("cannot stage output: {error}")));
        // Close the handle before cleanup, including on platforms that protect open files.
        drop(staging.file);
        let commit = staged
            .and_then(|_| self.recheck(protected, other, input_policy))
            .and_then(|_| {
                if self.replace && matches!(self.identity, Some(Identity::Existing(_))) {
                    fs::rename(&staging.path, path)
                } else {
                    fs::hard_link(&staging.path, path)
                }
                .map_err(|error| host(format!("cannot commit requested output: {error}")))
            });
        let committed = commit.is_ok();
        // Rename already consumes the staging name. Only established absence counts as cleanup.
        let cleanup = match fs::remove_file(&staging.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
        if let Err(error) = cleanup {
            let primary = commit
                .err()
                .map(|(_, reason)| format!("{reason}; "))
                .unwrap_or_default();
            return Err(DeliveryFailure {
                message: format!(
                    "{primary}staging cleanup failed: {error}; retained staging at {}",
                    staging.path.display()
                ),
                committed,
            });
        }
        commit.map_err(DeliveryFailure::before_commit)
    }
}
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Staging {
    path: PathBuf,
    file: File,
}
impl Staging {
    fn new(target: &Path) -> Result<Self, Error> {
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        for _ in 0..100 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".esdieff-stage-{}-{sequence}", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self { path, file });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(host(format!("cannot create staging file: {error}"))),
            }
        }
        Err(host("bounded staging-name attempts exhausted"))
    }
}
pub(super) fn read_plan(path: &OsStr, operation: &str) -> Result<Vec<u8>, Error> {
    let mut reader: Box<dyn Read> = if path == OsStr::new("-") {
        Box::new(std::io::stdin())
    } else {
        Box::new(
            File::open(path)
                .map_err(|error| host(format!("cannot acquire {operation} plan: {error}")))?,
        )
    };
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    while bytes.len() <= 4 * 1024 * 1024 {
        let available = chunk.len().min(4 * 1024 * 1024 + 1 - bytes.len());
        match reader.read(&mut chunk[..available]) {
            Ok(0) => return Ok(bytes),
            Ok(size) => {
                bytes.reserve_exact(size);
                bytes.extend_from_slice(&chunk[..size]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(host(format!("cannot read {operation} plan: {error}"))),
        }
    }
    Err(usage(format!(
        "{operation} plan exceeds 4 MiB byte ceiling"
    )))
}

pub(super) struct Delivery {
    document: Destination,
    report: Destination,
    protected: Vec<ProtectedInput>,
    input_policy: InputPolicy,
}
impl Delivery {
    pub(super) fn prepare(options: &Options<'_>) -> Result<Self, Error> {
        let protected = options
            .sources
            .iter()
            .map(|(_, path)| *path)
            .chain(std::iter::once(options.plan))
            .filter(|path| *path != OsStr::new("-"))
            .map(|path| {
                Ok(ProtectedInput {
                    path: PathBuf::from(path),
                    identity: identity(Path::new(path))?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let document =
            Destination::validate(options.document, options.replace_document, &protected)?;
        let report = Destination::validate(options.report, options.replace_report, &protected)?;
        if document.identity.is_some() && document.identity == report.identity {
            return Err(usage("document and report destinations alias"));
        }
        Ok(Self {
            document,
            report,
            protected,
            input_policy: InputPolicy::AliasesOnly,
        })
    }
    pub(super) fn require_unchanged_inputs(mut self) -> Self {
        self.input_policy = InputPolicy::Unchanged;
        self
    }
    pub(super) fn report_style(&self, color: ColorMode) -> HumanStyle {
        let style = if self.report.path.is_none() {
            color.stdout()
        } else {
            color.for_terminal(false)
        };
        if style.enabled() {
            HumanStyle::Ansi
        } else {
            HumanStyle::Plain
        }
    }
    fn document_status(&self, delivered: bool) -> String {
        if self.document.path.is_none() {
            return if delivered {
                "generated document already delivered to standard output (-)".to_owned()
            } else {
                "generated document delivery not completed to standard output (-), partial bytes may have been emitted".to_owned()
            };
        }
        let destination = self
            .document
            .path
            .as_ref()
            .map(|path| path.as_os_str())
            .unwrap_or_else(|| OsStr::new("-"));
        if delivered {
            format!(
                "generated document already delivered (committed) to {}",
                destination.to_string_lossy()
            )
        } else {
            format!(
                "generated document not delivered (not committed) to {}",
                destination.to_string_lossy()
            )
        }
    }
    pub(super) fn emit(&self, document: Option<&[u8]>, mut report: String) -> Result<(), Error> {
        if let Some(bytes) = document {
            self.document
                .emit(bytes, &self.protected, &self.report, self.input_policy)
                .map_err(|error| {
                    host(format!(
                        "{}; {}",
                        error.message,
                        self.document_status(error.committed)
                    ))
                })?;
        }
        if !report.ends_with('\n') {
            report.push('\n');
        }
        self.report
            .emit(
                report.as_bytes(),
                &self.protected,
                &self.document,
                self.input_policy,
            )
            .map_err(|error| {
                host(format!(
                    "{}; {}",
                    error.message,
                    self.document_status(document.is_some())
                ))
            })
    }
}
