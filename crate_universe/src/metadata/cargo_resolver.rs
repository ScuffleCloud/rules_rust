//! Tools for producing Crate metadata

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use cargo_metadata::{Package, TargetKind};
use cargo_platform::Platform;
use cfg_expr::expr::TargetMatcher;
use cfg_expr::Predicate;
use itertools::Itertools;

use crate::config::CrateId;
use crate::utils::target_triple::TargetTriple;

/// A list platform triples that support host tools
///
/// [Tier 1](https://doc.rust-lang.org/nightly/rustc/platform-support.html#tier-1-with-host-tools)
/// [Tier 2](https://doc.rust-lang.org/nightly/rustc/platform-support.html#tier-2-with-host-tools)
const RUSTC_TRIPLES_WITH_HOST_TOOLS: [&str; 26] = [
    // Tier 1
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "i686-pc-windows-gnu",
    "i686-pc-windows-msvc",
    "i686-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    // Tier 2
    "aarch64-pc-windows-msvc",
    "aarch64-unknown-linux-musl",
    "arm-unknown-linux-gnueabi",
    "arm-unknown-linux-gnueabihf",
    "armv7-unknown-linux-gnueabihf",
    "loongarch64-unknown-linux-gnu",
    "loongarch64-unknown-linux-musl",
    "powerpc-unknown-linux-gnu",
    "powerpc64-unknown-linux-gnu",
    "powerpc64le-unknown-linux-gnu",
    "riscv64gc-unknown-linux-gnu",
    "riscv64gc-unknown-linux-musl",
    "s390x-unknown-linux-gnu",
    "x86_64-unknown-freebsd",
    "x86_64-unknown-illumo",
    "x86_64-unknown-linux-musl",
    "x86_64-unknown-netbsd",
];

#[derive(Debug)]
struct ResolveDep<'a> {
    id: &'a cargo_metadata::PackageId,
    // The name in the Cargo.toml
    // it may be renamed in which case
    // this is the new name.
    name: &'a str,
    is_alias: bool,
    features: &'a [String],
    optional: bool,
    use_default_featues: bool,
    target: Option<&'a Platform>,
    kind: DependencyKind,
}

struct PackageWithDeps<'a> {
    package: &'a Package,
    flattened_features: BTreeMap<&'a str, BTreeSet<&'a str>>,
    is_proc_macro: bool,
    library_target_name: Option<&'a str>,
    deps: Vec<ResolveDep<'a>>,
}

#[derive(Hash, Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Debug)]
enum DependencyKind {
    /// The 'normal' kind
    Normal = 0,
    /// Those used in tests only
    Development = 1,
    /// Those used in build scripts only
    Build = 2,
}

impl From<cargo_metadata::DependencyKind> for DependencyKind {
    fn from(value: cargo_metadata::DependencyKind) -> Self {
        match value {
            cargo_metadata::DependencyKind::Build => DependencyKind::Build,
            cargo_metadata::DependencyKind::Development => DependencyKind::Development,
            cargo_metadata::DependencyKind::Normal => DependencyKind::Normal,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
struct DepBorrow<'a> {
    platforms: BTreeSet<&'a Platform>,
    features: BTreeSet<&'a str>,
    aliases_optional: BTreeSet<(Option<&'a str>, bool)>,
}

#[derive(Debug, Default)]
struct ResolvedPackageBorrow<'a> {
    features: BTreeSet<&'a str>,
    deps: BTreeMap<
        (
            &'a cargo_metadata::PackageId,
            &'a TargetTriple,
            DependencyKind,
        ),
        DepBorrow<'a>,
    >,
}

pub struct CargoResolver<'a> {
    workspace_members: BTreeSet<&'a cargo_metadata::PackageId>,
    dependency_resolve: BTreeMap<&'a cargo_metadata::PackageId, PackageWithDeps<'a>>,
}

#[derive(Hash, Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Debug)]
enum FeatureDep<'a> {
    Feature { name: &'a str, optional: bool },
    Enabled,
}

type ResolvedPackageMap<'a> =
    BTreeMap<(&'a cargo_metadata::PackageId, &'a TargetTriple), RefCell<ResolvedPackageBorrow<'a>>>;

/// A representation of a crate dependency
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
pub(crate) struct Dependency {
    /// The PackageId of the target
    pub(crate) id: CrateId,

