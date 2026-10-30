//! Fixed A09 service gate. The controller owns the service and decides whether GO is sent.
use rustix::fs::{self, MemfdFlags, OFlags, SealFlags, StatVfsMountFlags};
use rustix::io::{FdFlags, fcntl_setfd};
use rustix::net::{
    RecvAncillaryBuffer, RecvFlags, ReturnFlags, SendAncillaryBuffer, SendAncillaryMessage,
    SendFlags, SocketType, recvmsg, sendmsg, sockopt,
};
use rustix::process::getuid;
use serde::Deserialize;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{IoSlice, IoSliceMut, Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, FromRawFd};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const FRAME: &[u8] = b"ARCHY-SCRATCH-1";
const REQUEST_FRAME: &[u8] = b"ARCHY-REQUEST-1\n";
const SCRATCH_BYTES: u64 = 128 * 1024 * 1024;
const SCRATCH_INODES: u64 = 256;
const BWRAP: &str = "/usr/lib/archy/runtime/bin/bwrap";
const GATE: &str = "/usr/lib/archy/archy-service-gate";
const INVENTORY: &str = "/usr/lib/archy/runtime/inventory.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Blender,
    Ifc,
}

impl Backend {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "blender" => Ok(Self::Blender),
            "ifc" => Ok(Self::Ifc),
            _ => Err("unsupported backend".into()),
        }
    }
    fn operation(self) -> &'static str {
        match self {
            Self::Blender => "build_massing",
            Self::Ifc => "export_schematic_ifc",
        }
    }
    fn schema(self) -> &'static str {
        match self {
            Self::Blender => "blender-worker-v1",
            Self::Ifc => "ifc-worker-v1",
        }
    }
}

#[derive(Deserialize)]
struct Request {
    schema_version: String,
    operation: String,
    request_id: String,
    project_id: String,
    variant_id: String,
    run_id: String,
    output_dir: String,
    #[serde(default)]
    source_artifacts: Option<Vec<serde_json::Value>>,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema_version: u32,
    files: Vec<RuntimeFile>,
    system_files: Vec<RuntimeFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeFile {
    path: String,
    sha256: String,
    mode: u32,
}

struct UniqueJson(serde_json::Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueVisitor)
    }
}

struct UniqueVisitor;

impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = UniqueJson;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JSON without duplicate keys or nonfinite numbers")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(v.into()))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(v)
            .map(|n| UniqueJson(n.into()))
            .ok_or_else(|| E::custom("nonfinite JSON number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(v.into()))
    }
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(serde_json::Value::Null))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(serde_json::Value::Null))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(values.into()))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some((key, value)) = map.next_entry::<String, UniqueJson>()? {
            if values.insert(key, value.0).is_some() {
                return Err(de::Error::custom("duplicate JSON key"));
            }
        }
        Ok(UniqueJson(values.into()))
    }
}

fn unavailable(why: impl std::fmt::Display) -> String {
    format!("BACKEND_UNAVAILABLE: {why}")
}

fn private_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(unavailable("path is not absolute"));
    }
    for part in path.ancestors() {
        let m = std::fs::symlink_metadata(part).map_err(unavailable)?;
        let system_parent = part == Path::new("/") || part == Path::new("/tmp");
        if m.file_type().is_symlink()
            || (part != Path::new("/tmp") && m.mode() & 0o022 != 0)
            || (!system_parent && m.uid() != 0 && m.uid() != getuid().as_raw())
        {
            return Err(unavailable(format!("unsafe ancestor {}", part.display())));
        }
        if part == Path::new("/tmp") && m.mode() & 0o7777 != 0o1777 {
            return Err(unavailable("unsafe /tmp"));
        }
    }
    Ok(())
}

fn root_owned(uid: u32) -> bool {
    if uid == 0 {
        return true;
    }
    let self_uid = getuid().as_raw();
    uid == 65534
        && std::fs::read_to_string("/proc/self/uid_map")
            .ok()
            .is_some_and(|map| {
                map.split_whitespace().collect::<Vec<_>>()
                    == [self_uid.to_string(), self_uid.to_string(), "1".to_string()]
            })
}

fn trusted_path(p: &Path) -> Result<(), String> {
    if std::fs::canonicalize(p).map_err(unavailable)? != p {
        return Err(unavailable("noncanonical runtime path"));
    }
    for part in p.ancestors() {
        let m = std::fs::symlink_metadata(part).map_err(unavailable)?;
        if m.file_type().is_symlink() || !root_owned(m.uid()) || m.mode() & 0o022 != 0 {
            return Err(unavailable(format!(
                "mutable runtime path {}",
                part.display()
            )));
        }
    }
    Ok(())
}

fn trusted_file(path: &str, expected: &str) -> Result<(), String> {
    let p = Path::new(path);
    trusted_path(p)?;
    let m = std::fs::metadata(p).map_err(unavailable)?;
    if !m.is_file() || m.len() > 512 * 1024 * 1024 {
        return Err(unavailable("invalid runtime file"));
    }
    let mut hash = Sha256::new();
    let mut f = File::open(p).map_err(unavailable)?;
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf).map_err(unavailable)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    let digest = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if digest != expected {
        return Err(unavailable(format!("runtime digest mismatch: {path}")));
    }
    Ok(())
}

fn runtime_payload(
    root: &Path,
) -> Result<std::collections::BTreeMap<String, (String, u32)>, String> {
    trusted_path(root)?;
    let mut pending = vec![root.to_path_buf()];
    let mut files = std::collections::BTreeMap::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).map_err(unavailable)? {
            let entry = entry.map_err(unavailable)?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).map_err(unavailable)?;
            if meta.file_type().is_symlink() || !root_owned(meta.uid()) || meta.mode() & 0o022 != 0
            {
                return Err(unavailable("mutable runtime payload"));
            }
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                if path != Path::new(INVENTORY) {
                    if meta.len() > 512 * 1024 * 1024 {
                        return Err(unavailable("invalid runtime file"));
                    }
                    let mut hash = Sha256::new();
                    let mut file = File::open(&path).map_err(unavailable)?;
                    let mut buf = [0; 65536];
                    loop {
                        let n = file.read(&mut buf).map_err(unavailable)?;
                        if n == 0 {
                            break;
                        }
                        hash.update(&buf[..n]);
                    }
                    let digest = hash
                        .finalize()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect();
                    files.insert(
                        path.to_string_lossy().into_owned(),
                        (digest, meta.mode() & 0o7777),
                    );
                }
            } else {
                return Err(unavailable("special runtime payload"));
            }
        }
    }
    Ok(files)
}

