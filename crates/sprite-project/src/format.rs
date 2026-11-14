//! The native `.psmproj` format, version 1 (see `docs/formato-psmproj.md`).
//!
//! A ZIP container holding:
//! - `manifest.json`: format name and version, app version, the project model, the image table
//!   and the snapshot table;
//! - `images/<id>.png`: one lossless RGBA PNG per pixel buffer;
//! - `snapshots/<id>/project.json` and `snapshots/<id>/images/<img>.png`.
//!
//! Saving is atomic: the archive is written to a temporary file in the same folder, flushed to
//! disk, then renamed over the target, so a crash never leaves a half-written project.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sprite_animation::ImageStore;
use sprite_core::codec::{decode_png, encode_png};
use zip::write::SimpleFileOptions;

use crate::document::{Document, ProjectModel, Snapshot};
use crate::{ProjectError, time};

pub const FORMAT_NAME: &str = "pixel-studio-maker";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "psmproj";

#[derive(Serialize, Deserialize)]
struct ImageEntry {
    file: String,
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize)]
struct SnapshotEntry {
    id: u32,
    name: String,
    created: String,
    file: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    format_version: u32,
    app_version: String,
    saved_at: String,
    project: ProjectModel,
    images: BTreeMap<u32, ImageEntry>,
    #[serde(default)]
    snapshots: Vec<SnapshotEntry>,
}

#[derive(Serialize, Deserialize)]
struct SnapshotFile {
    project: ProjectModel,
    images: BTreeMap<u32, ImageEntry>,
}

fn zip_err(e: zip::result::ZipError) -> ProjectError {
    ProjectError::Format(e.to_string())
}

fn write_images(
    zip: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
    images: &ImageStore,
    prefix: &str,
    opts: SimpleFileOptions,
) -> Result<BTreeMap<u32, ImageEntry>, ProjectError> {
    let mut table = BTreeMap::new();
    for (id, img) in images {
        let file = format!("{prefix}images/{id}.png");
        // PNG is already deflated; store it as is.
        zip.start_file(file.as_str(), opts.compression_method(zip::CompressionMethod::Stored)).map_err(zip_err)?;
        zip.write_all(&encode_png(img)?)?;
        table.insert(*id, ImageEntry { file, width: img.width(), height: img.height() });
    }
    Ok(table)
}

/// Serializes a document into `.psmproj` bytes.
pub fn to_bytes(doc: &Document) -> Result<Vec<u8>, ProjectError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let images = write_images(&mut zip, doc.images(), "", opts)?;
    let mut snaps = Vec::new();
    for s in &doc.snapshots {
        let prefix = format!("snapshots/{}/", s.id);
        let table = write_images(&mut zip, &s.images, &prefix, opts)?;
        let file = format!("{prefix}project.json");
        zip.start_file(file.as_str(), opts).map_err(zip_err)?;
        zip.write_all(
            &serde_json::to_vec(&SnapshotFile { project: s.model.clone(), images: table })
                .map_err(|e| ProjectError::Format(e.to_string()))?,
        )?;
        snaps.push(SnapshotEntry { id: s.id, name: s.name.clone(), created: s.created.clone(), file });
    }
    let manifest = Manifest {
        format: FORMAT_NAME.into(),
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        saved_at: time::now_iso(),
        project: doc.model.clone(),
        images,
        snapshots: snaps,
    };
    zip.start_file("manifest.json", opts).map_err(zip_err)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| ProjectError::Format(e.to_string()))?)?;
    Ok(zip.finish().map_err(zip_err)?.into_inner())
}

