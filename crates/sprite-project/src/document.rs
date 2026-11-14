//! The in-memory project and its transactional history.
//!
//! Every change happens inside a transaction: [`Document::begin`] captures the model, and each
//! image is captured the first time it is touched ([`Document::image_mut`],
//! [`Document::insert_image`], [`Document::remove_image`]). [`Document::commit`] turns the
//! differences into one undo step; [`Document::rollback`] restores everything. Savepoints give
//! per-command atomicity inside a longer, explicit transaction.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_animation::sheet::SheetOptions;
use sprite_animation::{ImageStore, Sprite};
use sprite_core::{Image, ItemRef, Palette, Point};

use crate::ProjectError;

fn opaque() -> u8 {
    255
}
fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}

/// A reference image shown in the editor (tracing, style guide, another direction...). Never
/// exported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Reference {
    pub id: u32,
    pub name: String,
    pub image: u32,
    /// Sprite it belongs to (shown over that sprite's canvas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<u32>,
    #[serde(default)]
    pub position: Point,
    #[serde(default = "one")]
    pub scale: u32,
    #[serde(default = "opaque")]
    pub opacity: u8,
    #[serde(default = "yes")]
    pub visible: bool,
}

/// Export defaults stored with the project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExportDefaults {
    #[serde(default)]
    pub sheet: SheetOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

/// Everything about a project except pixels. Cheap to clone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectModel {
    /// Stable identifier (survives renames and moves).
    pub uid: String,
    pub name: String,
    pub sprites: Vec<Sprite>,
    #[serde(default)]
    pub palettes: Vec<Palette>,
    /// Palette used for integer color references and palette checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_palette: Option<String>,
    #[serde(default)]
    pub references: Vec<Reference>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub export: ExportDefaults,
    pub next_id: u32,
}

impl ProjectModel {
    pub fn sprite(&self, r: Option<&ItemRef>) -> Option<&Sprite> {
        match r {
            None => self.sprites.first(),
            Some(r) => self.sprites.iter().find(|s| r.matches(s.id, &s.name)),
        }
    }

    pub fn sprite_index(&self, r: Option<&ItemRef>) -> Option<usize> {
        match r {
            None => (!self.sprites.is_empty()).then_some(0),
            Some(r) => self.sprites.iter().position(|s| r.matches(s.id, &s.name)),
        }
    }

    pub fn palette(&self, name: Option<&str>) -> Option<&Palette> {
        let name = name.or(self.active_palette.as_deref())?;
        self.palettes.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Image ids referenced by cels and references.
    pub fn referenced_images(&self) -> BTreeSet<u32> {
        let mut set: BTreeSet<u32> = self.sprites.iter().flat_map(|s| s.cels.iter().map(|c| c.image)).collect();
        set.extend(self.references.iter().map(|r| r.image));
        set
    }
}

/// A named full copy of the project, restorable later.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub id: u32,
    pub name: String,
    pub created: String,
    pub model: ProjectModel,
    pub images: ImageStore,
}

#[derive(Debug, Clone)]
struct Savepoint {
    model: ProjectModel,
    touched: BTreeMap<u32, Option<Image>>,
}

#[derive(Debug, Clone)]
struct Tx {
    id: u64,
    label: String,
    model_before: ProjectModel,
    touched: BTreeMap<u32, Option<Image>>,
    savepoints: Vec<Savepoint>,
}

#[derive(Debug, Clone)]
struct Entry {
    id: u64,
    label: String,
    model_before: ProjectModel,
    model_after: ProjectModel,
    before: BTreeMap<u32, Option<Image>>,
    after: BTreeMap<u32, Option<Image>>,
    bytes: usize,
}

/// Result of a commit.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct CommitInfo {
    pub transaction: u64,
    pub label: String,
    /// Image ids whose pixels changed.
    pub changed_images: Vec<u32>,
    pub model_changed: bool,
    pub revision: u64,
}

impl CommitInfo {
    pub fn is_empty(&self) -> bool {
        self.changed_images.is_empty() && !self.model_changed
    }
}

/// History steps as (transaction id, label).
pub type HistoryList = Vec<(u64, String)>;

/// History limits.
const MAX_HISTORY_ENTRIES: usize = 1000;
const MAX_HISTORY_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Document {
    pub model: ProjectModel,
    images: ImageStore,
    pub snapshots: Vec<Snapshot>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    tx: Option<Tx>,
    next_tx: u64,
    revision: u64,
}

fn image_bytes(img: &Option<Image>) -> usize {
    img.as_ref().map(|i| i.width() as usize * i.height() as usize * 4).unwrap_or(0)
}