    /// The library target name of the dependency.
    pub(crate) target_name: Option<String>,

    /// The alias for the dependency from the perspective of the current package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) alias: Option<String>,

    /// Features enabled for this dependency.
    pub(crate) features: BTreeSet<String>,

    /// If this dep is optional
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) optional: bool,

    /// A set of platfoms that this dependency is for.
    pub(crate) platforms: BTreeSet<Platform>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct CrateAnnotation {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub features: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub deps: BTreeSet<Dependency>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub deps_dev: BTreeSet<Dependency>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub proc_macro_deps: BTreeSet<Dependency>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub proc_macro_deps_dev: BTreeSet<Dependency>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub build_deps: BTreeSet<Dependency>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub build_proc_macro_deps: BTreeSet<Dependency>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub build_link_deps: BTreeSet<Dependency>,
}

impl<'a> CargoResolver<'a> {
    pub fn new(metadata: &'a cargo_metadata::Metadata) -> Self {
        let mut packages_by_name = metadata
            .packages
            .iter()
            .into_group_map_by(|package| package.name.as_str());

        // Ensure that packages are sorted lowest version -> biggest version
        for packages in packages_by_name.values_mut() {
            packages.sort_by_key(|p| &p.version);
        }

        let workspace_members_pkg_id: BTreeSet<_> = metadata.workspace_members.iter().collect();
        let mut workspace_members = BTreeSet::new();

        // Now we resolve!
        let dependency_resolve: BTreeMap<_, _> = metadata
            .packages
            .iter()
            .map(|package| {
                let is_workspace_member = workspace_members_pkg_id.contains(&package.id);
                if is_workspace_member {
                    workspace_members.insert(&package.id);
                }

                (
                    &package.id,
                    PackageWithDeps {
                        deps: package
                            .dependencies
                            .iter()
                            .filter_map(|dep| {
                                // For non-workspace members we dont care for their development dependencies.
                                if dep.kind == cargo_metadata::DependencyKind::Development
                                    && !is_workspace_member
                                {
                                    return None;
                                }

                                // If the package doesnt exist or no version matched it means cargo
                                // determined that this package would never be enabled on any
                                // feature / target combination.
                                let matched_pkg =
                                    packages_by_name.get(dep.name.as_str())?.iter().rev().find(
                                        |pkg| {
                                            if dep.req.matches(&pkg.version)
                                                || (dep.req.comparators.is_empty()
                                                    && dep
                                                        .source
                                                        .as_ref()
                                                        .is_some_and(|s| s.starts_with("git+")))
                                            {
                                                true
                                            } else {
                                                false
                                            }
                                        },
                                    )?;

                                Some(ResolveDep {
                                    id: &matched_pkg.id,
                                    name: dep.rename.as_deref().unwrap_or(&dep.name),
                                    is_alias: dep.rename.is_some(),
                                    kind: dep.kind.into(),
                                    target: dep.target.as_ref(),
                                    features: &dep.features,
                                    optional: dep.optional,
                                    use_default_featues: dep.uses_default_features,
                                })
                            })
                            .collect(),
                        is_proc_macro: package
                            .targets
                            .iter()
                            .flat_map(|target| target.kind.iter())
                            .any(|kind| matches!(kind, TargetKind::ProcMacro)),
                        flattened_features: package
                            .features
                            .iter()
                            .map(|(feature, features)| {
                                let mut flat = BTreeSet::from_iter([feature.as_str()]);

                                let mut stack = Vec::from_iter(features);
                                while let Some(feat) = stack.pop() {
                                    let feat = match feat.split_once('/') {
                                        Some((feat, _)) if !feat.ends_with('?') => feat,
                                        None => feat.as_str(),
                                        _ => continue,
                                    };

                                    let Some(features) = package.features.get(feat) else {
                                        continue;
                                    };

                                    if flat.insert(feat) {
                                        stack.extend(features);
                                    }
                                }

                                (feature.as_str(), flat)
                            })
                            .collect(),
                        library_target_name: package
                            .targets
                            .iter()
                            .find(|target| {
                                target.kind.iter().any(|kind| {
                                    matches!(
                                        kind,
                                        TargetKind::CDyLib
                                            | TargetKind::DyLib
                                            | TargetKind::Lib
                                            | TargetKind::ProcMacro
                                            | TargetKind::RLib
                                            | TargetKind::StaticLib
                                    )
                                })
                            })
                            .map(|t| t.name.as_str()),
                        package,
                    },
                )
            })
            .collect();

        Self {
            dependency_resolve,
            workspace_members,
        }
    }

