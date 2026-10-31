//! Fixed layout and charged Markdown presentation for a selected grouping.
use super::*;
mod uses;
use crate::allowance::charge;
use crate::budgeted::{self, Exhausted};
use crate::effective::Collection;
use crate::generation::ReferenceExamples;
use crate::{
    Acquisition, AlternativeId, Characteristic, EffectiveDataDescription, EffectiveDefinition,
    EffectiveJson, EffectiveModel, Field, JsonView, Lookup, RequiredDeclarations, SampleKind,
};

/// Mandatory text is measured before any operation work. Slack covers fixed statuses,
/// omission counters and processing summaries, including an unknown remainder.
pub(super) struct Envelope {
    identity: String,
    attempts: String,
    pub(super) reserve: usize,
    pub(super) allocation_reserve: usize,
}
fn escaped_size(text: &str) -> usize {
    text.chars()
        .map(|ch| match ch {
            '&' | ':' | '@' => 5,
            '<' | '>' => 4,
            '\n' | '\r' => 5,
            ch if ch.is_control() => 6,
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' | '~' => 2,
            _ => ch.len_utf8(),
        })
        .sum()
}
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(escaped_size(text));
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            ':' => out.push_str("&#58;"),
            '@' => out.push_str("&#64;"),
            '\n' | '\r' => out.push_str(" ⏎ "),
            ch if ch.is_control() => {
                use std::fmt::Write;
                write!(out, "\\u{:04x}", ch as u32).expect("String write");
            }
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}
impl Envelope {
    pub(super) fn new(
        catalogue: &Catalogue,
        request: &ReferenceRequest<'_>,
    ) -> Result<Self, &'static str> {
        // The selected Given Name also appears in the title. Its decoded spelling
        // cannot exceed the pointer's escaped size, including imported positions.
        let identity_size = escaped_size(request.pointer)
            .saturating_mul(2)
            .saturating_add(256);
        let attempt_size = catalogue
            .retained_sources()
            .try_fold(0usize, |size, source| {
                size.checked_add(escaped_size(source.label().unwrap_or(source.key())) + 160)
            })
            .ok_or("reference source inventory exceeds report allowance")?;
        let reserve = identity_size
            .saturating_add(attempt_size)
            .saturating_add(2048);
        let allocation_reserve = reserve.saturating_mul(3);
        if reserve > catalogue.limits.report_bytes
            || allocation_reserve > catalogue.limits.effective_allocation
        {
            return Err(
                "report cannot retain mandatory reference identity, source attempts and omission summary",
            );
        }
        let mut attempts = String::new();
        for source in catalogue.retained_sources() {
            use std::fmt::Write;
            writeln!(
                attempts,
                "- Source {}: {}. Acquisition: {}. Content: {}.{}",
                source.id().get(),
                escape(source.label().unwrap_or(source.key())),
                source.acquisition().as_str(),
                if source.parsed.errors > 0 {
                    "decoding failed"
                } else if source.json().present().is_some() && source.content_complete() {
                    "retained"
                } else {
                    "unavailable or limited"
                },
                if source.label_omitted {
                    " Supplied label unavailable."
                } else {
                    ""
                },
            )
            .expect("String write");
        }
        Ok(Self {
            identity: format!(
                "# Model reference: {}\n\nSelected effective pointer: {}\n\n",
                escape(
                    request
                        .pointer
                        .rsplit('/')
                        .next()
                        .unwrap_or(request.pointer)
                ),
                escape(request.pointer)
            ),
            attempts,
            reserve,
            allocation_reserve,
        })
    }
}