/// Random-enough unique id without extra crates: time + process + address entropy.
pub fn new_uid() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let local = 0u8;
    let addr = &local as *const u8 as usize as u128;
    let pid = std::process::id() as u128;
    let mut x = nanos ^ (addr << 17) ^ (pid << 64) ^ 0x9e37_79b9_7f4a_7c15_f39c_c060_5ced_c834;
    // xorshift mixing
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd_c4ce_b9fe_1a85_ec53);
    x ^= x >> 29;
    format!("{:016x}", (x as u64) ^ ((x >> 64) as u64))
}

impl Document {
    /// An empty project (no sprites yet) with the default palette.
    pub fn new(name: impl Into<String>) -> Self {
        let palette = Palette::studio_default();
        let model = ProjectModel {
            uid: new_uid(),
            name: name.into(),
            sprites: Vec::new(),
            active_palette: Some(palette.name.clone()),
            palettes: vec![palette],
            references: Vec::new(),
            metadata: BTreeMap::new(),
            export: ExportDefaults::default(),
            next_id: 0,
        };
        Document::from_parts(model, ImageStore::new(), Vec::new())
    }

    pub fn from_parts(model: ProjectModel, images: ImageStore, snapshots: Vec<Snapshot>) -> Self {
        Document { model, images, snapshots, undo: Vec::new(), redo: Vec::new(), tx: None, next_tx: 1, revision: 0 }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn images(&self) -> &ImageStore {
        &self.images
    }

    pub fn image(&self, id: u32) -> Option<&Image> {
        self.images.get(&id)
    }

    pub fn alloc_id(&mut self) -> u32 {
        self.model.next_id += 1;
        self.model.next_id
    }

    fn record(&mut self, id: u32) {
        if let Some(tx) = &mut self.tx {
            let current = self.images.get(&id).cloned();
            tx.touched.entry(id).or_insert_with(|| current.clone());
            if let Some(sp) = tx.savepoints.last_mut() {
                sp.touched.entry(id).or_insert(current);
            }
        }
    }

    /// Mutable pixels, recorded for undo.
    pub fn image_mut(&mut self, id: u32) -> Option<&mut Image> {
        if !self.images.contains_key(&id) {
            return None;
        }
        self.record(id);
        self.images.get_mut(&id)
    }

    pub fn insert_image(&mut self, image: Image) -> u32 {
        let id = self.alloc_id();
        self.record(id);
        self.images.insert(id, image);
        id
    }

    pub fn replace_image(&mut self, id: u32, image: Image) {
        self.record(id);
        self.images.insert(id, image);
    }

    pub fn remove_image(&mut self, id: u32) {
        if self.images.contains_key(&id) {
            self.record(id);
            self.images.remove(&id);
        }
    }

    // ---- transactions -----------------------------------------------------------------------

    pub fn in_transaction(&self) -> bool {
        self.tx.is_some()
    }

    pub fn transaction_id(&self) -> Option<u64> {
        self.tx.as_ref().map(|t| t.id)
    }

    pub fn transaction_label(&self) -> Option<&str> {
        self.tx.as_ref().map(|t| t.label.as_str())
    }

    pub fn begin(&mut self, label: impl Into<String>) -> Result<u64, ProjectError> {
        if self.tx.is_some() {
            return Err(ProjectError::Transaction("já existe uma transação aberta".into()));
        }
        let id = self.next_tx;
        self.next_tx += 1;
        self.tx = Some(Tx { id, label: label.into(), model_before: self.model.clone(), touched: BTreeMap::new(), savepoints: Vec::new() });
        Ok(id)
    }

    pub fn savepoint(&mut self) {
        if let Some(tx) = &mut self.tx {
            tx.savepoints.push(Savepoint { model: self.model.clone(), touched: BTreeMap::new() });
        }
    }

    /// Keeps the changes made since the last savepoint.
    pub fn release_savepoint(&mut self) {
        if let Some(tx) = &mut self.tx
            && let Some(sp) = tx.savepoints.pop()
            && let Some(parent) = tx.savepoints.last_mut()
        {
            for (id, img) in sp.touched {
                parent.touched.entry(id).or_insert(img);
            }
        }
    }

    /// Undoes the changes made since the last savepoint.
    pub fn rollback_savepoint(&mut self) {
        let Some(tx) = &mut self.tx else { return };
        let Some(sp) = tx.savepoints.pop() else { return };
        self.model = sp.model;
        for (id, img) in sp.touched {
            match img {
                Some(i) => {
                    self.images.insert(id, i);
                }
                None => {
                    self.images.remove(&id);
                }
            }
        }
    }

    /// Drops pixel buffers no cel or reference uses any more (recorded, so undo brings them back).
    fn collect_garbage(&mut self) {
        let used = self.model.referenced_images();
        let unused: Vec<u32> = self.images.keys().filter(|id| !used.contains(id)).copied().collect();
        for id in unused {
            self.remove_image(id);
        }
    }

    pub fn commit(&mut self) -> Result<CommitInfo, ProjectError> {
        if self.tx.is_none() {
            return Err(ProjectError::Transaction("nenhuma transação aberta".into()));
        }
        self.collect_garbage();
        let tx = self.tx.take().expect("checked above");
        let mut before = BTreeMap::new();
        let mut after = BTreeMap::new();
        for (id, old) in tx.touched {
            let new = self.images.get(&id).cloned();
            if old != new {
                before.insert(id, old);
                after.insert(id, new);
            }
        }
        let model_changed = tx.model_before != self.model;
        let changed_images: Vec<u32> = after.keys().copied().collect();
        if model_changed || !changed_images.is_empty() {
            self.revision += 1;
            let bytes = before.values().map(image_bytes).sum::<usize>() + after.values().map(image_bytes).sum::<usize>();
            self.undo.push(Entry {
                id: tx.id,
                label: tx.label.clone(),
                model_before: tx.model_before,
                model_after: self.model.clone(),
                before,
                after,
                bytes,
            });
            self.redo.clear();
            self.trim_history();
        }
        Ok(CommitInfo { transaction: tx.id, label: tx.label, changed_images, model_changed, revision: self.revision })
    }

    pub fn rollback(&mut self) -> Result<(), ProjectError> {
        let Some(tx) = self.tx.take() else {
            return Err(ProjectError::Transaction("nenhuma transação aberta".into()));
        };
        self.model = tx.model_before;
        for (id, img) in tx.touched {
            match img {
                Some(i) => {
                    self.images.insert(id, i);
                }
                None => {
                    self.images.remove(&id);
                }
            }
        }
        Ok(())
    }

    fn trim_history(&mut self) {
        let mut total: usize = self.undo.iter().map(|e| e.bytes).sum();
        while self.undo.len() > 1 && (self.undo.len() > MAX_HISTORY_ENTRIES || total > MAX_HISTORY_BYTES) {
            total -= self.undo[0].bytes;
            self.undo.remove(0);
        }
    }

    fn apply_entry(&mut self, model: ProjectModel, images: &BTreeMap<u32, Option<Image>>) {
        self.model = model;
        for (id, img) in images {
            match img {
                Some(i) => {
                    self.images.insert(*id, i.clone());
                }
                None => {
                    self.images.remove(id);
                }
            }
        }
        self.revision += 1;
    }

    /// Returns the undone step's label and the image ids it touched.
    pub fn undo(&mut self) -> Result<Option<(String, Vec<u32>)>, ProjectError> {
        if self.tx.is_some() {
            return Err(ProjectError::Transaction("feche a transação aberta antes de desfazer".into()));
        }
        let Some(entry) = self.undo.pop() else { return Ok(None) };
        self.apply_entry(entry.model_before.clone(), &entry.before);
        let info = (entry.label.clone(), entry.before.keys().copied().collect());
        self.redo.push(entry);
        Ok(Some(info))
    }

    pub fn redo(&mut self) -> Result<Option<(String, Vec<u32>)>, ProjectError> {
        if self.tx.is_some() {
            return Err(ProjectError::Transaction("feche a transação aberta antes de refazer".into()));
        }
        let Some(entry) = self.redo.pop() else { return Ok(None) };
        self.apply_entry(entry.model_after.clone(), &entry.after);
        let info = (entry.label.clone(), entry.after.keys().copied().collect());
        self.undo.push(entry);
        Ok(Some(info))
    }

    /// Labels of undoable steps (oldest first) and of redoable ones (next first).
    pub fn history(&self) -> (HistoryList, HistoryList) {
        (self.undo.iter().map(|e| (e.id, e.label.clone())).collect(), self.redo.iter().rev().map(|e| (e.id, e.label.clone())).collect())
    }

    // ---- snapshots --------------------------------------------------------------------------

    pub fn create_snapshot(&mut self, name: impl Into<String>) -> u32 {
        let used = self.model.referenced_images();
        let images: ImageStore = self.images.iter().filter(|(id, _)| used.contains(id)).map(|(k, v)| (*k, v.clone())).collect();
        let id = self.snapshots.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        self.snapshots.push(Snapshot { id, name: name.into(), created: crate::time::now_iso(), model: self.model.clone(), images });
        id
    }

    /// Replaces the project content with a snapshot's (inside the current transaction, so it
    /// can be undone). Ids allocated since the snapshot stay reserved.
    pub fn restore_snapshot(&mut self, id: u32) -> Result<(), ProjectError> {
        if self.tx.is_none() {
            return Err(ProjectError::Transaction("restaurar snapshot exige uma transação".into()));
        }
        let snap = self
            .snapshots
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| ProjectError::Transaction(format!("snapshot {id} não existe")))?;
        let next_id = self.model.next_id.max(snap.model.next_id);
        let uid = self.model.uid.clone();
        let current: Vec<u32> = self.images.keys().copied().collect();
        for img_id in current {
            if !snap.images.contains_key(&img_id) {
                self.remove_image(img_id);
            }
        }
        for (img_id, img) in snap.images {
            if self.images.get(&img_id) != Some(&img) {
                self.replace_image(img_id, img);
            }
        }
        self.model = snap.model;
        self.model.next_id = next_id;
        self.model.uid = uid;
        Ok(())
    }