fn read_entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, ProjectError> {
    let mut file = archive.by_name(name).map_err(|_| ProjectError::Format(format!("falta \"{name}\" no arquivo")))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn read_images(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, table: &BTreeMap<u32, ImageEntry>) -> Result<ImageStore, ProjectError> {
    let mut store = ImageStore::new();
    for (id, entry) in table {
        let img = decode_png(&read_entry(archive, &entry.file)?)?;
        if img.width() != entry.width || img.height() != entry.height {
            return Err(ProjectError::Integrity(format!(
                "imagem {id} tem {}x{}, manifesto diz {}x{}",
                img.width(),
                img.height(),
                entry.width,
                entry.height
            )));
        }
        store.insert(*id, img);
    }
    Ok(store)
}

/// Parses `.psmproj` bytes, checking format, version and integrity.
pub fn from_bytes(bytes: &[u8]) -> Result<Document, ProjectError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| ProjectError::Format(format!("não é um projeto válido ({e})")))?;
    let raw = read_entry(&mut archive, "manifest.json")?;
    let header: serde_json::Value = serde_json::from_slice(&raw).map_err(|e| ProjectError::Format(format!("manifest.json: {e}")))?;
    if header.get("format").and_then(|v| v.as_str()) != Some(FORMAT_NAME) {
        return Err(ProjectError::Format("não é um projeto do Pixel Studio Maker".into()));
    }
    let version = header.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version > FORMAT_VERSION {
        return Err(ProjectError::NewerVersion { found: version, supported: FORMAT_VERSION });
    }
    let manifest: Manifest =
        serde_json::from_value(migrate(header, version)?).map_err(|e| ProjectError::Format(format!("manifest.json: {e}")))?;
    let images = read_images(&mut archive, &manifest.images)?;
    let problems = check_integrity(&manifest.project, &images);
    if !problems.is_empty() {
        return Err(ProjectError::Integrity(problems.join("; ")));
    }
    let mut snapshots = Vec::new();
    for entry in &manifest.snapshots {
        let file: SnapshotFile = serde_json::from_slice(&read_entry(&mut archive, &entry.file)?)
            .map_err(|e| ProjectError::Format(format!("{}: {e}", entry.file)))?;
        let imgs = read_images(&mut archive, &file.images)?;
        snapshots.push(Snapshot {
            id: entry.id,
            name: entry.name.clone(),
            created: entry.created.clone(),
            model: file.project,
            images: imgs,
        });
    }
    Ok(Document::from_parts(manifest.project, images, snapshots))
}

/// Upgrades older manifests to the current shape. Version 1 is the first; future versions add
/// steps here so old files keep opening.
fn migrate(manifest: serde_json::Value, version: u32) -> Result<serde_json::Value, ProjectError> {
    match version {
        1 => Ok(manifest),
        v => Err(ProjectError::Format(format!("versão de formato desconhecida: {v}"))),
    }
}

/// Structural checks: unique ids, cels pointing at existing layers/frames/images, animations
/// pointing at existing frames, image sizes matching their sprite.
pub fn check_integrity(model: &ProjectModel, images: &ImageStore) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut claim = |id: u32, what: &str, problems: &mut Vec<String>| {
        if !seen.insert(id) {
            problems.push(format!("id {id} repetido ({what})"));
        }
        if id > model.next_id {
            problems.push(format!("id {id} maior que next_id {}", model.next_id));
        }
    };
    for s in &model.sprites {
        claim(s.id, "sprite", &mut problems);
        for l in &s.layers {
            claim(l.id, "layer", &mut problems);
        }
        for f in &s.frames {
            claim(f.id, "frame", &mut problems);
        }
        for a in &s.animations {
            claim(a.id, "animação", &mut problems);
            for t in &a.tracks {
                for f in &t.frames {
                    if s.frame(*f).is_none() {
                        problems.push(format!("animação \"{}\" usa frame {f}, que não existe", a.name));
                    }
                }
            }
        }
        for c in &s.cels {
            if s.layer_index(c.layer).is_none() || s.frame(c.frame).is_none() {
                problems.push(format!("cel ({}, {}) aponta para layer/frame inexistente", c.layer, c.frame));
            }
            match images.get(&c.image) {
                None => problems.push(format!("cel ({}, {}) aponta para imagem {} inexistente", c.layer, c.frame, c.image)),
                Some(img) if img.width() != s.width || img.height() != s.height => problems.push(format!(
                    "imagem {} tem {}x{}, sprite \"{}\" é {}x{}",
                    c.image,
                    img.width(),
                    img.height(),
                    s.name,
                    s.width,
                    s.height
                )),
                _ => {}
            }
        }
        if s.layers.is_empty() || s.frames.is_empty() {
            problems.push(format!("sprite \"{}\" precisa de pelo menos uma layer e um frame", s.name));
        }
    }
    for r in &model.references {
        if !images.contains_key(&r.image) {
            problems.push(format!("referência \"{}\" sem imagem", r.name));
        }
    }
    problems
}