struct Writer<'a> {
    catalogue: &'a Catalogue,
    body: String,
    metadata: String,
    contents: String,
    sources: String,
    room: usize,
    pointers: usize,
    budget: OperationBudget,
    conditions: Vec<ReferenceCondition>,
    omissions: ReferenceOmissions,
    failed: bool,
    partial: bool,
    next_definition: usize,
    next_source: usize,
    lines: Vec<(SourceId, Vec<usize>)>,
}
impl Writer<'_> {
    /// Escaping is charged before allocating, including bytes amplified by Markdown escapes.
    fn text(&mut self, raw: &str) -> Result<Option<String>, Exhausted> {
        budgeted::work(&mut self.budget, raw.len() + 1)?;
        let size = escaped_size(raw);
        if size > self.room {
            self.omissions.bytes += 1;
            return Ok(None);
        }
        if !charge(&mut self.budget.allocation, size) {
            return Err(Exhausted::Allocation);
        }
        Ok(Some(escape(raw)))
    }
    fn row(&mut self, pieces: &[&str]) -> Result<bool, Exhausted> {
        let size: usize = pieces.iter().map(|p| p.len()).sum();
        budgeted::work(&mut self.budget, size + 1)?;
        if size > self.room {
            self.omissions.bytes += 1;
            return Ok(false);
        }
        // Retained String capacity and final Markdown assembly are both bounded.
        if !charge(&mut self.budget.allocation, size.saturating_mul(4)) {
            return Err(Exhausted::Allocation);
        }
        self.room -= size;
        for piece in pieces {
            self.body.push_str(piece);
        }
        Ok(true)
    }
    fn condition(
        &mut self,
        code: &'static str,
        message: &'static str,
        definite: bool,
    ) -> Result<(), Exhausted> {
        self.failed |= definite;
        self.partial = true;
        if self.conditions.len() >= self.catalogue.limits.conditions {
            self.omissions.conditions += 1;
        } else if !self
            .conditions
            .iter()
            .any(|c| c.code == code && c.message == message && c.definite == definite)
        {
            budgeted::kept(
                &mut self.conditions,
                ReferenceCondition {
                    code,
                    message,
                    definite,
                },
                &mut self.budget,
            )?;
        }
        self.row(&["\nProcessing condition: ", code, ". ", message, "\n\n"])?;
        Ok(())
    }
    fn unavailable(&mut self, reason: &'static str) -> Result<(), Exhausted> {
        self.condition("reference.content-unavailable", reason, false)
    }
    fn pointer(&mut self, json: EffectiveJson<'_>) -> Result<Option<String>, Exhausted> {
        budgeted::work(&mut self.budget, 1)?;
        let cap = self
            .catalogue
            .limits
            .pointer_bytes
            .min(self.pointers)
            .min(8192);
        if !charge(&mut self.budget.allocation, cap) {
            return Err(Exhausted::Allocation);
        }
        let pointer = json.pointer(cap as u32);
        self.budget.allocation += cap
            - match &pointer {
                Field::Present(p) => p.len(),
                _ => 0,
            };
        match pointer {
            Field::Present(pointer) if pointer.len() <= self.pointers => {
                self.pointers -= pointer.len();
                Ok(Some(pointer))
            }
            _ => {
                self.omissions.pointers += 1;
                self.row(&["\nEffective pointer unavailable: pointer allowance.\n\n"])?;
                Ok(None)
            }
        }
    }
    fn heading(
        &mut self,
        name: &str,
        class: &str,
        json: EffectiveJson<'_>,
        named: bool,
    ) -> Result<bool, Exhausted> {
        let Some(name) = self.text(name)? else {
            self.omissions.entries += 1;
            return Ok(false);
        };
        let id = self.next_definition;
        self.next_definition += 1;
        if !charge(
            &mut self.budget.allocation,
            name.len().saturating_mul(2) + class.len().saturating_mul(2) + 200,
        ) {
            return Err(Exhausted::Allocation);
        }
        let anchor = format!("definition-{id}");
        let heading = format!("\n<a id=\"{anchor}\"></a>\n\n## {name} ({class})\n\n");
        let toc = if named {
            format!("- [{name} ({class})](#{anchor})\n")
        } else {
            String::new()
        };
        if heading.len() + toc.len() > self.room {
            self.omissions.bytes += 1;
            self.omissions.entries += 1;
            return Ok(false);
        }
        let size = heading.len() + toc.len();
        budgeted::work(&mut self.budget, size + 1)?;
        if !charge(&mut self.budget.allocation, size.saturating_mul(4)) {
            return Err(Exhausted::Allocation);
        }
        self.room -= size;
        self.contents.push_str(&toc);
        self.body.push_str(&heading);
        if let Some(pointer) = self.pointer(json)?
            && let Some(pointer) = self.text(&pointer)?
        {
            self.row(&["Effective pointer: ", &pointer, "\n\n"])?;
        }
        self.origin(json)?;
        for key in ["label", "description"] {
            match json.get(key) {
                Field::Present(value) => self.quality(key, value, "")?,
                Field::Unavailable(reason) => self.unavailable(reason)?,
                Field::Absent => {}
            }
        }
        Ok(true)
    }
    fn source_entry(&mut self, json: JsonView<'_>) -> Result<Option<(String, String)>, Exhausted> {
        let source = self
            .catalogue
            .source(json.source_id())
            .expect("origin source");
        let cap = self
            .catalogue
            .limits
            .pointer_bytes
            .min(self.pointers)
            .min(8192);
        if !charge(&mut self.budget.allocation, cap) {
            return Err(Exhausted::Allocation);
        }
        let pointer = json.pointer(cap as u32).present();
        self.budget.allocation += cap - pointer.as_ref().map_or(0, String::len);
        let Some(pointer) = pointer else {
            self.omissions.pointers += 1;
            return Ok(None);
        };
        if pointer.len() > self.pointers {
            self.omissions.pointers += 1;
            return Ok(None);
        }
        self.pointers -= pointer.len();
        let Some(label) = self.text(source.label().unwrap_or(source.key()))? else {
            return Ok(None);
        };
        let Some(pointer) = self.text(&pointer)? else {
            return Ok(None);
        };
        if !charge(&mut self.budget.allocation, 64) {
            return Err(Exhausted::Allocation);
        }
        let span = json.span();
        let position = if source.has_original_bytes() && span.start <= source.raw_bytes().len() {
            if !self.lines.iter().any(|(id, _)| *id == source.id()) {
                budgeted::work(&mut self.budget, source.raw_bytes().len() + 1)?;
                let mut starts = Vec::new();
                budgeted::kept(&mut starts, 0, &mut self.budget)?;
                for (index, byte) in source.raw_bytes().iter().enumerate() {
                    if *byte == b'\n' {
                        budgeted::kept(&mut starts, index + 1, &mut self.budget)?;
                    }
                }
                budgeted::kept(&mut self.lines, (source.id(), starts), &mut self.budget)?;
            }
            let starts = &self
                .lines
                .iter()
                .find(|(id, _)| *id == source.id())
                .expect("indexed source")
                .1;
            let line = starts.partition_point(|start| *start <= span.start);
            let col = span.start - starts[line - 1] + 1;
            format!("line {line}, column {col}")
        } else {
            "source position unavailable".to_owned()
        };
        if !charge(
            &mut self.budget.allocation,
            label.len() + pointer.len() + 256,
        ) {
            return Err(Exhausted::Allocation);
        }
        let id = self.next_source;
        self.next_source += 1;
        Ok(Some((
            format!("[source {id}](#source-{id})"),
            format!(
                "\n<a id=\"source-{id}\"></a>\n\n- Source {id}: {label}, pointer {pointer}, {position}.\n"
            ),
        )))
    }
    fn origin(&mut self, json: EffectiveJson<'_>) -> Result<(), Exhausted> {
        let explanation = json.explanation();
        let mut links = Vec::new();
        let source_count = explanation.sources().count();
        for (index, source) in explanation
            .sources()
            .chain(explanation.consumer())
            .enumerate()
        {
            if let Some((link, entry)) = self.source_entry(source)? {
                budgeted::kept(&mut links, (index, link, entry), &mut self.budget)?;
            } else {
                self.partial = true;
                self.row(&["Source position unavailable or omitted.\n\n"])?;
            }
        }
        let size: usize = links
            .iter()
            .map(|(_, link, entry)| link.len() + entry.len() + 4)
            .sum::<usize>()
            + 100;
        if size > self.room {
            self.omissions.bytes += 1;
            return Ok(());
        }
        // Admit links and their destination entries together. An exhausted allowance
        // must never retain a link whose source-index entry was lost.
        budgeted::work(&mut self.budget, size + 1)?;
        if !charge(&mut self.budget.allocation, size.saturating_mul(4)) {
            return Err(Exhausted::Allocation);
        }
        let before = self.body.len() + self.sources.len();
        self.body.push_str("Origin: ");
        self.body.push_str(explanation.kind().as_str());
        self.body.push_str(". ");
        for (index, link, entry) in links {
            if index > 0 {
                self.body.push_str(if index < source_count {
                    " Previous source: "
                } else {
                    " Importing consumer: "
                });
            }
            self.body.push_str(&link);
            self.body.push_str(". ");
            self.sources.push_str(&entry);
        }
        self.body.push_str("\n\n");
        self.room -= self.body.len() + self.sources.len() - before;
        Ok(())
    }
    fn quality(
        &mut self,
        name: &str,
        value: EffectiveJson<'_>,
        qualification: &str,
    ) -> Result<(), Exhausted> {
        let Some(name) = self.text(name)? else {
            return Ok(());
        };
        let mut work = self.budget.work;
        let mut allocation = self.budget.allocation;
        let value_text = value.render_for_report(self.room, &mut work, &mut allocation);
        self.budget.work = work;
        self.budget.allocation = allocation;
        let raw = match value_text {
            Field::Present(raw) => raw,
            _ => {
                self.omissions.bytes += 1;
                return Ok(());
            }
        };
        let Some(raw) = self.text(&raw)? else {
            return Ok(());
        };
        if self.row(&["- ", &name, ": ", &raw, qualification, "\n"])? {
            self.origin(value)?;
        }
        Ok(())
    }
    fn characteristic(
        &mut self,
        key: &str,
        value: Field<Characteristic>,
        json: EffectiveJson<'_>,
    ) -> Result<(), Exhausted> {
        match value {
            Field::Present(Characteristic::Declared(value)) => {
                self.row(&[
                    "- ",
                    key,
                    ": ",
                    if value { "true" } else { "false" },
                    " (declared).\n",
                ])?;
                if let Field::Present(value) = json.get(key) {
                    self.origin(value)?;
                }
            }
            Field::Present(Characteristic::SpecifiedDefault(value)) => {
                self.row(&[
                    "- ",
                    key,
                    ": ",
                    if value { "true" } else { "false" },
                    " (specified default, no authored origin).\n",
                ])?;
            }
            Field::Unavailable(reason) => {
                self.row(&["- ", key, ": meaning unavailable. ", reason, "\n"])?;
                if let Field::Present(value) = json.get(key) {
                    self.quality(key, value, " (declared content, meaning not established)")?;
                }
            }
            Field::Absent => {}
        }
        Ok(())
    }
}