    /// Replaces the whole content (used by reload/recovery). Clears history.
    pub fn reset_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.tx = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::Color;

    fn doc_with_image() -> (Document, u32) {
        let mut doc = Document::new("t");
        doc.begin("setup").unwrap();
        let id = doc.insert_image(Image::new(4, 4));
        // Keep it referenced so garbage collection leaves it alone.
        let mut ids = {
            let mut n = 1000;
            move || {
                n += 1;
                n
            }
        };
        let mut s = Sprite::new(1, "s", 4, 4, &mut ids);
        s.cels.push(sprite_animation::Cel { layer: s.layers[0].id, frame: s.frames[0].id, image: id, opacity: 255 });
        doc.model.sprites.push(s);
        doc.commit().unwrap();
        (doc, id)
    }

    #[test]
    fn commit_undo_redo() {
        let (mut doc, id) = doc_with_image();
        let r0 = doc.revision();
        doc.begin("paint").unwrap();
        doc.image_mut(id).unwrap().set(1, 1, Color::BLACK);
        let info = doc.commit().unwrap();
        assert_eq!(info.changed_images, vec![id]);
        assert_eq!(doc.revision(), r0 + 1);
        doc.undo().unwrap().unwrap();
        assert_eq!(doc.image(id).unwrap().get(1, 1), Some(Color::TRANSPARENT));
        doc.redo().unwrap().unwrap();
        assert_eq!(doc.image(id).unwrap().get(1, 1), Some(Color::BLACK));
        assert_eq!(doc.history().0.last().unwrap().1, "paint");
    }