const ELF_LOADER: &str = "/usr/lib/ld-linux-x86-64.so.2";
const ELF_ENTRY_POINTS: [&str; 4] = [
    "/usr/lib/archy/runtime/bin/bwrap",
    "/usr/lib/archy/runtime/blender/blender",
    "/usr/lib/archy/runtime/python/bin/python3.12",
    "/usr/lib/archy/runtime/node/bin/node",
];

struct ElfRequirements {
    needed: Vec<String>,
    search: Vec<PathBuf>,
    inherited: Vec<PathBuf>,
    has_runpath: bool,
    has_search_path: bool,
}

fn elf_file_mapping(loads: &[(u64, u64, u64)], address: u64, size: u64) -> Result<u64, String> {
    let mut mapping = None;
    for &(virtual_address, file_offset, file_size) in loads {
        let Some(relative) = address.checked_sub(virtual_address) else {
            continue;
        };
        if relative.checked_add(size).is_none_or(|end| end > file_size) {
            continue;
        }
        let offset = file_offset
            .checked_add(relative)
            .ok_or_else(|| unavailable("ELF mapping overflow"))?;
        if mapping.is_some_and(|old| old != offset) {
            return Err(unavailable("ambiguous ELF load mapping"));
        }
        mapping = Some(offset);
    }
    mapping.ok_or_else(|| unavailable("ELF range outside load segments"))
}

fn elf_requirements(path: &str, entry: bool) -> Result<Option<ElfRequirements>, String> {
    let file = File::open(path).map_err(unavailable)?;
    let mut hdr = [0u8; 64];
    let n = file.read_at(&mut hdr, 0).map_err(unavailable)?;
    if n < 4 || &hdr[..4] != b"\x7fELF" {
        return if entry {
            Err(unavailable("runtime entry is not ELF"))
        } else {
            Ok(None)
        };
    }
    if n != hdr.len()
        || hdr[4] != 2
        || hdr[5] != 1
        || hdr[6] != 1
        || u16::from_le_bytes([hdr[18], hdr[19]]) != 62
    {
        return Err(unavailable(format!("unsupported runtime ELF: {path}")));
    }
    let ty = u16::from_le_bytes([hdr[16], hdr[17]]);
    if ty != 2 && ty != 3 {
        return if entry {
            Err(unavailable("unsupported entry ELF type"))
        } else {
            Ok(None)
        };
    }
    let phoff = u64::from_le_bytes(hdr[32..40].try_into().unwrap());
    let phsize = u16::from_le_bytes(hdr[54..56].try_into().unwrap());
    let phnum = u16::from_le_bytes(hdr[56..58].try_into().unwrap());
    if phsize != 56 || phnum == 0 || phnum > 1024 {
        return Err(unavailable("invalid runtime ELF program headers"));
    }
    let file_len = file.metadata().map_err(unavailable)?.len();
    let mut loads = Vec::new();
    let mut dynamic = None;
    let mut interpreter = false;
    for index in 0..u64::from(phnum) {
        let offset = phoff
            .checked_add(index * 56)
            .ok_or_else(|| unavailable("ELF header overflow"))?;
        let mut ph = [0u8; 56];
        file.read_exact_at(&mut ph, offset).map_err(unavailable)?;
        let kind = u32::from_le_bytes(ph[..4].try_into().unwrap());
        let off = u64::from_le_bytes(ph[8..16].try_into().unwrap());
        let addr = u64::from_le_bytes(ph[16..24].try_into().unwrap());
        let size = u64::from_le_bytes(ph[32..40].try_into().unwrap());
        let memory_size = u64::from_le_bytes(ph[40..48].try_into().unwrap());
        if (kind == 1 || kind == 2)
            && (size > memory_size
                || off.checked_add(size).is_none_or(|end| end > file_len)
                || addr.checked_add(memory_size).is_none())
        {
            return Err(unavailable("invalid ELF segment bounds"));
        }
        if kind == 1 {
            let alignment = u64::from_le_bytes(ph[48..56].try_into().unwrap());
            if (alignment > 1
                && (!alignment.is_power_of_two() || off % alignment != addr % alignment))
                || off % 4096 != addr % 4096
            {
                return Err(unavailable("invalid ELF load alignment"));
            }
            loads.push((addr, off, size));
        }
        if kind == 2 && dynamic.replace((addr, off, size)).is_some() {
            return Err(unavailable("duplicate ELF dynamic segment"));
        }
        if kind == 3 {
            if interpreter || !(2..=256).contains(&size) {
                return Err(unavailable("invalid ELF interpreter"));
            }
            let mut data = vec![0; size as usize];
            file.read_exact_at(&mut data, off).map_err(unavailable)?;
            if data.last() != Some(&0) || data[..data.len() - 1].contains(&0) {
                return Err(unavailable("invalid ELF interpreter"));
            }
            let name = std::str::from_utf8(&data[..data.len() - 1]).map_err(unavailable)?;
            if !Path::new(name).is_absolute()
                || std::fs::canonicalize(name).map_err(unavailable)? != Path::new(ELF_LOADER)
            {
                return Err(unavailable("unsupported ELF interpreter"));
            }
            interpreter = true;
        }
    }
    if entry && (!interpreter || dynamic.is_none()) {
        return Err(unavailable("entry must use the audited dynamic loader"));
    }
    let Some((dynamic_addr, dynamic_off, dynamic_size)) = dynamic else {
        return Ok(None);
    };
    if dynamic_size == 0 || dynamic_size > 1024 * 1024 || dynamic_size % 16 != 0 {
        return Err(unavailable("invalid ELF dynamic table"));
    }
    if elf_file_mapping(&loads, dynamic_addr, dynamic_size)? != dynamic_off {
        return Err(unavailable(
            "ELF dynamic segment mapping disagrees with file offset",
        ));
    }
    let mut table = vec![0u8; dynamic_size as usize];
    file.read_exact_at(&mut table, dynamic_off)
        .map_err(unavailable)?;
    let mut straddr = None;
    let mut strsize = None;
    let mut offsets = Vec::new();
    let mut searches = Vec::new();
    let mut terminated = false;
    for row in table.as_chunks::<16>().0 {
        let tag = u64::from_le_bytes(row[..8].try_into().unwrap());
        let value = u64::from_le_bytes(row[8..].try_into().unwrap());
        match tag {
            0 => {
                terminated = true;
                break;
            }
            1 => offsets.push(value),
            5 => {
                if straddr.replace(value).is_some() {
                    return Err(unavailable("duplicate ELF string table"));
                }
            }
            10 => {
                if strsize.replace(value).is_some() {
                    return Err(unavailable("duplicate ELF string size"));
                }
            }
            15 | 29 => {
                if searches.iter().any(|(previous, _)| *previous == tag) {
                    return Err(unavailable("duplicate ELF search path"));
                }
                searches.push((tag, value));
            }
            0x6ffffffb if value & 0x800 != 0 => {
                return Err(unavailable("unsupported ELF NODEFLIB"));
            }
            0x6ffffefb | 0x6ffffefc | 0x7ffffffd | 0x7fffffff => {
                return Err(unavailable("unsupported ELF audit or filter"));
            }
            _ => {}
        }
    }
    if !terminated {
        return Err(unavailable("unterminated ELF dynamic table"));
    }
    if offsets.is_empty() && searches.is_empty() {
        return Ok(Some(ElfRequirements {
            needed: vec![],
            search: vec![],
            inherited: vec![],
            has_runpath: false,
            has_search_path: false,
        }));
    }
    let addr = straddr.ok_or_else(|| unavailable("missing ELF string table"))?;
    let size = strsize.ok_or_else(|| unavailable("missing ELF string size"))?;
    if size == 0 {
        return Err(unavailable("invalid ELF string size"));
    }
    let offset = elf_file_mapping(&loads, addr, size)?;
    if offset.checked_add(size).is_none_or(|end| end > file_len) {
        return Err(unavailable("ELF string table exceeds file"));
    }
    let string = |index: u64| -> Result<String, String> {
        let remaining = size
            .checked_sub(index)
            .filter(|&n| n > 0)
            .ok_or_else(|| unavailable("invalid ELF string offset"))?;
        let length = remaining.min(4096) as usize;
        let mut bytes = vec![0u8; length];
        file.read_exact_at(
            &mut bytes,
            offset
                .checked_add(index)
                .ok_or_else(|| unavailable("ELF string offset overflow"))?,
        )
        .map_err(unavailable)?;
        let end = bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| unavailable("unterminated or oversized ELF string"))?;
        Ok(std::str::from_utf8(&bytes[..end])
            .map_err(unavailable)?
            .to_owned())
    };
    let mut needed = Vec::new();
    for offset in offsets {
        let name = string(offset)?;
        if name.is_empty()
            || name.contains('/')
            || name.contains('$')
            || name == "."
            || name == ".."
        {
            return Err(unavailable("unsupported ELF needed name"));
        }
        needed.push(name);
    }
    let origin = Path::new(path)
        .parent()
        .ok_or_else(|| unavailable("ELF has no parent"))?
        .to_string_lossy();
    let has_runpath = searches.iter().any(|(tag, _)| *tag == 29);
    let has_search_path = !searches.is_empty();
    let mut rpath = Vec::new();
    let mut runpath = Vec::new();
    for (tag, offset) in searches {
        let value = string(offset)?;
        let search = if tag == 29 { &mut runpath } else { &mut rpath };
        for part in value.split(':') {
            let expanded = part
                .replace("${ORIGIN}", &origin)
                .replace("$ORIGIN", &origin);
            if expanded.is_empty() || expanded.contains('$') || !Path::new(&expanded).is_absolute()
            {
                return Err(unavailable("unsupported ELF search path"));
            }
            match std::fs::canonicalize(&expanded) {
                Ok(dir) if dir.starts_with("/usr/lib/") || dir == Path::new("/usr/lib") => {
                    search.push(dir)
                }
                Ok(_) => return Err(unavailable("ELF search path outside /usr/lib")),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(unavailable(e)),
            }
        }
    }
    let inherited = if has_runpath { vec![] } else { rpath.clone() };
    let search = if has_runpath { runpath } else { rpath };
    Ok(Some(ElfRequirements {
        needed,
        search,
        inherited,
        has_runpath,
        has_search_path,
    }))
}