enum Task<'a> {
    Definition(&'a str, Class, Field<EffectiveDefinition<'a>>, usize),
    Data(
        &'a str,
        Field<EffectiveDataDescription<'a>>,
        usize,
        Option<SampleKind>,
    ),
}

pub(super) fn render<'a>(
    catalogue: &'a Catalogue,
    request: ReferenceRequest<'_>,
    envelope: Envelope,
    model: &EffectiveModel<'a>,
    required: Option<&Collection<'a>>,
    budget: OperationBudget,
) -> Result<ReferenceResult<'a>, &'static str> {
    let mut writer = Writer {
        catalogue,
        body: String::new(),
        metadata: String::new(),
        contents: String::new(),
        sources: String::new(),
        room: catalogue.limits.report_bytes - envelope.reserve,
        pointers: catalogue.limits.pointer_total,
        budget,
        conditions: Vec::new(),
        omissions: ReferenceOmissions::default(),
        failed: model.reference_errors > 0
            || required.is_some_and(|r| r.errors > 0)
            || model.failure.is_some_and(|f| f.definite())
            || std::iter::once(request.document)
                .chain(model.dependencies.iter().copied())
                .chain(model.unavailable_dependencies.iter().copied())
                .any(|id| {
                    catalogue.source(id).is_some_and(|s| {
                        s.parsed.errors > 0 || s.acquisition() == Acquisition::Failed
                    })
                }),
        partial: false,
        next_definition: 1,
        next_source: 1,
        lines: Vec::new(),
    };
    let process = (|| -> Result<(), Exhausted> {
        let selected_source = catalogue
            .source(request.document)
            .expect("validated source");
        if selected_source.parsed.errors > 0 || selected_source.acquisition() == Acquisition::Failed
        {
            writer.condition(
                "reference.input-failed",
                "Selected input acquisition or decoding failed.",
                true,
            )?;
        }
        if model.reference_errors > 0 {
            writer.condition(
                "reference.composition-failed",
                "Composition established a definite reference failure.",
                true,
            )?;
        }
        if let Some(failure) = model.failure {
            writer.condition(failure.code(), failure.reason, failure.definite())?;
        }
        if let Some(required) = required
            && required.errors > 0
        {
            writer.condition(
                "reference.required-failed",
                "Required declaration inspection established a definite failure.",
                true,
            )?;
        }
        let conditions = std::mem::take(&mut writer.body);
        for key in ["title", "version"] {
            if let Some(value) = selected_source
                .json()
                .present()
                .and_then(|r| r.get("info").present())
                .and_then(|info| info.get(key).present())
                .and_then(|v| v.text().present())
                && let Some(value) = writer.text(value)?
            {
                writer.row(&["Document ", key, ": ", &value, "\n\n"])?;
            }
        }
        writer.metadata = std::mem::replace(&mut writer.body, conditions);
        let root = match model.json() {
            Field::Present(root) => root,
            Field::Unavailable(reason) => {
                writer.unavailable(reason)?;
                return Ok(());
            }
            Field::Absent => {
                writer.unavailable("Selected grouping unavailable.")?;
                return Ok(());
            }
        };
        let class = match root.n().role {
            Some(crate::role::Role::Definition(class)) => class,
            _ => {
                writer.unavailable("Selected grouping class unavailable.")?;
                return Ok(());
            }
        };
        let mut stack = Vec::new();
        budgeted::kept(
            &mut stack,
            Task::Definition(
                root.n().key.unwrap_or("Selected grouping"),
                class,
                Field::Present(EffectiveDefinition::from_json(root, class)),
                0,
            ),
            &mut writer.budget,
        )?;
        let mut examples = ReferenceExamples::new(catalogue, request.document);
        while let Some(task) = stack.pop() {
            budgeted::work(&mut writer.budget, 1)?;
            let depth = match &task {
                Task::Definition(_, _, _, depth) | Task::Data(_, _, depth, _) => *depth,
            };
            if depth > catalogue.limits.depth.min(64) {
                writer.omissions.entries += 1;
                writer.omissions.unknown_remainder = true;
                writer.unavailable("Reference traversal depth limit.")?;
                continue;
            }
            match task {
                Task::Definition(name, class, field, depth) => {
                    let definition = match field {
                        Field::Present(value) => value,
                        Field::Unavailable(reason) => {
                            if let Some(name) = writer.text(name)? {
                                writer.row(&["\nUnavailable definition: ", &name, "\n"])?;
                            }
                            writer.unavailable(reason)?;
                            continue;
                        }
                        Field::Absent => continue,
                    };
                    let json = definition.json();
                    if !writer.heading(name, class.keyword(), json, true)? {
                        writer.omissions.unknown_remainder = true;
                        continue;
                    }
                    if let Some(failure) = budgeted::failure(json) {
                        writer.condition(failure.code(), failure.reason, failure.definite())?;
                    }
                    if class == Class::Context {
                        writer.row(&[
                            "Context is non-interactive metadata. Recorded draft: ",
                            crate::context::DRAFT,
                            ".\n\n",
                        ])?;
                    }
                    requiredness(&mut writer, required, model, json)?;
                    if matches!(class, Class::Thing | Class::Object) {
                        for key in ["minItems", "maxItems"] {
                            if let Field::Present(value) = json.get(key) {
                                writer.quality(key, value, " (grouping repetition)")?;
                            }
                        }
                    }
                    if class == Class::Property {
                        for (key, field) in [
                            ("readable", definition.readable_characteristic()),
                            ("writable", definition.writable_characteristic()),
                            ("observable", definition.observable_characteristic()),
                        ] {
                            writer.characteristic(key, field, json)?;
                        }
                    }
                    let mut children = Vec::new();
                    if matches!(class, Class::Property | Class::Data | Class::Context) {
                        data(
                            &mut writer,
                            definition.data_description(),
                            depth,
                            &mut children,
                        )?;
                        if class != Class::Data {
                            example(
                                &mut writer,
                                &mut examples,
                                if class == Class::Context {
                                    SampleKind::Context
                                } else {
                                    SampleKind::Property
                                },
                                json,
                            )?;
                        }
                    }
                    if class == Class::Action {
                        budgeted::kept(
                            &mut children,
                            Task::Data(
                                "Input",
                                definition.input(),
                                depth + 1,
                                Some(SampleKind::ActionInput),
                            ),
                            &mut writer.budget,
                        )?;
                    }
                    if matches!(class, Class::Action | Class::Event) {
                        budgeted::kept(
                            &mut children,
                            Task::Data(
                                "Output",
                                definition.output(),
                                depth + 1,
                                Some(if class == Class::Action {
                                    SampleKind::ActionOutput
                                } else {
                                    SampleKind::EventOutput
                                }),
                            ),
                            &mut writer.budget,
                        )?;
                    }
                    for child_class in [
                        Class::Property,
                        Class::Action,
                        Class::Event,
                        Class::Context,
                        Class::Data,
                        Class::Object,
                        Class::Thing,
                    ] {
                        if !class.allows_child(child_class) {
                            continue;
                        }
                        match definition.children(child_class) {
                            Field::Present(rows) => {
                                for (name, value) in rows {
                                    budgeted::work(&mut writer.budget, name.len() + 1)?;
                                    budgeted::kept(
                                        &mut children,
                                        Task::Definition(name, child_class, value, depth + 1),
                                        &mut writer.budget,
                                    )?;
                                }
                            }
                            Field::Unavailable(reason) => {
                                writer.row(&[
                                    "\nUnavailable collection: ",
                                    child_class.keyword(),
                                    ".\n",
                                ])?;
                                writer.unavailable(reason)?;
                            }
                            Field::Absent => {}
                        }
                    }
                    retained_definition_content(&mut writer, json, class)?;
                    for child in children.into_iter().rev() {
                        budgeted::kept(&mut stack, child, &mut writer.budget)?;
                    }
                }
                Task::Data(name, field, depth, kind) => {
                    let description = match field {
                        Field::Present(description) => description,
                        Field::Absent => {
                            if let Some(name) = writer.text(name)? {
                                writer.row(&["\n", &name, ": No data description.\n\n"])?;
                            }
                            continue;
                        }
                        Field::Unavailable(reason) => {
                            if let Some(name) = writer.text(name)? {
                                writer.row(&["\n", &name, ": data description unavailable.\n"])?;
                            }
                            writer.unavailable(reason)?;
                            continue;
                        }
                    };
                    if !writer.heading(name, "data description", description.json(), true)? {
                        writer.omissions.unknown_remainder = true;
                        continue;
                    }
                    let mut children = Vec::new();
                    data(
                        &mut writer,
                        Field::Present(description),
                        depth,
                        &mut children,
                    )?;
                    if let Some(kind) = kind {
                        example(&mut writer, &mut examples, kind, description.json())?;
                    }
                    for child in children.into_iter().rev() {
                        budgeted::kept(&mut stack, child, &mut writer.budget)?;
                    }
                }
            }
        }
        if request.uses {
            uses::render(&mut writer, &request)?;
        }
        Ok(())
    })();
    if process.is_err() {
        writer.partial = true;
        writer.omissions.unknown_remainder = true;
    }
    let complete = !writer.partial && !writer.omissions.any();
    let status = if writer.failed {
        "Definite requested processing failure"
    } else if complete {
        "Completed documentation"
    } else {
        "Partial reference"
    };
    let mut markdown = if let Some(root) = model.json().present() {
        let class = match root.n().role {
            Some(crate::role::Role::Definition(class)) => class.keyword(),
            _ => "class unavailable",
        };
        format!(
            "# {} ({class})\n\nSelected effective pointer: {}\n\n",
            escape(root.n().key.unwrap_or("Selected grouping")),
            escape(request.pointer)
        )
    } else {
        envelope.identity
    };
    use std::fmt::Write;
    writeln!(markdown, "Documentation status: {status}.\n\nRFC 9880. This reference documents the effective interface. It is not a Full model assessment, compatibility verdict, satisfiability proof or device test. Run `esdieff check` separately. Format, unit and pattern spellings do not establish that every use of their meaning was checked.\n").expect("String write");
    markdown.push_str(&writer.metadata);
    markdown.push_str("## Contents\n\n");
    markdown.push_str(&writer.contents);
    markdown.push_str(&writer.body);
    markdown.push_str("\n## Source index\n\nSupplied source attempts:\n\n");
    markdown.push_str(&envelope.attempts);
    markdown.push_str(&writer.sources);
    markdown.push_str("\n## Processing conditions and omissions\n\n");
    for condition in &writer.conditions {
        // Conditions were already rendered locally. This fixed summary retains their
        // severity even if optional presentation ran out.
        if condition.definite {
            markdown.push_str("Definite processing failures were established.\n\n");
            break;
        }
    }
    writeln!(markdown, "Omitted entries: {}. Byte-limited rows or sections: {}. Unavailable pointers: {}. Unretained conditions: {}. Unknown remainder count: {}.{}",
        writer.omissions.entries, writer.omissions.bytes, writer.omissions.pointers, writer.omissions.conditions,
        if writer.omissions.unknown_remainder { "yes" } else { "no" },
        if process.is_err() { " Processing stopped at the work or allocation allowance." } else { "" },
    ).expect("String write");
    debug_assert!(markdown.len() <= catalogue.limits.report_bytes);
    Ok(ReferenceResult {
        catalogue,
        document: request.document,
        markdown,
        complete,
        failed: writer.failed,
        conditions: writer.conditions,
        omissions: writer.omissions,
    })
}