    pub fn execute(
        &self,
        target_triples: impl IntoIterator<Item = impl Borrow<TargetTriple>>,
    ) -> BTreeMap<CrateId, BTreeMap<TargetTriple, CrateAnnotation>> {
        let mut data = BTreeMap::default();

        let target_triples: Vec<_> = target_triples
            .into_iter()
            .map(|triple| triple.borrow().clone())
            .collect();

        let host_triples: Vec<_> = target_triples
            .iter()
            // Only query triples for platforms that have host tools.
            .filter(|host_triple| {
                RUSTC_TRIPLES_WITH_HOST_TOOLS.contains(&host_triple.to_cargo().as_str())
            })
            .cloned()
            .collect();

        // We only want to spawn processes for unique cargo platforms
        for host in &host_triples {
            for target in &target_triples {
                self.resolve(host, target, &mut data);
            }
        }

        data
    }

    fn resolve(
        &self,
        host: &TargetTriple,
        target: &TargetTriple,
        data: &mut BTreeMap<CrateId, BTreeMap<TargetTriple, CrateAnnotation>>,
    ) {
        let host_flags = cfg_expr::targets::get_builtin_target_by_triple(&host.to_cargo()).unwrap();
        let target_flags =
            cfg_expr::targets::get_builtin_target_by_triple(&target.to_cargo()).unwrap();

        let mut resolved = ResolvedPackageMap::new();

        let mut stack: Vec<_> = self
            .workspace_members
            .iter()
            .map(|id| {
                (
                    *id,
                    target,
                    self.dependency_resolve[id]
                        .package
                        .features
                        .keys()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        while let Some((id, location, features)) = stack.pop() {
            let PackageWithDeps {
                deps,
                flattened_features,
                package,
                ..
            } = &self.dependency_resolve[&id];

            let flattened_features: BTreeSet<_> = features
                .iter()
                .flat_map(|feature| flattened_features.get(feature))
                .flatten()
                .copied()
                .collect();

            let mut any_changed = !resolved.contains_key(&(id, location));
            let new_pkg = resolved.entry((id, location)).or_default();

            for feature in &flattened_features {
                any_changed |= new_pkg.get_mut().features.insert(*feature);
            }

            if !any_changed {
                continue;
            }

            let enabled_deps = flattened_features
                .iter()
                .copied()
                .flat_map(|f| package.features.get(f))
                .flatten()
                .filter_map(|feature| {
                    if let Some(dep) = feature.strip_prefix("dep:") {
                        Some((dep, FeatureDep::Enabled))
                    } else if let Some((dep, feature)) = feature.split_once('/') {
                        let (dep, optional) = if let Some(dep) = dep.strip_suffix('?') {
                            (dep, true)
                        } else {
                            (dep, false)
                        };

                        Some((
                            dep,
                            FeatureDep::Feature {
                                name: feature,
                                optional,
                            },
                        ))
                    } else {
                        None
                    }
                })
                .into_grouping_map()
                .collect::<BTreeSet<_>>();

            let activated_deps = deps.iter().filter_map(|dep| {
                let dep_location = if location == target
                    && (matches!(dep.kind, DependencyKind::Build)
                        || self.dependency_resolve[&dep.id].is_proc_macro)
                {
                    host
                } else {
                    location
                };

                if let Some(cfg_expr) = dep.target {
                    let location_flags = if dep_location == host {
                        host_flags
                    } else {
                        target_flags
                    };

                    if !match cfg_expr {
                        Platform::Cfg(cfg) => cfg_expr::Expression::parse(&cfg.to_string())
                            .unwrap()
                            .eval(|pred| match pred {
                                Predicate::Target(tp) => location_flags.matches(tp),
                                _ => false,
                            }),
                        Platform::Name(name) => {
                            location_flags.triple.as_str().eq_ignore_ascii_case(name)
                        }
                    } {
                        return None;
                    }
                }

                if dep.optional {
                    if enabled_deps.get(dep.name).is_none_or(|options| {
                        !options.iter().any(|o| {
                            matches!(
                                o,
                                FeatureDep::Enabled
                                    | FeatureDep::Feature {
                                        optional: false,
                                        ..
                                    }
                            )
                        })
                    }) {
                        return None;
                    }
                }

                Some((dep, dep_location))
            });

            for (dep, dep_location) in activated_deps {
                let resolved_package = resolved.get(&(id, location)).unwrap();
                let mut resolved_package = resolved_package.borrow_mut();
                let should_default = dep.use_default_featues
                    && self.dependency_resolve[&dep.id]
                        .package
                        .features
                        .contains_key("default");
                let resolved_dep = resolved_package
                    .deps
                    .entry((dep.id, dep_location, dep.kind))
                    .or_default();

                resolved_dep.platforms.extend(dep.target);

                resolved_dep
                    .aliases_optional
                    .insert((dep.is_alias.then_some(dep.name), dep.optional));

                if should_default {
                    resolved_dep.features.insert("default");
                }

                resolved_dep.features.extend(
                    dep.features.iter().map(|f| f.as_str()).chain(
                        enabled_deps
                            .get(dep.name)
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter_map(|f| match f {
                                FeatureDep::Feature { name, .. } => Some(name),
                                _ => None,
                            }),
                    ),
                );

                if resolved.get(&(dep.id, dep_location)).is_none_or(|pkg| {
                    resolved_dep
                        .features
                        .iter()
                        .any(|feat| !pkg.borrow().features.contains(feat))
                }) {
                    stack.push((
                        dep.id,
                        dep_location,
                        resolved_dep.features.iter().copied().collect(),
                    ))
                }
            }
        }

        for ((id, location), package) in &resolved {
            let package = package.borrow();
            let pkg = self.dependency_resolve[id].package;
            let annotation = data
                .entry(CrateId::from(pkg))
                .or_default()
                .entry((*location).clone())
                .or_default();

            if self.workspace_members.contains(id) {
                resolved.iter().for_each(|((pkg_id, location), pkg)| {
                    if *location != target || pkg_id == id {
                        return;
                    }

                    let pkg = pkg.borrow();

                    for feature in pkg
                        .deps
                        .range(
                            (*id, target, DependencyKind::Normal)
                                ..=(*id, target, DependencyKind::Build),
                        )
                        .flat_map(|f| f.1.features.iter())
                    {
                        annotation.features.insert(feature.to_string());
                    }
                });
            } else {
                package.features.iter().for_each(|feat| {
                    annotation.features.insert(feat.to_string());
                });
            }

            for ((dep_id, _, kind), dep) in &package.deps {
                let dep_pkg = &self.dependency_resolve[dep_id];
                for (alias, optional) in &dep.aliases_optional {
                    let dependency = Dependency {
                        features: dep.features.iter().map(|f| f.to_string()).collect(),
                        alias: alias.map(|a| a.replace("-", "_")),
                        id: CrateId::from(dep_pkg.package),
                        target_name: dep_pkg.library_target_name.map(|t| t.to_string()),
                        optional: *optional,
                        platforms: dep.platforms.iter().copied().cloned().collect(),
                    };

                    if *kind == DependencyKind::Normal
                        && !dep_pkg.is_proc_macro
                        && dep_pkg.package.links.is_some()
                    {
                        annotation.build_link_deps.insert(dependency.clone());
                    }

                    match (kind, dep_pkg.is_proc_macro) {
                        (DependencyKind::Normal, false) => &mut annotation.deps,
                        (DependencyKind::Development, false) => &mut annotation.deps_dev,
                        (DependencyKind::Build, false) => &mut annotation.build_deps,
                        (DependencyKind::Normal, true) => &mut annotation.proc_macro_deps,
                        (DependencyKind::Development, true) => &mut annotation.proc_macro_deps_dev,
                        (DependencyKind::Build, true) => &mut annotation.build_proc_macro_deps,
                    }
                    .insert(dependency);
                }
            }
        }
    }
}