fn verify_elf_coverage(
    payload: &std::collections::BTreeMap<String, (String, u32)>,
    system: &[RuntimeFile],
) -> Result<(), String> {
    match std::fs::symlink_metadata("/etc/ld.so.preload") {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => {
            return Err(unavailable(
                "system dynamic loader preload is not supported",
            ));
        }
    }
    for path in ["/lib", "/lib64", "/usr/lib64"] {
        match std::fs::canonicalize(path) {
            Ok(dir) if dir == Path::new("/usr/lib") => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && path == "/usr/lib64" => {}
            _ => return Err(unavailable("unsupported system loader search layout")),
        }
    }
    let expected: std::collections::BTreeSet<PathBuf> =
        system.iter().map(|v| PathBuf::from(&v.path)).collect();
    if !expected.contains(Path::new(ELF_LOADER)) {
        return Err(unavailable("dynamic loader absent from system inventory"));
    }
    // bwrap runs before the private mount namespace exists. Ensure every
    // dependency in its /usr/lib graph is present before executing the loader.
    let mut prelaunch_dirs = vec![PathBuf::from("/usr/lib")];
    match std::fs::read_dir("/usr/lib/glibc-hwcaps") {
        Ok(entries) => {
            for entry in entries {
                let dir = std::fs::canonicalize(entry.map_err(unavailable)?.path())
                    .map_err(unavailable)?;
                if !dir.starts_with("/usr/lib/") || !dir.is_dir() {
                    return Err(unavailable("invalid prelaunch hwcaps directory"));
                }
                trusted_path(&dir)?;
                prelaunch_dirs.push(dir);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(unavailable(e)),
    }
    let mut pending = vec![PathBuf::from(BWRAP)];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let name = path
            .to_str()
            .ok_or_else(|| unavailable("invalid bwrap dependency path"))?;
        let info = elf_requirements(name, path == Path::new(BWRAP))?
            .ok_or_else(|| unavailable("bwrap dependency is not dynamic ELF"))?;
        if info.has_search_path {
            return Err(unavailable("unsupported bwrap prelaunch ELF search path"));
        }
        for needed in info.needed {
            let default = std::fs::canonicalize(Path::new("/usr/lib").join(&needed))
                .map_err(|_| unavailable(format!("bwrap system dependency missing: {needed}")))?;
            for directory in &prelaunch_dirs {
                let candidate = directory.join(&needed);
                let dependency = match std::fs::canonicalize(candidate) {
                    Ok(value) => value,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(unavailable(e)),
                };
                if !expected.contains(&dependency) {
                    return Err(unavailable(format!(
                        "bwrap system dependency absent from inventory: {}",
                        dependency.display()
                    )));
                }
                pending.push(dependency);
            }
            if !expected.contains(&default) {
                return Err(unavailable(
                    "bwrap default dependency absent from inventory",
                ));
            }
        }
    }
    let mut objects = Vec::new();
    let mut inherited = std::collections::BTreeSet::new();
    let mut dynamic_files = std::collections::BTreeSet::new();
    for path in payload.keys().chain(system.iter().map(|v| &v.path)) {
        let entry = ELF_ENTRY_POINTS.contains(&path.as_str());
        if let Some(info) = elf_requirements(path, entry)? {
            dynamic_files.insert(PathBuf::from(path));
            inherited.extend(info.inherited.iter().cloned());
            objects.push((path, info));
        } else if expected.contains(Path::new(path)) {
            return Err(unavailable("system inventory entry is not a dynamic ELF"));
        }
    }

    fn expanded_dirs(
        dirs: &std::collections::BTreeSet<PathBuf>,
    ) -> Result<std::collections::BTreeSet<PathBuf>, String> {
        let mut candidates = dirs.clone();
        for dir in dirs {
            trusted_path(dir)?;
            let hwcaps = dir.join("glibc-hwcaps");
            match std::fs::read_dir(&hwcaps) {
                Ok(entries) => {
                    for entry in entries {
                        let dir = std::fs::canonicalize(entry.map_err(unavailable)?.path())
                            .map_err(unavailable)?;
                        if !dir.starts_with("/usr/lib/") || !dir.is_dir() {
                            return Err(unavailable("invalid glibc hwcaps directory"));
                        }
                        trusted_path(&dir)?;
                        candidates.insert(dir);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(unavailable(e)),
            }
        }
        Ok(candidates)
    }

    let inherited = expanded_dirs(&inherited)?;
    for (requester, info) in objects {
        let mut own = std::collections::BTreeSet::from([PathBuf::from("/usr/lib")]);
        own.extend(info.search);
        // Hwcaps selection depends on the executing CPU. Inventory every
        // alternative, but require an ordinary search-path match for portability.
        let own_base = own;
        let own = expanded_dirs(&own_base)?;
        let mut candidates = own.clone();
        if !info.has_runpath {
            candidates.extend(inherited.iter().cloned());
        }
        for needed in info.needed {
            let mut found_own = false;
            for dir in &candidates {
                let path = match std::fs::canonicalize(dir.join(&needed)) {
                    Ok(path) => path,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(unavailable(e)),
                };
                if own_base.contains(dir) {
                    found_own = true;
                }
                if path.starts_with("/usr/lib/archy/runtime/") {
                    let name = path
                        .to_str()
                        .ok_or_else(|| unavailable("invalid ELF dependency path"))?;
                    if !payload.contains_key(name) || !dynamic_files.contains(&path) {
                        return Err(unavailable("ELF package dependency absent or not dynamic"));
                    }
                } else if !expected.contains(&path) {
                    return Err(unavailable(format!(
                        "ELF system dependency absent from inventory: {}",
                        path.display()
                    )));
                }
            }
            if !found_own {
                return Err(unavailable(format!(
                    "ELF requester {requester} has no dependency in ordinary search directories: {needed}"
                )));
            }
        }
    }
    Ok(())
}

fn inventory(backend: Backend) -> Result<(), String> {
    let m =
        std::fs::metadata(INVENTORY).map_err(|e| unavailable(format!("runtime inventory: {e}")))?;
    if !root_owned(m.uid()) || m.mode() & 0o022 != 0 || m.len() > 16 * 1024 * 1024 {
        return Err(unavailable("runtime inventory is mutable or oversized"));
    }
    trusted_path(Path::new(INVENTORY))?;
    let data = std::fs::read(INVENTORY).map_err(unavailable)?;
    let inventory: Inventory = serde_json::from_slice(&data).map_err(unavailable)?;
    if inventory.schema_version != 2 || inventory.files.is_empty() {
        return Err(unavailable("runtime inventory version"));
    }
    let payload = runtime_payload(Path::new("/usr/lib/archy/runtime"))?;
    let mut required = std::collections::BTreeSet::from([BWRAP]);
    match backend {
        Backend::Blender => {
            required.insert("/usr/lib/archy/runtime/blender/blender");
            required.insert("/usr/lib/archy/runtime/workers/blender/worker.py");
        }
        Backend::Ifc => {
            required.insert("/usr/lib/archy/runtime/python/bin/python3.12");
            required.insert("/usr/lib/archy/runtime/workers/ifc/archy_ifc_worker/worker.py");
            required.insert("/usr/lib/archy/runtime/node/bin/node");
            required.insert("/usr/lib/archy/runtime/ifc-verifier/reader.cjs");
            required.insert("/usr/lib/archy/runtime/ifc-verifier/verify-stdin.cjs");
            required.insert("/usr/lib/archy/runtime/ifc-verifier/step-scope.cjs");
            required.insert(
                "/usr/lib/archy/runtime/ifc-verifier/node_modules/web-ifc/web-ifc-api-node.js",
            );
            required.insert(
                "/usr/lib/archy/runtime/ifc-verifier/node_modules/web-ifc/web-ifc-node.wasm",
            );
            required
                .insert("/usr/lib/archy/runtime/ifc-verifier/node_modules/web-ifc/package.json");
        }
    }
    for (entries, prefix) in [
        (&inventory.files, "/usr/lib/archy/runtime/"),
        (&inventory.system_files, "/usr/lib/"),
    ] {
        let mut previous = "";
        for entry in entries {
            let path = entry.path.as_str();
            if path <= previous
                || !path.starts_with(prefix)
                || (prefix == "/usr/lib/" && path.starts_with("/usr/lib/archy/runtime/"))
                || path == INVENTORY
                || path.split('/').any(|part| part == "." || part == "..")
                || !matches!(entry.mode, 0o644 | 0o755)
                || entry.sha256.len() != 64
                || !entry
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(unavailable("invalid runtime inventory entry"));
            }
            if prefix == "/usr/lib/archy/runtime/" {
                if payload.get(path) != Some(&(entry.sha256.clone(), entry.mode)) {
                    return Err(unavailable(format!(
                        "runtime digest or mode mismatch: {path}"
                    )));
                }
            } else {
                trusted_file(path, &entry.sha256)?;
                let mode = std::fs::metadata(path).map_err(unavailable)?.mode() & 0o7777;
                if mode != entry.mode {
                    return Err(unavailable(format!("runtime mode mismatch: {path}")));
                }
            }
            required.remove(path);
            previous = path;
        }
    }
    if !required.is_empty() {
        return Err(unavailable("incomplete runtime inventory"));
    }
    if payload.len() != inventory.files.len() {
        return Err(unavailable("runtime payload differs from inventory"));
    }
    verify_elf_coverage(&payload, &inventory.system_files)?;
    let gate = std::fs::metadata(GATE).map_err(unavailable)?;
    if !root_owned(gate.uid()) || gate.mode() & 0o022 != 0 || !gate.is_file() {
        return Err(unavailable("packaged gate identity"));
    }
    Ok(())
}

fn validate_scratch(path: &Path) -> Result<File, String> {
    private_path(path)?;
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        fs::Mode::empty(),
    )
    .map_err(unavailable)?;
    let f: File = fd.into();
    let m = f.metadata().map_err(unavailable)?;
    if !m.is_dir() || m.uid() != getuid().as_raw() || m.mode() & 0o7777 != 0o700 {
        return Err(unavailable("scratch ownership/mode"));
    }
    if fs::fstatfs(&f).map_err(unavailable)?.f_type as u64 != 0x0102_1994 {
        return Err(unavailable("scratch is not tmpfs"));
    }
    let s = fs::fstatvfs(&f).map_err(unavailable)?;
    let bytes = s
        .f_blocks
        .checked_mul(s.f_frsize)
        .ok_or_else(|| unavailable("scratch quota overflow"))?;
    if bytes == 0
        || bytes > SCRATCH_BYTES
        || s.f_files == 0
        || s.f_files > SCRATCH_INODES
        || !s.f_flag.contains(
            StatVfsMountFlags::NOSUID | StatVfsMountFlags::NODEV | StatVfsMountFlags::NOEXEC,
        )
    {
        return Err(unavailable("scratch byte/inode quota or mount flags"));
    }
    Ok(f)
}

fn verify_cgroup(id: &str) -> Result<(), String> {
    let cgroup = std::fs::read_to_string("/proc/self/cgroup").map_err(unavailable)?;
    let group = cgroup
        .trim()
        .strip_prefix("0::")
        .ok_or_else(|| unavailable("no cgroup v2 membership"))?;
    if !group.starts_with("/user.slice/")
        || !group.ends_with(&format!("/archy-job-{id}.service"))
        || group.contains("..")
    {
        return Err(unavailable("service cgroup identity mismatch"));
    }
    let root = Path::new("/sys/fs/cgroup").join(group.trim_start_matches('/'));
    for (key, expected) in [
        ("memory.max", "2147483648"),
        ("memory.swap.max", "0"),
        ("pids.max", "64"),
    ] {
        if std::fs::read_to_string(root.join(key))
            .map_err(unavailable)?
            .trim()
            != expected
        {
            return Err(unavailable(format!("effective {key}")));
        }
    }
    let cpu = std::fs::read_to_string(root.join("cpu.max")).map_err(unavailable)?;
    let parts: Vec<_> = cpu.split_whitespace().collect();
    if parts.len() != 2
        || parts[0].parse::<u64>().ok().filter(|n| *n > 0) != parts[1].parse::<u64>().ok()
    {
        return Err(unavailable("effective cpu.max"));
    }
    Ok(())
}

fn send_scratch(sock: &UnixDatagram, scratch: &File) -> Result<(), String> {
    if sockopt::socket_type(sock).map_err(unavailable)? != SocketType::SEQPACKET {
        return Err(unavailable("control FD is not seqpacket"));
    }
    if sockopt::socket_peercred(sock).map_err(unavailable)?.uid != getuid() {
        return Err(unavailable("control peer UID mismatch"));
    }
    let fds = [scratch.as_fd()];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !ancillary.push(SendAncillaryMessage::ScmRights(&fds)) {
        return Err(unavailable("ancillary capacity"));
    }
    if sendmsg(
        sock,
        &[IoSlice::new(FRAME)],
        &mut ancillary,
        SendFlags::empty(),
    )
    .map_err(unavailable)?
        != FRAME.len()
    {
        return Err(unavailable("short scratch frame"));
    }
    Ok(())
}

fn receive_request(
    sock: &UnixDatagram,
    backend: Backend,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    if sockopt::socket_type(sock).map_err(unavailable)? != SocketType::SEQPACKET {
        return Err(unavailable("control FD is not seqpacket"));
    }
    // A zero-length seqpacket also reads as zero bytes. Credentials distinguish it from EOF.
    sockopt::set_socket_passcred(sock, true).map_err(unavailable)?;
    sock.set_read_timeout(Some(timeout)).map_err(unavailable)?;
    let mut token = [0; 3];
    if sock.recv(&mut token).map_err(unavailable)? != 2 || &token[..2] != b"GO" {
        return Err(unavailable("missing supervisor GO"));
    }
    let mut frame = vec![0; REQUEST_FRAME.len() + 1024 * 1024 + 1];
    let n = sock.recv(&mut frame).map_err(unavailable)?;
    if n <= REQUEST_FRAME.len()
        || n > REQUEST_FRAME.len() + 1024 * 1024
        || !frame[..n].starts_with(REQUEST_FRAME)
    {
        return Err(unavailable("missing or oversized typed request frame"));
    }
    let raw = frame[REQUEST_FRAME.len()..n].to_vec();
    validate_request(&raw, backend)?;
    let mut byte = [0; 1];
    let mut iov = [IoSliceMut::new(&mut byte)];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let tail = recvmsg(sock, &mut iov, &mut ancillary, RecvFlags::CMSG_CLOEXEC)
        .map_err(|e| unavailable(format!("request channel EOF: {e}")))?;
    if tail.bytes != 0
        || tail
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        || ancillary.drain().next().is_some()
    {
        return Err(unavailable("trailing request packet"));
    }
    Ok(raw)
}

fn validate_request(raw: &[u8], backend: Backend) -> Result<(), String> {
    let unique: UniqueJson = serde_json::from_slice(raw).map_err(unavailable)?;
    if !unique.0.is_object() {
        return Err(unavailable("request root is not an object"));
    }
    let req: Request = serde_json::from_slice(raw).map_err(unavailable)?;
    let output = match backend {
        Backend::Blender => "output",
        Backend::Ifc => "output/ifc",
    };
    if req.schema_version != backend.schema()
        || req.operation != backend.operation()
        || req.output_dir != output
        || req.source_artifacts.as_ref().is_some_and(|a| !a.is_empty())
        || [req.request_id, req.project_id, req.variant_id, req.run_id]
            .iter()
            .any(|s| !valid_id(s))
    {
        return Err(unavailable(
            "unsupported typed operation, identifier or output policy",
        ));
    }
    let expected: &[&str] = match backend {
        Backend::Blender => &["params", "site", "input_digest", "preset_id"],
        Backend::Ifc => &["variant", "site", "source_digest"],
    };
    if req.extra.len() != expected.len() || expected.iter().any(|key| !req.extra.contains_key(*key))
    {
        return Err(unavailable("unexpected worker request fields"));
    }
    if backend == Backend::Blender
        && req.extra.get("preset_id").and_then(|v| v.as_str()) != Some("smoke-cpu-v1")
    {
        return Err(unavailable("unsupported render preset"));
    }
    Ok(())
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn sealed_request(raw: &[u8]) -> Result<File, String> {
    let fd = fs::memfd_create(
        "archy-request",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(unavailable)?;
    let mut file: File = fd.into();
    file.write_all(raw).map_err(unavailable)?;
    fs::fcntl_add_seals(
        &file,
        SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE,
    )
    .map_err(unavailable)?;
    fcntl_setfd(&file, FdFlags::empty()).map_err(unavailable)?;
    Ok(file)
}

fn launch(backend: Backend, scratch: &File, request: &File) -> Result<(), String> {
    fcntl_setfd(scratch, FdFlags::empty()).map_err(unavailable)?;
    let args = sandbox_argv(backend, scratch.as_raw_fd(), request.as_raw_fd());
    let status = Command::new(ELF_LOADER)
        .args(["--inhibit-cache", "--library-path", "/usr/lib", BWRAP])
        .args(args)
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(unavailable)?;
    if status.code() == Some(20) {
        std::process::exit(20);
    }
    if !status.success() {
        return Err(unavailable(format!("sandbox/worker exited {status}")));
    }
    Ok(())
}

fn sandbox_argv(backend: Backend, scratch_fd: i32, request_fd: i32) -> Vec<String> {
    let mut a = [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--dir",
        "/etc",
        "--setenv",
        "LC_ALL",
        "C",
        "--setenv",
        "HOME",
        "/job/output",
        "--setenv",
        "TMPDIR",
        "/job/output",
        "--setenv",
        "XDG_CACHE_HOME",
        "/job/output",
        "--setenv",
        "PYTHONNOUSERSITE",
        "1",
        "--ro-bind",
        "/usr/lib",
        "/usr/lib",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib",
        "/lib64",
        "--dir",
        "/runtime",
        "--ro-bind",
        GATE,
        "/runtime/gate",
        "--proc",
        "/proc",
        "--dir",
        "/dev",
        "--dev-bind",
        "/dev/null",
        "/dev/null",
        "--dev-bind",
        "/dev/zero",
        "/dev/zero",
        "--dev-bind",
        "/dev/urandom",
        "/dev/urandom",
        "--dir",
        "/job",
        "--dir",
        "/job/input",
        "--dir",
        "/job/output",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect::<Vec<_>>();
    for ns in ["user", "mnt", "pid", "net", "ipc", "uts"] {
        // These values are service-owned observations, never caller input.
        let value = std::fs::read_link(format!("/proc/self/ns/{ns}")).ok();
        if let Some(value) = value {
            a.extend([
                "--setenv".into(),
                format!("ARCHY_HOST_NS_{}", ns.to_ascii_uppercase()),
                value.to_string_lossy().into_owned(),
            ]);
        }
    }
    a.extend([
        "--ro-bind-fd".into(),
        request_fd.to_string(),
        "/job/input/request.json".into(),
        "--bind-fd".into(),
        scratch_fd.to_string(),
        "/job/output".into(),
        "--remount-ro".into(),
        "/".into(),
        "--".into(),
        "/runtime/gate".into(),
        "--inside".into(),
        match backend {
            Backend::Blender => "blender",
            Backend::Ifc => "ifc",
        }
        .into(),
    ]);
    a
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 2
        && let Some(backend) = args[1].strip_prefix("--audit-runtime=")
    {
        return inventory(Backend::parse(backend)?);
    }
    if args.len() == 3 && args[1] == "--inside" {
        return inside(Backend::parse(&args[2])?);
    }
    if args.len() != 3 {
        return Err(unavailable("fixed gate takes backend and scratch only"));
    }
    let backend = Backend::parse(
        args[1]
            .strip_prefix("--backend=")
            .ok_or_else(|| unavailable("backend flag"))?,
    )?;
    let scratch = PathBuf::from(
        args[2]
            .strip_prefix("--scratch=")
            .ok_or_else(|| unavailable("scratch flag"))?,
    );
    let id = scratch
        .to_str()
        .and_then(|s| s.strip_prefix("/tmp/archy-job-"))
        .and_then(|s| s.strip_suffix("/scratch"))
        .ok_or_else(|| unavailable("scratch identity"))?;
    if id.len() != 32
        || !id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(unavailable("scratch job ID"));
    }
    verify_cgroup(id)?;
    let scratch_file = validate_scratch(&scratch)?;
    // systemd-run forwards its private seqpacket peer as service stdin.
    let sock = unsafe { UnixDatagram::from_raw_fd(0) };
    send_scratch(&sock, &scratch_file)?;
    let request = receive_request(&sock, backend, Duration::from_secs(8))?;
    drop(sock);
    inventory(backend)?;
    let request = sealed_request(&request)?;
    launch(backend, &scratch_file, &request)
}

fn inside(backend: Backend) -> Result<(), String> {
    if !Path::new("/job/input/request.json").is_file() || !Path::new("/job/output").is_dir() {
        return Err(unavailable("sandbox mounts missing"));
    }
    for readonly in ["/", "/usr/lib", "/job/input/request.json"] {
        if !fs::statvfs(readonly)
            .map_err(unavailable)?
            .f_flag
            .contains(StatVfsMountFlags::RDONLY)
        {
            return Err(unavailable(format!("sandbox mount writable: {readonly}")));
        }
    }
    for path in ["/etc/ld.so.cache", "/etc/ld.so.preload"] {
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(unavailable(format!(
                    "sandbox loader configuration visible: {path}"
                )));
            }
        }
    }
    for ns in ["user", "mnt", "pid", "net", "ipc", "uts"] {
        let key = format!("ARCHY_HOST_NS_{}", ns.to_ascii_uppercase());
        let host = std::env::var(&key).map_err(unavailable)?;
        let current = std::fs::read_link(format!("/proc/self/ns/{ns}")).map_err(unavailable)?;
        if current.to_string_lossy() == host {
            return Err(unavailable(format!("{ns} namespace not isolated")));
        }
    }
    if fs::statfs("/job/output").map_err(unavailable)?.f_type as u64 != 0x0102_1994 {
        return Err(unavailable("sandbox output is not tmpfs"));
    }
    let output = fs::statvfs("/job/output").map_err(unavailable)?;
    let capacity = output
        .f_blocks
        .checked_mul(output.f_frsize)
        .ok_or_else(|| unavailable("sandbox quota overflow"))?;
    if capacity == 0
        || capacity > SCRATCH_BYTES
        || output.f_files == 0
        || output.f_files > SCRATCH_INODES
        || !output.f_flag.contains(
            StatVfsMountFlags::NOSUID | StatVfsMountFlags::NODEV | StatVfsMountFlags::NOEXEC,
        )
    {
        return Err(unavailable("sandbox output quota or mount flags"));
    }
    for forbidden in ["/home", "/run/user", "/sys/fs/cgroup", "/dev/dri"] {
        if Path::new(forbidden).exists() {
            return Err(unavailable(format!("host resource visible: {forbidden}")));
        }
    }
    let status = std::fs::read_to_string("/proc/self/status").map_err(unavailable)?;
    if !status
        .lines()
        .any(|line| line == "CapEff:\t0000000000000000")
    {
        return Err(unavailable("sandbox capabilities remain"));
    }
    let extras = std::fs::read_dir("/proc/self/fd")
        .map_err(unavailable)?
        .map(|entry| {
            entry
                .map_err(unavailable)?
                .file_name()
                .to_string_lossy()
                .parse::<i32>()
                .map_err(unavailable)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for fd in extras.into_iter().filter(|fd| *fd > 2) {
        unsafe {
            drop(File::from_raw_fd(fd));
        }
    }
    let argv: Vec<&str> = match backend {
        Backend::Blender => vec![
            "--background",
            "--factory-startup",
            "--disable-autoexec",
            "--offline-mode",
            "--python-exit-code",
            "20",
            "--threads",
            "1",
            "--python",
            "/usr/lib/archy/runtime/workers/blender/worker.py",
            "--",
            "--request",
            "/job/input/request.json",
            "--response",
            "/job/output/response.json",
        ],
        Backend::Ifc => vec![
            "-I",
            "-m",
            "archy_ifc_worker",
            "--project-root",
            "/job",
            "--request",
            "input/request.json",
        ],
    };
    let program = match backend {
        Backend::Blender => "/usr/lib/archy/runtime/blender/blender",
        Backend::Ifc => "/usr/lib/archy/runtime/python/bin/python3.12",
    };
    let status = Command::new(program)
        .args(argv)
        .env_clear()
        .env("LC_ALL", "C")
        .env("HOME", "/job/output")
        .env("TMPDIR", "/job/output")
        .env("XDG_CACHE_HOME", "/job/output")
        .env("PYTHONNOUSERSITE", "1")
        .current_dir("/job")
        .status()
        .map_err(unavailable)?;
    let expected = expected_response_status(status.code())?;
    let response = match backend {
        Backend::Blender => "/job/output/response.json",
        Backend::Ifc => "/job/output/ifc/response.json",
    };
    let meta = std::fs::symlink_metadata(response).map_err(unavailable)?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.nlink() != 1
        || meta.len() == 0
        || meta.len() > 1024 * 1024
    {
        return Err(unavailable("bounded worker response absent"));
    }
    let file = File::open(response).map_err(unavailable)?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(unavailable)?;
    if bytes.len() as u64 != meta.len() {
        return Err(unavailable("worker response changed"));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(unavailable)?;
    if value.get("status").and_then(serde_json::Value::as_str) != Some(expected) {
        return Err(unavailable("worker exit and response status disagree"));
    }
    if status.code() == Some(20) {
        std::process::exit(20);
    }
    Ok(())
}

fn expected_response_status(code: Option<i32>) -> Result<&'static str, String> {
    match code {
        Some(0) => Ok("ok"),
        Some(20) => Ok("error"),
        _ => Err(unavailable("worker exited without a handled response")),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(78);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_elf_string_tables_are_read_by_bounded_reference() {
        let path = std::env::temp_dir().join(format!("archy-elf-large-{}", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let strings_size: u64 = 21_897_354;
        let strings_offset: u64 = 4096;
        let total = strings_offset + strings_size;
        file.set_len(total).unwrap();
        let mut header = [0u8; 64];
        header[..6].copy_from_slice(b"\x7fELF\x02\x01");
        header[6] = 1;
        header[16..18].copy_from_slice(&3u16.to_le_bytes());
        header[18..20].copy_from_slice(&62u16.to_le_bytes());
        header[32..40].copy_from_slice(&64u64.to_le_bytes());
        header[54..56].copy_from_slice(&56u16.to_le_bytes());
        header[56..58].copy_from_slice(&2u16.to_le_bytes());
        file.write_all_at(&header, 0).unwrap();
        let mut ph = [0u8; 112];
        ph[..4].copy_from_slice(&1u32.to_le_bytes());
        ph[32..40].copy_from_slice(&total.to_le_bytes());
        ph[40..48].copy_from_slice(&total.to_le_bytes());
        ph[56..60].copy_from_slice(&2u32.to_le_bytes());
        ph[64..72].copy_from_slice(&176u64.to_le_bytes());
        ph[72..80].copy_from_slice(&176u64.to_le_bytes());
        ph[88..96].copy_from_slice(&64u64.to_le_bytes());
        ph[96..104].copy_from_slice(&64u64.to_le_bytes());
        file.write_all_at(&ph, 64).unwrap();
        let mut dynamic = [0u8; 64];
        for (i, (tag, value)) in [(5u64, strings_offset), (10, strings_size), (1, 1), (0, 0)]
            .into_iter()
            .enumerate()
        {
            dynamic[i * 16..i * 16 + 8].copy_from_slice(&tag.to_le_bytes());
            dynamic[i * 16 + 8..i * 16 + 16].copy_from_slice(&value.to_le_bytes());
        }
        file.write_all_at(&dynamic, 176).unwrap();
        file.write_all_at(b"\0libfixture.so\0", strings_offset)
            .unwrap();
        let info = elf_requirements(path.to_str().unwrap(), false)
            .unwrap()
            .unwrap();
        assert_eq!(info.needed, ["libfixture.so"]);
        file.write_all_at(&177u64.to_le_bytes(), 64 + 56 + 16)
            .unwrap();
        assert!(
            elf_requirements(path.to_str().unwrap(), false).is_err(),
            "PT_DYNAMIC file offset must agree with PT_LOAD virtual mapping"
        );
        file.write_all_at(&176u64.to_le_bytes(), 64 + 56 + 16)
            .unwrap();
        file.write_all_at(&strings_size.to_le_bytes(), 176 + 2 * 16 + 8)
            .unwrap();
        assert!(elf_requirements(path.to_str().unwrap(), false).is_err());
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires the pinned Blender payload at the fixed path in a disposable container"]
    fn pinned_blender_real_elf_segments_validate() {
        for path in [
            "/usr/lib/archy/runtime/blender/blender",
            "/usr/lib/archy/runtime/blender/lib/libopenvdb.so.12.0.0",
        ] {
            let info = elf_requirements(path, path.ends_with("/blender"))
                .unwrap()
                .unwrap();
            assert!(!info.needed.is_empty(), "{path}");
        }
    }

    #[test]
    fn handled_error_exit_is_distinct_from_success_and_crash() {
        assert_eq!(expected_response_status(Some(0)).unwrap(), "ok");
        assert_eq!(expected_response_status(Some(20)).unwrap(), "error");
        assert!(expected_response_status(Some(1)).is_err());
        assert!(expected_response_status(None).is_err());
    }
    fn valid_frame() -> Vec<u8> {
        let mut frame = REQUEST_FRAME.to_vec();
        frame.extend(serde_json::json!({"schema_version":"blender-worker-v1","operation":"build_massing",
            "request_id":"r1","project_id":"p1","variant_id":"variant-abc","run_id":"run1",
            "output_dir":"output","params":{},"site":{},"input_digest":"x","preset_id":"smoke-cpu-v1"}).to_string().as_bytes());
        frame
    }
    fn pair() -> (UnixDatagram, UnixDatagram) {
        let (left, right) = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            SocketType::SEQPACKET,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        (UnixDatagram::from(left), UnixDatagram::from(right))
    }
    #[test]
    fn exact_request_then_eof() {
        let (left, right) = pair();
        right.send(b"GO").unwrap();
        right.send(&valid_frame()).unwrap();
        drop(right);
        assert!(receive_request(&left, Backend::Blender, Duration::from_millis(40)).is_ok());
    }
    #[test]
    fn trailing_request_packet_is_rejected() {
        let (left, right) = pair();
        right.send(b"GO").unwrap();
        right.send(&valid_frame()).unwrap();
        right.send(b"EXTRA-FRAME").unwrap();
        drop(right);
        assert!(receive_request(&left, Backend::Blender, Duration::from_millis(40)).is_err());
    }
    #[test]
    fn open_peer_after_request_is_rejected() {
        let (left, right) = pair();
        right.send(b"GO").unwrap();
        right.send(&valid_frame()).unwrap();
        assert!(receive_request(&left, Backend::Blender, Duration::from_millis(40)).is_err());
    }
    #[test]
    fn empty_trailing_packet_is_rejected() {
        let (left, right) = pair();
        right.send(b"GO").unwrap();
        right.send(&valid_frame()).unwrap();
        right.send(b"").unwrap();
        drop(right);
        assert!(receive_request(&left, Backend::Blender, Duration::from_millis(40)).is_err());
    }
    #[test]
    fn request_fields_are_fixed() {
        assert!(!valid_id("../../etc/passwd"));
        assert!(!valid_id("x$(id)"));
        let argv = sandbox_argv(Backend::Blender, 4, 5);
        assert!(argv.contains(&"--unshare-net".to_string()));
        assert!(
            !argv
                .iter()
                .any(|s| s == "/bin/sh" || s == "-c" || s == "--bind-try")
        );
    }
    #[test]
    fn no_request_after_go_is_unavailable() {
        let (left, right) = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            SocketType::SEQPACKET,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let left = UnixDatagram::from(left);
        let right = UnixDatagram::from(right);
        right.send(b"GO").unwrap();
        drop(right);
        assert!(receive_request(&left, Backend::Blender, Duration::from_millis(40)).is_err());
    }
    #[test]
    fn typed_request_rejects_path_and_operation_forgeries() {
        let base = serde_json::json!({"schema_version":"blender-worker-v1","operation":"build_massing",
            "request_id":"r1","project_id":"p1","variant_id":"variant-abc","run_id":"run1",
            "output_dir":"output","params":{},"site":{},"input_digest":"x","preset_id":"smoke-cpu-v1"});
        assert!(validate_request(base.to_string().as_bytes(), Backend::Blender).is_ok());
        for (field, value) in [
            ("operation", "execute_shell"),
            ("output_dir", "../../home"),
            ("preset_id", "custom"),
            ("run_id", "$(id)"),
        ] {
            let mut bad = base.clone();
            bad[field] = serde_json::json!(value);
            assert!(
                validate_request(bad.to_string().as_bytes(), Backend::Blender).is_err(),
                "{field}"
            );
        }
        assert!(validate_request(br#"{"schema_version":"blender-worker-v1","operation":"build_massing","operation":"build_massing"}"#, Backend::Blender).is_err());
        let nested_duplicate = base
            .to_string()
            .replace("\"params\":{}", "\"params\":{\"x\":1,\"x\":2}");
        assert!(validate_request(nested_duplicate.as_bytes(), Backend::Blender).is_err());
    }
}