fn requiredness(
    writer: &mut Writer<'_>,
    collection: Option<&Collection<'_>>,
    model: &EffectiveModel<'_>,
    json: EffectiveJson<'_>,
) -> Result<(), Exhausted> {
    let Some(collection) = collection else {
        writer.unavailable("Required declaration inspection unavailable.")?;
        return Ok(());
    };
    let view = RequiredDeclarations {
        collection,
        model: Some(model),
    };
    if let Field::Present(value) = json.get("sdfRequired") {
        writer.quality(
            "sdfRequired",
            value,
            " (declaration syntax, relationships inspected below)",
        )?;
    }
    let mut found = false;
    for owner in view.owners() {
        budgeted::work(&mut writer.budget, 1)?;
        if owner
            .effective_owner()
            .present()
            .is_none_or(|value| value.node != json.node)
        {
            continue;
        }
        found = true;
        writer.row(&[
            "Declaration-requiredness: ",
            owner.state().as_str(),
            ".\n\n",
        ])?;
        if !owner.complete() {
            if matches!(
                json.n().role,
                Some(crate::role::Role::Definition(Class::Context))
            ) && collection.traversed
            {
                writer.row(&["Context declaration-requiredness meaning is not established under the recorded draft.\n\n"])?;
                // The complete declaration spelling above documents this unsupported
                // draft use, including malformed scalar syntax. It has no established
                // declaration targets to enumerate.
                continue;
            } else {
                writer.unavailable("Required declaration relationships incomplete.")?;
            }
        }
        match owner.references() {
            Field::Present(references) => {
                for reference in references {
                    budgeted::work(&mut writer.budget, 1)?;
                    writer.row(&[
                        "- Required declaration relationship: ",
                        reference.status().as_str(),
                        ".\n",
                    ])?;
                    if let Lookup::Found(targets) = reference.targets() {
                        for target in targets {
                            budgeted::work(&mut writer.budget, 1)?;
                            if let Some(name) = writer.text(target.given_name())? {
                                writer.row(&[
                                    "  - ",
                                    &name,
                                    " (",
                                    target.class().keyword(),
                                    ")\n",
                                ])?;
                            }
                        }
                    }
                }
            }
            Field::Unavailable(reason) => writer.unavailable(reason)?,
            Field::Absent => {}
        }
    }
    if !found {
        if collection.traversed {
            writer.row(&[
                "Declaration-requiredness: no declared relationship on this definition.\n\n",
            ])?;
        } else {
            writer.unavailable("Required declaration owner inventory incomplete.")?;
        }
    }
    Ok(())
}

