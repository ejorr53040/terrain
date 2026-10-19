//! The Functionality view of a Workspace: each crate is a System holding its
//! Capabilities, its Unsorted items and its Libraries. Nothing here writes code.

use std::fs;
use std::io;
use std::path::Path;

use crate::{Id, MoveError, Workspace};

/// One crate of the workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct System {
    pub name: String,
    /// The crate's directory relative to the workspace root; "" for the root crate.
    pub dir: String,
    /// Other Systems this one depends on by path.
    pub depends_on: Vec<String>,
    /// The user's Capabilities in this System, in the order they were made.
    pub capabilities: Vec<Capability>,
    /// Top-level items of the System's files that are in no Capability, in file order.
    pub unsorted: Vec<Item>,
    /// This System's own copy of each Library it uses: visible Crate dependencies by
    /// name, then Outside systems in sidecar order.
    pub libraries: Vec<Library>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Capability {
    /// Which Capability this is, for `rename`, `assign` and friends.
    pub index: usize,
    pub name: String,
    pub items: Vec<Item>,
    /// Items it refers to that no longer exist; they stay counted until dismissed.
    pub missing: usize,
    /// Names of the System's Libraries its items use, in the System's Library order.
    pub uses: Vec<String>,
}

/// A top-level item of one of a System's files.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub file: String,
    /// "kind name", such as "fn boot".
    pub label: String,
    pub line: usize,
    /// The item's block, for its code.
    pub id: Id,
}

impl Item {
    fn reference(&self) -> ItemRef {
        ItemRef {
            file: self.file.clone(),
            label: self.label.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Library {
    pub name: String,
    pub kind: LibraryKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LibraryKind {
    Crate,
    Outside { icon: String },
}

/// Crates hidden unless pinned: everywhere, and not what a System is about.
const PLUMBING: &[&str] = &[
    "anyhow",
    "bytes",
    "log",
    "once_cell",
    "serde",
    "serde_json",
    "thiserror",
    "tracing",
    "tracing-subscriber",
];

/// A Capability as the sidecar stores it: items by file and "kind name" label.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SavedCapability {
    name: String,
    system: String,
    items: Vec<ItemRef>,
}

/// A top-level item of a file, as the sidecar refers to it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ItemRef {
    pub file: String,
    /// "kind name", such as "fn boot".
    pub label: String,
}

/// A Library the code drives at run time, declared in the sidecar.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct OutsideSystem {
    name: String,
    /// One of vm, network, database, tls, http, storage, process.
    icon: String,
    /// The files that drive it.
    files: Vec<String>,
}

/// A crate as its Cargo.toml describes it.
#[derive(Debug, Clone)]
pub(crate) struct Manifest {
    name: String,
    dir: String,
    /// Dependency names, and whether each is a path dependency.
    deps: Vec<(String, bool)>,
}

impl Workspace {
    /// The workspace's crates as Systems, sorted by name.
    pub fn systems(&self) -> Vec<System> {
        let mut systems: Vec<System> = self
            .manifests
            .iter()
            .map(|m| {
                let is_system = |dep: &str| self.manifests.iter().any(|o| o.name == dep);
                let mut libraries: Vec<Library> = m
                    .deps
                    .iter()
                    .filter(|(dep, path)| !(*path && is_system(dep)))
                    .filter(|(dep, _)| {
                        !PLUMBING.contains(&dep.as_str()) || self.sidecar.pinned.contains(dep)
                    })
                    .map(|(dep, _)| Library {
                        name: dep.clone(),
                        kind: LibraryKind::Crate,
                    })
                    .collect();
                libraries.sort_by(|a, b| a.name.cmp(&b.name));
                libraries.extend(
                    self.sidecar
                        .outside
                        .iter()
                        .filter(|o| {
                            o.files
                                .iter()
                                .any(|f| self.owner(f).is_some_and(|own| own.name == m.name))
                        })
                        .map(|o| Library {
                            name: o.name.clone(),
                            kind: LibraryKind::Outside {
                                icon: o.icon.clone(),
                            },
                        }),
                );
                let mut unsorted = self.items_of(&m.name);
                let mut capabilities = Vec::new();
                for (index, saved) in self.sidecar.capabilities.iter().enumerate() {
                    if saved.system != m.name {
                        continue;
                    }
                    let mut items = Vec::new();
                    for r in &saved.items {
                        // ponytail: two items with the same label in one file resolve to the first.
                        if let Some(i) = unsorted.iter().position(|it| it.reference() == *r) {
                            items.push(unsorted.remove(i));
                        }
                    }
                    let missing = saved.items.len() - items.len();
                    let uses = libraries
                        .iter()
                        .filter(|lib| items.iter().any(|it| self.item_uses(it, lib)))
                        .map(|lib| lib.name.clone())
                        .collect();
                    capabilities.push(Capability {
                        index,
                        name: saved.name.clone(),
                        items,
                        missing,
                        uses,
                    });
                }
                System {
                    capabilities,
                    unsorted,
                    name: m.name.clone(),
                    dir: m.dir.clone(),
                    depends_on: m
                        .deps
                        .iter()
                        .filter(|(dep, path)| *path && is_system(dep))
                        .map(|(dep, _)| dep.clone())
                        .collect(),
                    libraries,
                }
            })
            .collect();
        systems.sort_by(|a, b| a.name.cmp(&b.name));
        systems
    }

