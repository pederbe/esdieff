//! Immutable Requested assessment policy shared by execution and report views.
//! Follows ADR 0009: Bounded processing and reporting, ADR 0011: Machine-readable
//! report contracts, and ADR 0013: Shared workflow and report guarantees.
use crate::Assessment;

pub(crate) const MODIFIED_AREA: &str = "modified-date-time/1";

#[derive(Clone, Copy)]
pub(crate) enum StatusSource {
    Initial(usize),
    Advisory,
    Modified,
    Units(usize),
    ContentFormat(usize),
    Pattern,
    Numeric,
    Consistency(usize),
    Requirements,
}

pub(crate) struct Area {
    pub(crate) id: &'static str,
    pub(crate) status: StatusSource,
    // Expanded producers already supply their own precise unfinished conditions.
    pub(crate) unfinished_condition: bool,
}
impl Area {
    const fn new(id: &'static str, status: StatusSource, unfinished_condition: bool) -> Self {
        Self {
            id,
            status,
            unfinished_condition,
        }
    }
}

const INITIAL_AREAS: [Area; 8] = [
    Area::new("document-envelope/1", StatusSource::Initial(0), true),
    Area::new("base-roles-names/1", StatusSource::Initial(1), true),
    Area::new("metadata-namespaces/1", StatusSource::Initial(2), true),
    Area::new("established-composition/1", StatusSource::Initial(3), true),
    Area::new("basic-parameters/1", StatusSource::Initial(4), true),
    Area::new(
        "alternatives-requiredness/1",
        StatusSource::Initial(5),
        true,
    ),
    Area::new("extensions-policies/1", StatusSource::Initial(6), true),
    Area::new(crate::proof::AREA, StatusSource::Advisory, true),
];

const FULL_AREAS: [Area; 13] = {
    use crate::inventory::FULL_AREAS as IDS;
    use StatusSource::Requirements;
    [
        Area::new(IDS[0], Requirements, true),
        Area::new(IDS[1], Requirements, true),
        Area::new(IDS[2], Requirements, true),
        Area::new(IDS[3], Requirements, true),
        Area::new(IDS[4], Requirements, true),
        Area::new(IDS[5], Requirements, true),
        Area::new(IDS[6], Requirements, true),
        Area::new(IDS[7], Requirements, true),
        Area::new(IDS[8], Requirements, true),
        Area::new(IDS[9], Requirements, true),
        Area::new(IDS[10], Requirements, true),
        Area::new(IDS[11], Requirements, true),
        Area::new(IDS[12], StatusSource::Advisory, true),
    ]
};

pub(crate) struct Capabilities {
    pub(crate) modified: bool,
    pub(crate) units: bool,
    pub(crate) content_format: bool,
    pub(crate) pattern: bool,
    pub(crate) numeric: bool,
    pub(crate) consistency: bool,
    pub(crate) full: bool,
}

struct Reserves {
    registry: usize,
    pattern: usize,
    expanded: usize,
}

pub(crate) struct Profile {
    pub(crate) identity: &'static str,
    pub(crate) inventory: &'static str,
    areas: &'static [Area],
    additions: &'static [Area],
    pub(crate) enabled: Capabilities,
    reserves: Reserves,
}
impl Profile {
    pub(crate) fn areas(&self) -> impl Iterator<Item = &'static Area> + Clone {
        self.areas.iter().chain(self.additions)
    }
    pub(crate) fn area(&self, id: &str) -> Option<&'static Area> {
        self.areas().find(|area| area.id == id)
    }
    pub(crate) fn area_count(&self) -> usize {
        self.areas.len() + self.additions.len()
    }
    pub(crate) fn registry_requested(&self) -> bool {
        self.enabled.units || self.enabled.content_format
    }
    pub(crate) fn report_reserve(&self) -> usize {
        self.reserves.registry + self.reserves.pattern + self.reserves.expanded
    }
    pub(crate) fn admit_reporting(
        &self,
        reserve: usize,
        report_bytes: usize,
        registry_policy: bool,
    ) -> Result<(), &'static str> {
        if registry_policy && !self.registry_requested() {
            return Err("recorded registry policy requires a registry assessment");
        }
        if self.registry_requested() {
            crate::registry::qualify()?;
        }
        let registry_envelope = reserve.checked_add(self.reserves.registry);
        if registry_envelope.is_none_or(|n| n > report_bytes) {
            return Err("report cannot reserve the registry assessment envelope");
        }
        if registry_envelope
            .and_then(|n| n.checked_add(self.reserves.pattern))
            .is_none_or(|n| n > report_bytes)
        {
            return Err("report cannot reserve the pattern assessment envelope");
        }
        if self.reserves.expanded != 0 && reserve + self.reserves.expanded > report_bytes {
            return Err("report cannot reserve the expanded assessment envelope");
        }
        Ok(())
    }
}