fn retained_definition_content(
    writer: &mut Writer<'_>,
    json: EffectiveJson<'_>,
    class: Class,
) -> Result<(), Exhausted> {
    if matches!(class, Class::Property | Class::Data | Class::Context) {
        return Ok(());
    }
    for value in json.children() {
        budgeted::work(&mut writer.budget, 1)?;
        let Some(key) = value.n().key else {
            writer.unavailable("Quality identity unavailable.")?;
            continue;
        };
        if matches!(key, "label" | "description" | "sdfRef" | "sdfRequired")
            || [
                Class::Property,
                Class::Action,
                Class::Event,
                Class::Context,
                Class::Data,
                Class::Object,
                Class::Thing,
            ]
            .iter()
            .any(|c| c.keyword() == key && class.allows_child(*c))
            || key == "sdfInputData" && class == Class::Action
            || key == "sdfOutputData" && matches!(class, Class::Action | Class::Event)
            || matches!(key, "minItems" | "maxItems")
                && matches!(class, Class::Thing | Class::Object)
        {
            continue;
        }
        writer.quality(key, value, " (declared content, meaning not established)")?;
    }
    Ok(())
}

/// Typed data access determines which retained qualities have supported meaning in this
/// role. Unknown and malformed qualities are documentary content, never enforced claims.
fn supported(description: EffectiveDataDescription<'_>, key: &str) -> bool {
    match key {
        "type" => matches!(
            description.data_type(),
            Field::Present(crate::DataType::Known(_))
        ),
        "sdfType" => matches!(description.sdf_type(), Field::Present(_)),
        "format" => matches!(description.format(), Field::Present(_)),
        "contentFormat" => matches!(description.content_format(), Field::Present(_)),
        "unit" => matches!(description.unit(), Field::Present(_)),
        "pattern" => matches!(description.pattern(), Field::Present(_)),
        "minimum" => matches!(description.minimum(), Field::Present(_)),
        "maximum" => matches!(description.maximum(), Field::Present(_)),
        "exclusiveMinimum" => matches!(description.exclusive_minimum(), Field::Present(_)),
        "exclusiveMaximum" => matches!(description.exclusive_maximum(), Field::Present(_)),
        "multipleOf" => matches!(description.multiple_of(), Field::Present(_)),
        "minLength" => matches!(description.minimum_length(), Field::Present(_)),
        "maxLength" => matches!(description.maximum_length(), Field::Present(_)),
        "minItems" => matches!(description.minimum_items(), Field::Present(_)),
        "maxItems" => matches!(description.maximum_items(), Field::Present(_)),
        "uniqueItems" => matches!(description.unique_items(), Field::Present(_)),
        "const" => matches!(description.constant(), Field::Present(_)),
        "default" => matches!(description.declared_default(), Field::Present(_)),
        "enum" => matches!(description.source_enum(), Field::Present(_)),
        "required" => matches!(description.required_entries(), Field::Present(_)),
        _ => false,
    }
}
fn data<'a>(
    writer: &mut Writer<'_>,
    field: Field<EffectiveDataDescription<'a>>,
    depth: usize,
    children: &mut Vec<Task<'a>>,
) -> Result<(), Exhausted> {
    let description = match field {
        Field::Present(value) => value,
        Field::Unavailable(reason) => {
            writer.unavailable(reason)?;
            return Ok(());
        }
        Field::Absent => return Ok(()),
    };
    let json = description.json();
    writer.characteristic("nullable", description.nullable_characteristic(), json)?;
    writer.row(&["\nConstraints and declared qualities (exact JSON values):\n\n"])?;
    for value in json.children() {
        budgeted::work(&mut writer.budget, 1)?;
        let Some(key) = value.n().key else {
            writer.unavailable("Data quality identity unavailable.")?;
            continue;
        };
        if matches!(key, "label" | "description" | "nullable" | "sdfRef") {
            continue;
        }
        if matches!(key, "readable" | "writable" | "observable")
            && matches!(
                json.n().role,
                Some(crate::role::Role::Definition(Class::Property))
            )
        {
            continue;
        }
        if matches!(key, "properties" | "items" | "sdfChoice") {
            continue;
        }
        let qualification = if key == "default" {
            " (documentary default, not an inserted value or an admitted example)"
        } else if supported(description, key) {
            ""
        } else {
            " (declared content, meaning not established)"
        };
        writer.quality(key, value, qualification)?;
    }
    match description.properties() {
        Field::Present(properties) => {
            for (name, field) in properties {
                budgeted::work(&mut writer.budget, name.len() + 1)?;
                if let Some(name_text) = writer.text(name)? {
                    let required = match description.required_entries() {
                        Field::Present(entries) => {
                            let mut required = false;
                            for entry in entries {
                                budgeted::work(&mut writer.budget, name.len() + 1)?;
                                required |= entry.text().present() == Some(name);
                            }
                            Some(required)
                        }
                        Field::Absent => Some(false),
                        Field::Unavailable(_) => None,
                    };
                    writer.row(&[
                        "- Map entry ",
                        &name_text,
                        ": ",
                        match required {
                            Some(true) => "required",
                            Some(false) => "optional",
                            None => "requiredness unavailable",
                        },
                        " (separate from nullability).\n",
                    ])?;
                }
                budgeted::kept(
                    children,
                    Task::Data(name, field, depth + 1, None),
                    &mut writer.budget,
                )?;
            }
        }
        Field::Unavailable(reason) => {
            writer.unavailable(reason)?;
            if let Field::Present(value) = json.get("properties") {
                writer.quality(
                    "properties",
                    value,
                    " (declared content, meaning not established)",
                )?;
            }
        }
        Field::Absent => {}
    }
    match description.items() {
        Field::Present(items) => budgeted::kept(
            children,
            Task::Data("Items", Field::Present(items), depth + 1, None),
            &mut writer.budget,
        )?,
        Field::Unavailable(reason) => {
            // Role boundaries are documented locally. Retaining the declaration fulfills
            // documentation, while inability to enumerate known nested content is partial.
            writer.row(&["\nItems meaning unavailable: ", reason, ".\n"])?;
            if let Field::Present(value) = json.get("items") {
                writer.quality(
                    "items",
                    value,
                    " (declared content, meaning not established)",
                )?;
            }
        }
        Field::Absent => {}
    }
    match description.alternatives() {
        Field::Present(alternatives) => {
            for (id, field) in alternatives {
                let name = match id {
                    AlternativeId::Choice(name) => name,
                    AlternativeId::Enum(_) => "Enum alternative",
                };
                budgeted::kept(
                    children,
                    Task::Data(name, field, depth + 1, None),
                    &mut writer.budget,
                )?;
            }
        }
        Field::Unavailable(reason) => {
            writer.unavailable(reason)?;
            if let Field::Present(value) = json.get("sdfChoice") {
                writer.quality(
                    "sdfChoice",
                    value,
                    " (declared content, meaning not established)",
                )?;
            }
        }
        Field::Absent => {}
    }
    Ok(())
}

fn example(
    writer: &mut Writer<'_>,
    examples: &mut ReferenceExamples<'_>,
    kind: SampleKind,
    json: EffectiveJson<'_>,
) -> Result<(), Exhausted> {
    let Some(mut pointer) = writer.pointer(json)? else {
        writer.row(&["\nExample generation unavailable: effective pointer unavailable.\n\n"])?;
        return Ok(());
    };
    if !kind.suffix().is_empty() {
        pointer.truncate(pointer.len().saturating_sub(kind.suffix().len()));
    }
    let example = examples.search(kind, pointer, json, &mut writer.budget.patterns);
    writer.row(&["\n", example.status, ".\n\n"])?;
    if let Some(value) = example.value {
        // The delimiter cannot be closed by any line within supplied JSON string content.
        let longest = value.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
        let fence = "`".repeat(3.max(longest + 1));
        writer.row(&[&fence, "json\n", &value, "\n", &fence, "\n\n"])?;
    }
    Ok(())
}