    /// Make an empty Capability in `system`; returns its index.
    pub fn new_capability(&mut self, system: &str, name: &str) -> Result<usize, MoveError> {
        if !self.manifests.iter().any(|m| m.name == system) {
            return Err(MoveError::Invalid("no such System"));
        }
        self.sidecar.capabilities.push(SavedCapability {
            name: name.to_string(),
            system: system.to_string(),
            items: Vec::new(),
        });
        self.save_sidecar()?;
        Ok(self.sidecar.capabilities.len() - 1)
    }

    /// Put two items of one System into a new Capability named after the first.
    pub fn group(&mut self, first: &ItemRef, second: &ItemRef) -> Result<usize, MoveError> {
        let system = self.system_of(first)?;
        if first == second || self.system_of(second)? != system {
            return Err(MoveError::Invalid("a Capability holds items of one System"));
        }
        let name = first
            .label
            .split_once(' ')
            .map_or(first.label.as_str(), |(_, n)| n)
            .to_string();
        let index = self.new_capability(&system, &name)?;
        self.assign(first, Some(index))?;
        self.assign(second, Some(index))?;
        Ok(index)
    }

    /// Move an item into a Capability of its System, or back to Unsorted (`None`).
    pub fn assign(&mut self, item: &ItemRef, to: Option<usize>) -> Result<(), MoveError> {
        let system = self.system_of(item)?;
        if let Some(c) = to {
            match self.sidecar.capabilities.get(c) {
                Some(cap) if cap.system == system => {}
                Some(_) => return Err(MoveError::Invalid("a Capability stays inside its System")),
                None => return Err(MoveError::Invalid("no such Capability")),
            }
        }
        for cap in &mut self.sidecar.capabilities {
            cap.items.retain(|r| r != item);
        }
        if let Some(c) = to {
            self.sidecar.capabilities[c].items.push(item.clone());
        }
        Ok(self.save_sidecar()?)
    }

    pub fn rename(&mut self, capability: usize, name: &str) -> Result<(), MoveError> {
        let cap = self
            .sidecar
            .capabilities
            .get_mut(capability)
            .ok_or(MoveError::Invalid("no such Capability"))?;
        cap.name = name.to_string();
        Ok(self.save_sidecar()?)
    }

    /// Remove a Capability; its items go back to Unsorted. Later indices shift down.
    pub fn remove_capability(&mut self, capability: usize) -> Result<(), MoveError> {
        if capability >= self.sidecar.capabilities.len() {
            return Err(MoveError::Invalid("no such Capability"));
        }
        self.sidecar.capabilities.remove(capability);
        Ok(self.save_sidecar()?)
    }

    /// Forget the items a Capability refers to that no longer exist.
    pub fn dismiss_missing(&mut self, capability: usize) -> Result<(), MoveError> {
        let system = self
            .sidecar
            .capabilities
            .get(capability)
            .ok_or(MoveError::Invalid("no such Capability"))?
            .system
            .clone();
        let present: Vec<ItemRef> = self.items_of(&system).iter().map(Item::reference).collect();
        self.sidecar.capabilities[capability]
            .items
            .retain(|r| present.contains(r));
        Ok(self.save_sidecar()?)
    }

    /// An item uses a Crate dependency it names as a path root (`tokio::spawn`), and an
    /// Outside system if it lives in one of the files declared to drive it.
    fn item_uses(&self, item: &Item, lib: &Library) -> bool {
        match lib.kind {
            LibraryKind::Crate => {
                let root = format!("{}::", lib.name.replace('-', "_"));
                let text = self.text(&item.id).unwrap_or_default();
                text.match_indices(&root).any(|(at, _)| {
                    !text[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ':')
                })
            }
            LibraryKind::Outside { .. } => self
                .sidecar
                .outside
                .iter()
                .any(|o| o.name == lib.name && o.files.contains(&item.file)),
        }
    }

