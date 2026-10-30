use archy_cli::{
    DesktopStatusInput, design_manifest_schema, desktop_capabilities, desktop_capabilities_schema,
    desktop_open_schema, desktop_status, desktop_status_schema,
};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn copy_private_tree(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir(destination).expect("create fixture directory");
    std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700))
        .expect("private fixture directory");
    for entry in std::fs::read_dir(source).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let target = destination.join(entry.file_name());
        let kind = entry.file_type().expect("fixture type");
        if kind.is_dir() {
            copy_private_tree(&entry.path(), &target);
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target).expect("copy fixture file");
        } else {
            panic!("fixture contains non-file entry");
        }
    }
}

fn main() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/courtyard-housing/project.yaml");
    let status = desktop_status(DesktopStatusInput {
        project,
        run: None,
        limit: 20,
        cursor: None,
    })
    .expect("example project status");
    let saved = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/courtyard-housing/expected/accepted-project");
    let copied =
        std::env::temp_dir().join(format!("archy-desktop-contract-{}", uuid::Uuid::new_v4()));
    copy_private_tree(&saved, &copied);
    let accepted_status = desktop_status(DesktopStatusInput {
        project: copied.join("project.yaml"),
        run: None,
        limit: 20,
        cursor: None,
    })
    .expect("accepted project status");
    std::fs::remove_dir_all(&copied).expect("remove disposable fixture copy");
    let value = serde_json::json!({
        "schemas": {
            "desktop-capabilities": desktop_capabilities_schema(),
            "desktop-status": desktop_status_schema(),
            "desktop-open": desktop_open_schema(),
            "design-manifest": design_manifest_schema(),
        },
        "capabilities": desktop_capabilities(),
        "status": status,
        "accepted_status": accepted_status,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("serialize contracts")
    );
}