    #[test]
    fn empty_commit_makes_no_step() {
        let (mut doc, id) = doc_with_image();
        let steps = doc.history().0.len();
        doc.begin("noop").unwrap();
        doc.image_mut(id).unwrap();
        let info = doc.commit().unwrap();
        assert!(info.is_empty());
        assert_eq!(doc.history().0.len(), steps);
    }

    #[test]
    fn rollback_and_savepoints() {
        let (mut doc, id) = doc_with_image();
        doc.begin("big").unwrap();
        doc.image_mut(id).unwrap().set(0, 0, Color::BLACK);
        doc.savepoint();
        doc.image_mut(id).unwrap().set(1, 0, Color::BLACK);
        doc.model.name = "renamed".into();
        doc.rollback_savepoint();
        assert_eq!(doc.image(id).unwrap().get(1, 0), Some(Color::TRANSPARENT));
        assert_eq!(doc.image(id).unwrap().get(0, 0), Some(Color::BLACK));
        assert_eq!(doc.model.name, "t");
        doc.rollback().unwrap();
        assert_eq!(doc.image(id).unwrap().get(0, 0), Some(Color::TRANSPARENT));
        assert!(doc.undo().is_ok());
    }

    #[test]
    fn unreferenced_images_are_collected_and_restorable() {
        let (mut doc, id) = doc_with_image();
        doc.begin("drop cel").unwrap();
        doc.model.sprites[0].cels.clear();
        doc.commit().unwrap();
        assert!(doc.image(id).is_none());
        doc.undo().unwrap();
        assert!(doc.image(id).is_some());
    }

    #[test]
    fn snapshots_restore_content() {
        let (mut doc, id) = doc_with_image();
        let snap = doc.create_snapshot("antes");
        doc.begin("paint").unwrap();
        doc.image_mut(id).unwrap().fill(Color::WHITE);
        doc.commit().unwrap();
        doc.begin("restore").unwrap();
        doc.restore_snapshot(snap).unwrap();
        doc.commit().unwrap();
        assert!(doc.image(id).unwrap().is_blank());
        doc.undo().unwrap();
        assert_eq!(doc.image(id).unwrap().get(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn transactions_do_not_nest() {
        let mut doc = Document::new("t");
        doc.begin("a").unwrap();
        assert!(doc.begin("b").is_err());
        assert!(doc.undo().is_err());
        doc.commit().unwrap();
        assert!(doc.commit().is_err());
    }
}