    /// Top-level items of the System's parseable files, in file then source order.
    fn items_of(&self, system: &str) -> Vec<Item> {
        let mut items = Vec::new();
        for (f, file) in self.files.iter().enumerate() {
            if file.error || !self.owner(&file.name).is_some_and(|m| m.name == system) {
                continue;
            }
            for (c, b) in file.children.iter().enumerate() {
                items.push(Item {
                    file: file.name.clone(),
                    label: format!("{} {}", b.kind, b.name),
                    line: b.line,
                    id: Id(vec![f, c]),
                });
            }
        }
        items
    }

    /// The System an existing item belongs to.
    fn system_of(&self, item: &ItemRef) -> Result<String, MoveError> {
        let system = self
            .owner(&item.file)
            .ok_or(MoveError::Invalid("no such item"))?
            .name
            .clone();
        if self
            .items_of(&system)
            .iter()
            .any(|i| i.reference() == *item)
        {
            Ok(system)
        } else {
            Err(MoveError::Invalid("no such item"))
        }
    }

    /// Where a System sits on the canvas, if it has been placed.
    pub fn system_position(&self, system: &str) -> Option<[f32; 2]> {
        self.sidecar.systems.get(system).copied()
    }

    /// Put a System at `at` on the canvas and save the sidecar.
    pub fn place_system(&mut self, system: &str, at: [f32; 2]) -> io::Result<()> {
        self.sidecar.systems.insert(system.to_string(), at);
        self.save_sidecar()
    }

    /// Show (`true`) or hide again a plumbing crate, and save the sidecar.
    pub fn pin(&mut self, krate: &str, shown: bool) -> io::Result<()> {
        self.sidecar.pinned.retain(|p| p != krate);
        if shown {
            self.sidecar.pinned.push(krate.to_string());
        }
        self.save_sidecar()
    }

    /// The crate a file belongs to: the one with the deepest directory holding it.
    fn owner(&self, file: &str) -> Option<&Manifest> {
        self.manifests
            .iter()
            .filter(|m| m.dir.is_empty() || file.starts_with(&format!("{}/", m.dir)))
            .max_by_key(|m| m.dir.len())
    }
}

/// Every crate under `root`: the root package, if any, and the workspace members.
pub(crate) fn read_manifests(root: &Path) -> io::Result<Vec<Manifest>> {
    let Some(top) = read_toml(&root.join("Cargo.toml"))? else {
        return Ok(Vec::new());
    };
    let shared = top
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table());
    let mut dirs = Vec::new();
    if top.contains_key("package") {
        dirs.push(String::new());
    }
    let members = top
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array());
    for member in members.into_iter().flatten().filter_map(|m| m.as_str()) {
        // ponytail: only "dir/*" globs; other glob forms are taken literally.
        match member.strip_suffix("/*") {
            Some(parent) => {
                let mut found: Vec<String> = fs::read_dir(root.join(parent))?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().join("Cargo.toml").is_file())
                    .map(|e| format!("{parent}/{}", e.file_name().to_string_lossy()))
                    .collect();
                found.sort();
                dirs.extend(found);
            }
            None => dirs.push(member.to_string()),
        }
    }

    let mut manifests = Vec::new();
    for dir in dirs {
        let toml = if dir.is_empty() {
            top.clone()
        } else {
            let Some(t) = read_toml(&root.join(&dir).join("Cargo.toml"))? else {
                continue;
            };
            t
        };
        let Some(name) = toml
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        else {
            continue;
        };
        let deps = toml
            .get("dependencies")
            .and_then(|d| d.as_table())
            .into_iter()
            .flatten()
            .map(|(dep, spec)| {
                let inherited = spec.get("workspace").and_then(|w| w.as_bool()) == Some(true);
                let spec = if inherited {
                    shared.and_then(|s| s.get(dep)).unwrap_or(spec)
                } else {
                    spec
                };
                (dep.clone(), spec.get("path").is_some())
            })
            .collect();
        manifests.push(Manifest {
            name: name.to_string(),
            dir,
            deps,
        });
    }
    Ok(manifests)
}

fn read_toml(path: &Path) -> io::Result<Option<toml::Table>> {
    match fs::read_to_string(path) {
        Ok(text) => text.parse::<toml::Table>().map(Some).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
