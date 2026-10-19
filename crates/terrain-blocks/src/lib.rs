//! terrain-blocks: a Rust crate's structure as blocks. The files on disk are the
//! source of truth (ADR 0007); a Workspace is a parsed view of them.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser};

mod systems;
pub use systems::{Capability, Item, ItemRef, Library, LibraryKind, System};

/// A file, container or item, with its stack of blocks inside.
#[derive(Debug, Clone)]
pub struct Block {
    /// "file", "fn", "struct", "enum", "trait", "impl", "mod", "const", "static" or "type".
    pub kind: &'static str,
    pub name: String,
    /// 1-based line where the block starts, leading comments and attributes included.
    pub line: usize,
    pub children: Vec<Block>,
    /// The file has syntax errors: shown empty, never written.
    pub error: bool,
    /// Bytes of the block's whole lines in its file, leading comments included.
    span: Range<usize>,
}

/// Where a block is: its file's index, then its index in each stack down to it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(pub Vec<usize>);

#[derive(Debug)]
pub enum MoveError {
    /// The move makes no sense, such as a block into itself.
    Invalid(&'static str),
    /// These files changed on disk since they were last read; nothing was written.
    Stale(Vec<String>),
    Io(io::Error),
}

impl From<io::Error> for MoveError {
    fn from(e: io::Error) -> Self {
        MoveError::Io(e)
    }
}

pub struct Workspace {
    root: PathBuf,
    files: Vec<Block>,
    /// Text of each file as last read or written, parallel to `files`.
    texts: Vec<String>,
    /// Per move, the earlier text of each file it touched, by name (refresh renumbers files).
    undo: Vec<Vec<(String, String)>>,
    /// The user's view of the crate, saved in the sidecar.
    sidecar: Sidecar,
    /// Every crate in the workspace, re-read on refresh.
    manifests: Vec<systems::Manifest>,
}

const SIDECAR: &str = ".terrain-blocks.ron";

/// What the sidecar holds: the user's view of the crate, never code (ADR 0007).
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Sidecar {
    /// Canvas position of each placed file block, by file name.
    files: BTreeMap<String, [f32; 2]>,
    /// Canvas position of each placed System, by crate name.
    systems: BTreeMap<String, [f32; 2]>,
    capabilities: Vec<systems::SavedCapability>,
    outside: Vec<systems::OutsideSystem>,
    /// Plumbing crates the user wants shown anyway.
    pinned: Vec<String>,
}

impl Workspace {
    pub fn open(root: &Path) -> io::Result<Workspace> {
        let sidecar = match fs::read_to_string(root.join(SIDECAR)) {
            Ok(text) => {
                ron::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Sidecar::default(),
            Err(e) => return Err(e),
        };
        let mut ws = Workspace {
            root: root.to_path_buf(),
            files: Vec::new(),
            texts: Vec::new(),
            undo: Vec::new(),
            sidecar,
            manifests: Vec::new(),
        };
        ws.refresh()?;
        Ok(ws)
    }

    /// Where a file block sits on the canvas, if it has been placed.
    pub fn position(&self, file: &Id) -> Option<[f32; 2]> {
        self.sidecar
            .files
            .get(&self.files.get(*file.0.first()?)?.name)
            .copied()
    }

    /// Put a file block at `at` on the canvas and save the sidecar.
    pub fn place(&mut self, file: &Id, at: [f32; 2]) -> io::Result<()> {
        let name = self.files[file.0[0]].name.clone();
        self.sidecar.files.insert(name, at);
        self.save_sidecar()
    }

    fn save_sidecar(&self) -> io::Result<()> {
        let text = ron::ser::to_string_pretty(&self.sidecar, Default::default())
            .map_err(io::Error::other)?;
        fs::write(self.root.join(SIDECAR), text)
    }

    /// Re-read the crate from disk. Returns the names of files added, changed or removed.
    pub fn refresh(&mut self) -> io::Result<Vec<String>> {
        let mut paths = Vec::new();
        rust_files(&self.root, &mut paths)?;
        paths.sort();
        let mut changed = Vec::new();
        let (mut files, mut texts) = (Vec::new(), Vec::new());
        for path in paths {
            let text = fs::read_to_string(&path)?;
            let name = path
                .strip_prefix(&self.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            match self.files.iter().position(|f| f.name == name) {
                Some(i) if self.texts[i] == text => files.push(self.files[i].clone()),
                _ => {
                    files.push(parse_file(name.clone(), &text));
                    changed.push(name);
                }
            }
            texts.push(text);
        }
        for old in &self.files {
            if !files.iter().any(|f| f.name == old.name) {
                changed.push(old.name.clone());
            }
        }
        changed.sort();
        (self.files, self.texts) = (files, texts);
        self.manifests = systems::read_manifests(&self.root)?;
        Ok(changed)
    }

    /// The block at a path of labels: a file name, then "kind name" down its stacks.
    pub fn find(&self, path: &[&str]) -> Option<Id> {
        let (file, rest) = path.split_first()?;
        let mut id = vec![self.files.iter().position(|f| f.name == *file)?];
        let mut stack = &self.files[id[0]].children;
        for label in rest {
            let i = stack
                .iter()
                .position(|b| format!("{} {}", b.kind, b.name) == *label)?;
            id.push(i);
            stack = &stack[i].children;
        }
        Some(Id(id))
    }

    pub fn block(&self, id: &Id) -> Option<&Block> {
        let (file, rest) = id.0.split_first()?;
        rest.iter()
            .try_fold(self.files.get(*file)?, |b, &i| b.children.get(i))
    }

    /// The block's source text, leading comments included.
    pub fn text(&self, id: &Id) -> Option<&str> {
        Some(&self.texts[*id.0.first()?][self.block(id)?.span.clone()])
    }

    /// Move `block` with its comments and children into `into`'s stack, before the
    /// block now at `index` (or last, if `index` is the stack's length). Writes the files.
    pub fn move_block(&mut self, block: &Id, into: &Id, index: usize) -> Result<(), MoveError> {
        if block.0.len() < 2 {
            return Err(MoveError::Invalid("file blocks only move on the canvas"));
        }
        if into.0.starts_with(&block.0) {
            return Err(MoveError::Invalid("a block can't move into itself"));
        }
        let moving = self
            .block(block)
            .ok_or(MoveError::Invalid("no such block"))?
            .clone();
        let target = self
            .block(into)
            .ok_or(MoveError::Invalid("no such container"))?;
        let from = block.0[0];
        let to = into.0[0];
        if self.files[from].error || self.files[to].error {
            return Err(MoveError::Invalid(
                "a file with syntax errors is never written",
            ));
        }
        let touched = if from == to {
            vec![from]
        } else {
            vec![from, to]
        };
        self.check_fresh(&touched)?;

        let src = &self.texts[from];
        let cut = with_one_blank_line(src, moving.span.clone());
        let old_indent = indent(&src[moving.span.clone()]);
        let dst = &self.texts[to];
        let (at, text) = match (target.children.get(index), target.children.last()) {
            (Some(next), _) => {
                let ind = indent(&dst[next.span.clone()]);
                (
                    next.span.start,
                    reindent(&src[moving.span.clone()], old_indent, ind) + "\n",
                )
            }
            (None, Some(last)) if index == target.children.len() => {
                let ind = indent(&dst[last.span.clone()]);
                (
                    last.span.end,
                    "\n".to_string() + &reindent(&src[moving.span.clone()], old_indent, ind),
                )
            }
            // ponytail: empty containers aren't drop targets yet.
            _ => return Err(MoveError::Invalid("no such place in the stack")),
        };

        self.undo.push(
            touched
                .iter()
                .map(|&f| (self.files[f].name.clone(), self.texts[f].clone()))
                .collect(),
        );
        let mut new_from = self.texts[from].clone();
        if from == to {
            // Both edits index the old text, so apply the later one first.
            if at >= cut.end {
                new_from.insert_str(at, &text);
                new_from.replace_range(cut, "");
            } else {
                new_from.replace_range(cut, "");
                new_from.insert_str(at, &text);
            }
            self.write(from, new_from)?;
        } else {
            new_from.replace_range(cut, "");
            let mut new_to = self.texts[to].clone();
            new_to.insert_str(at, &text);
            self.write(from, new_from)?;
            self.write(to, new_to)?;
        }
        Ok(())
    }

    /// Put back the files as they were before the last move. False if there was none.
    pub fn undo(&mut self) -> Result<bool, MoveError> {
        let Some(earlier) = self.undo.last() else {
            return Ok(false);
        };
        let mut files = Vec::new();
        for (name, _) in earlier {
            match self.files.iter().position(|f| f.name == *name) {
                Some(i) => files.push(i),
                None => return Err(MoveError::Stale(vec![name.clone()])),
            }
        }
        self.check_fresh(&files)?;
        for (file, (_, text)) in files.into_iter().zip(self.undo.pop().unwrap()) {
            self.write(file, text)?;
        }
        Ok(true)
    }

    fn check_fresh(&self, files: &[usize]) -> Result<(), MoveError> {
        let mut stale = Vec::new();
        for &file in files {
            if fs::read_to_string(self.root.join(&self.files[file].name))? != self.texts[file] {
                stale.push(self.files[file].name.clone());
            }
        }
        if stale.is_empty() {
            Ok(())
        } else {
            Err(MoveError::Stale(stale))
        }
    }

    fn write(&mut self, file: usize, text: String) -> io::Result<()> {
        fs::write(self.root.join(&self.files[file].name), &text)?;
        self.files[file] = parse_file(self.files[file].name.clone(), &text);
        self.texts[file] = text;
        Ok(())
    }

    /// The crate directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// File blocks, sorted by path.
    pub fn files(&self) -> &[Block] {
        &self.files
    }
}

/// True if `cargo check` passes in `root`. Blocks until it finishes, so the app
/// runs it on its own thread.
pub fn cargo_check(root: &Path) -> io::Result<bool> {
    let status = std::process::Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(status.success())
}

// ponytail: skips target/ and dot-dirs only; read .gitignore if other junk shows up.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            rust_files(&path, out)?;
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_file(name: String, text: &str) -> Block {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let tree = parser.parse(text, None).unwrap();
    let error = tree.root_node().has_error();
    Block {
        kind: "file",
        name,
        line: 1,
        children: if error {
            Vec::new()
        } else {
            stack(tree.root_node(), text)
        },
        error,
        span: 0..text.len(),
    }
}

/// The blocks directly inside `node` (a source file or a `{ }` body).
fn stack(node: Node, text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    // First of the comments and attributes directly above the next item.
    let mut lead: Option<Node> = None;
    let mut prev: Option<Node> = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if prev.is_some_and(|p| blank_line_between(p, child)) {
            lead = None;
        }
        prev = Some(child);
        if matches!(
            child.kind(),
            "line_comment" | "block_comment" | "attribute_item"
        ) {
            lead.get_or_insert(child);
            continue;
        }
        let first = lead.take().unwrap_or(child);
        let Some((kind, name)) = describe(child, text) else {
            continue;
        };
        let body = match kind {
            "impl" | "trait" | "mod" => child.child_by_field_name("body"),
            _ => None,
        };
        blocks.push(Block {
            kind,
            name,
            line: first.start_position().row + 1,
            children: body.map(|b| stack(b, text)).unwrap_or_default(),
            error: false,
            span: line_start(text, first.start_byte())..line_end(text, child.end_byte()),
        });
    }
    blocks
}

fn describe(node: Node, text: &str) -> Option<(&'static str, String)> {
    let field = |f: &str| {
        node.child_by_field_name(f)
            .map(|n| text[n.byte_range()].to_string())
    };
    let kind = match node.kind() {
        "function_item" | "function_signature_item" => "fn",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "trait_item" => "trait",
        "mod_item" => "mod",
        "const_item" => "const",
        "static_item" => "static",
        "type_item" => "type",
        "impl_item" => {
            let ty = field("type")?;
            let name = match field("trait") {
                Some(t) => format!("{t} for {ty}"),
                None => ty,
            };
            return Some(("impl", name));
        }
        _ => return None,
    };
    Some((kind, field("name")?))
}

/// True if an empty line separates `a` from `b`, which follows it.
fn blank_line_between(a: Node, b: Node) -> bool {
    let end = a.end_position();
    // A line comment's node may swallow its newline.
    let a_last_row = if end.column == 0 {
        end.row - 1
    } else {
        end.row
    };
    b.start_position().row > a_last_row + 1
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// Just past the newline ending the line `at` is on.
fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |i| at + i + 1)
}

/// `span` plus one blank line next to it (after, else before), so stacks keep one
/// blank line between blocks once it is cut.
fn with_one_blank_line(text: &str, span: Range<usize>) -> Range<usize> {
    let after = &text[span.end..line_end(text, span.end)];
    if !after.is_empty() && after.trim().is_empty() {
        return span.start..span.end + after.len();
    }
    if span.start > 0 {
        let before = line_start(text, span.start - 1);
        if text[before..span.start].trim().is_empty() {
            return before..span.end;
        }
    }
    span
}

fn indent(text: &str) -> &str {
    &text[..text.len() - text.trim_start_matches([' ', '\t']).len()]
}

fn reindent(text: &str, from: &str, to: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| match line.strip_prefix(from) {
            Some(rest) if !line.trim().is_empty() => format!("{to}{rest}"),
            _ => line.to_string(),
        })
        .collect()
}
