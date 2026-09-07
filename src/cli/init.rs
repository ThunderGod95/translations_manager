use std::path::Path;

use anyhow::{Result, anyhow};
use include_dir::{Dir, DirEntry, include_dir};
use std::fs::{create_dir_all, write};

pub static TEMPLATE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/template");

pub fn write_embedded_dir(embedded_dir: &'static Dir<'_>, dest: impl AsRef<Path>) -> Result<()> {
    let dest = dest.as_ref();

    if dest.exists() {
        return Err(anyhow!("Project path '{}' already exists.", dest.display()));
    }

    create_dir_all(dest)?;

    for entry in embedded_dir.entries() {
        let dest_path = dest.to_path_buf();
        let entry_path = entry.path();
        let file_name = entry_path
            .file_name()
            .ok_or_else(|| anyhow!("Entry has no filename -> {}", entry_path.display()))?;

        let full_dest_path = dest_path.join(file_name);

        match entry {
            DirEntry::Dir(dir) => {
                write_embedded_dir(dir, full_dest_path)?;
            }
            DirEntry::File(file) => {
                write(full_dest_path, file.contents())?;
            }
        }
    }

    Ok(())
}