/// Writes bytes atomically: temp file in the same folder, fsync, rename over the target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "projeto".into());
    let temp: PathBuf = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(e.into());
    }
    Ok(())
}

pub fn save(doc: &Document, path: &Path) -> Result<(), ProjectError> {
    write_atomic(path, &to_bytes(doc)?)
}

pub fn load(path: &Path) -> Result<Document, ProjectError> {
    from_bytes(&fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_animation::{Cel, Sprite};
    use sprite_core::{Color, Image};

    fn sample() -> Document {
        let mut doc = Document::new("Herói");
        doc.begin("setup").unwrap();
        let sid = doc.alloc_id();
        let mut ids = Vec::new();
        for _ in 0..2 {
            ids.push(doc.alloc_id());
        }
        let mut it = ids.into_iter();
        let mut s = Sprite::new(sid, "hero", 8, 8, &mut || it.next().unwrap());
        let mut img = Image::new(8, 8);
        img.set(3, 3, Color::rgb(57, 118, 210));
        img.set(4, 4, Color::rgba(10, 20, 30, 40));
        let image = doc.insert_image(img);
        s.cels.push(Cel { layer: s.layers[0].id, frame: s.frames[0].id, image, opacity: 255 });
        doc.model.sprites.push(s);
        doc.commit().unwrap();
        doc.create_snapshot("primeiro");
        doc
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let doc = sample();
        let bytes = to_bytes(&doc).unwrap();
        let back = from_bytes(&bytes).unwrap();
        assert_eq!(back.model, doc.model);
        assert_eq!(back.images(), doc.images());
        assert_eq!(back.snapshots.len(), 1);
        assert_eq!(back.snapshots[0].model, doc.snapshots[0].model);
        assert_eq!(back.snapshots[0].images, doc.snapshots[0].images);
    }

    #[test]
    fn rejects_newer_versions_and_garbage() {
        assert!(matches!(from_bytes(b"nope"), Err(ProjectError::Format(_))));
        let doc = sample();
        let bytes = to_bytes(&doc).unwrap();
        // Rewrite the manifest with a future version.
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_slice(&read_entry(&mut archive, "manifest.json").unwrap()).unwrap();
        manifest["format_version"] = serde_json::json!(99);
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("manifest.json", SimpleFileOptions::default()).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        let future = zip.finish().unwrap().into_inner();
        assert!(matches!(from_bytes(&future), Err(ProjectError::NewerVersion { found: 99, .. })));
    }

    #[test]
    fn integrity_catches_dangling_cels() {
        let mut doc = sample();
        doc.model.sprites[0].cels[0].image = 9999;
        let problems = check_integrity(&doc.model, doc.images());
        assert!(problems.iter().any(|p| p.contains("9999")), "{problems:?}");
    }

    #[test]
    fn atomic_save_and_load() {
        let dir = std::env::temp_dir().join(format!("psm-format-test-{}", std::process::id()));
        let path = dir.join("hero.psmproj");
        let doc = sample();
        save(&doc, &path).unwrap();
        save(&doc, &path).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(back.model, doc.model);
        let leftovers: Vec<_> =
            fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