const INITIAL: Profile = Profile {
    identity: "esdieff.initial/1",
    inventory: "esdieff.initial/1",
    areas: &INITIAL_AREAS,
    additions: &[],
    enabled: Capabilities {
        modified: false,
        units: false,
        content_format: false,
        pattern: false,
        numeric: false,
        consistency: false,
        full: false,
    },
    reserves: Reserves {
        registry: 0,
        pattern: 0,
        expanded: 0,
    },
};

pub(crate) const fn profile(assessment: Assessment) -> &'static Profile {
    match assessment {
        Assessment::Initial => &INITIAL,
        Assessment::ExpandedModified => {
            const {
                &Profile {
                    identity: "esdieff.expanded.modified/1",
                    inventory: "esdieff.expanded.modified/1",
                    additions: &[Area::new(MODIFIED_AREA, StatusSource::Modified, false)],
                    enabled: Capabilities {
                        modified: true,
                        ..INITIAL.enabled
                    },
                    reserves: Reserves {
                        expanded: 1024,
                        ..INITIAL.reserves
                    },
                    ..INITIAL
                }
            }
        }
        Assessment::ExpandedUnits => {
            const {
                &Profile {
                    identity: "esdieff.expanded.units/1",
                    inventory: "esdieff.expanded.units/1",
                    additions: &[
                        Area::new(crate::units::AREAS[0], StatusSource::Units(0), false),
                        Area::new(crate::units::AREAS[1], StatusSource::Units(1), false),
                    ],
                    enabled: Capabilities {
                        units: true,
                        ..INITIAL.enabled
                    },
                    reserves: Reserves {
                        registry: crate::registry::UNIT_RESERVE,
                        ..INITIAL.reserves
                    },
                    ..INITIAL
                }
            }
        }
        Assessment::ExpandedContentFormat => {
            const {
                &Profile {
                    identity: "esdieff.expanded.content-format/1",
                    inventory: "esdieff.expanded.content-format/1",
                    additions: &[
                        Area::new(
                            crate::content_format::AREAS[0],
                            StatusSource::ContentFormat(0),
                            false,
                        ),
                        Area::new(
                            crate::content_format::AREAS[1],
                            StatusSource::ContentFormat(1),
                            false,
                        ),
                    ],
                    enabled: Capabilities {
                        content_format: true,
                        ..INITIAL.enabled
                    },
                    reserves: Reserves {
                        registry: crate::registry::CONTENT_RESERVE,
                        ..INITIAL.reserves
                    },
                    ..INITIAL
                }
            }
        }
        Assessment::ExpandedPatternSyntax => {
            const {
                &Profile {
                    identity: "esdieff.expanded.pattern-syntax/1",
                    inventory: "esdieff.expanded.pattern-syntax/1",
                    additions: &[Area::new(
                        crate::pattern::AREA,
                        StatusSource::Pattern,
                        false,
                    )],
                    enabled: Capabilities {
                        pattern: true,
                        ..INITIAL.enabled
                    },
                    reserves: Reserves {
                        pattern: crate::pattern::REPORT_RESERVE,
                        ..INITIAL.reserves
                    },
                    ..INITIAL
                }
            }
        }
        Assessment::ExpandedNumeric => {
            const {
                &Profile {
                    identity: "esdieff.expanded.numeric/1",
                    inventory: "esdieff.expanded.numeric/1",
                    additions: &[Area::new(
                        crate::proof::NUMERIC_AREA,
                        StatusSource::Numeric,
                        true,
                    )],
                    enabled: Capabilities {
                        numeric: true,
                        ..INITIAL.enabled
                    },
                    ..INITIAL
                }
            }
        }
        Assessment::ExpandedConsistency => {
            const {
                &Profile {
                    identity: "esdieff.expanded.consistency/1",
                    inventory: "esdieff.expanded.consistency/1",
                    additions: &[
                        Area::new(
                            crate::sample::CONSISTENCY_AREAS[0],
                            StatusSource::Consistency(0),
                            true,
                        ),
                        Area::new(
                            crate::sample::CONSISTENCY_AREAS[1],
                            StatusSource::Consistency(1),
                            true,
                        ),
                    ],
                    enabled: Capabilities {
                        consistency: true,
                        ..INITIAL.enabled
                    },
                    ..INITIAL
                }
            }
        }
        Assessment::FullBaseline => &Profile {
            identity: "rfc9880/base-1",
            inventory: crate::inventory::FULL_INVENTORY,
            areas: &FULL_AREAS,
            additions: &[],
            enabled: Capabilities {
                modified: true,
                units: true,
                content_format: true,
                pattern: true,
                full: true,
                // Full model requirements do not request independent expanded advice.
                numeric: false,
                consistency: false,
            },
            // Full has its own reserve, independent of summed expanded profiles.
            reserves: Reserves {
                registry: crate::registry::FULL_RESERVE,
                pattern: crate::pattern::REPORT_RESERVE,
                expanded: 0,
            },
        },
    }
}
