use archy_core::{ArchyError, InputFormat, parse_project};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

pub fn run(name: &str, dir: &Path) -> Result<(), ArchyError> {
    // Parse before touching the filesystem: project IDs cannot inject YAML or paths.
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(ArchyError::schema(
            "name",
            "Use 1-128 ASCII letters, digits, hyphens or underscores",
        ));
    }
    let text = format!(
        "# yaml-language-server: $schema=./project.schema.json\n{}",
        include_str!("../../../examples/courtyard-housing/project.yaml")
            .replace("courtyard-demo", name)
    );
    parse_project(&text, InputFormat::Yaml)?;
    // create_dir is atomic and refuses existing directories, files and symlinks.
    // The parent must exist; no ancestors outside the requested target are created.
    fs::create_dir(dir).map_err(|e| crate::io_error(dir, e))?;
    for (name, bytes) in [
        (
            "project.schema.json",
            include_bytes!("../../../schemas/project.schema.json").as_slice(),
        ),
        ("project.yaml", text.as_bytes()),
    ] {
        let path = dir.join(name);
        // Never remove target files on errors: concurrent/user changes stay intact.
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| crate::io_error(&path, e))?;
        file.write_all(bytes)
            .map_err(|e| crate::io_error(&path, e))?;
        file.sync_all().map_err(|e| crate::io_error(&path, e))?;
    }
    Ok(())
}
