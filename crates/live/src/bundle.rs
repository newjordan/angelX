//! Immutable, content-addressed CommonJS generations. Every local dependency
//! is captured together; require never reads the changing source directory.
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[path = "bend.rs"]
mod bend;

const MAX_FILES: usize = 64;
/// A whole game's rules in Bend (sources and the compiled program together)
/// are a few hundred KiB; a generation past this is not a generation.
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub api_version: u32,
    pub id: String,
    pub kind: String,
    pub entry: String,
    #[serde(default)]
    pub probe: serde_json::Value,
    #[serde(default)]
    pub state_schema: u32,
    #[serde(default)]
    pub exports: Vec<crate::kernel::Export>,
    #[serde(default)]
    pub imports: Vec<crate::kernel::Import>,
    #[serde(default)]
    pub assets: Vec<String>,
    #[serde(default)]
    pub bend: Option<bend::Build>,
}

#[derive(Clone, Debug)]
pub(super) struct Bundle {
    pub manifest: Manifest,
    pub digest: String,
    pub build_error: Option<String>,
    files: BTreeMap<String, String>,
    pub assets: BTreeMap<String, std::sync::Arc<crate::Asset>>,
    stamp: SourceStamp,
}

impl Bundle {
    pub fn read(root: &Path, id: &str) -> Result<Self, String> {
        // Two matching reads reject a directory changing during capture. A
        // separate debounce in the registry also covers editor partial saves.
        let mut first = capture(root)?;
        let raw = first.get("module.json").ok_or("missing module.json")?;
        let manifest: Manifest = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if !matches!(manifest.api_version, 1 | 2) || manifest.id != id {
            return Err("module id or host API version differs".into());
        }
        if !matches!(
            manifest.kind.as_str(),
            "text" | "realm" | "agent" | "ui" | "game" | "tool-router" | "policy" | "system"
        ) {
            return Err("unsupported live module kind".into());
        }
        if (manifest.kind == "system") != (manifest.api_version == 2) {
            return Err("system modules require API 2; projections require API 1".into());
        }
        if manifest.kind == "system" {
            manifest.system_manifest().validate()?;
        }
        if matches!(id, "realm" | "agent" | "ui" | "game" | "tool-router") && manifest.kind != id {
            return Err("built-in module kind differs from its host contract".into());
        }
        if !first.contains_key(&manifest.entry) || !manifest.entry.ends_with(".js") {
            return Err("entry must name a captured JavaScript file".into());
        }
        let before = SourceStamp::read(root, &manifest.assets)?;
        if first != capture(root)? {
            return Err("module changed while its generation was being captured".into());
        }
        let assets = capture_assets(root, &manifest.assets)?;
        if assets != capture_assets(root, &manifest.assets)? {
            return Err("module assets changed during capture".into());
        }
        let build_error = manifest.bend.as_ref().and_then(|build| {
            if build.output == manifest.entry {
                Some("Bend output must be separate from the adapter entry".into())
            } else {
                bend::prepare(build, &mut first).err()
            }
        });
        let stamp = SourceStamp::read(root, &manifest.assets)?;
        if before != stamp {
            return Err("module inputs changed during capture".into());
        }
        let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        for (name, source) in &first {
            hash.update(&(name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update(&(source.len() as u64).to_le_bytes());
            hash.update(source.as_bytes());
        }
        for (name, asset) in &assets {
            hash.update(&(name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update(asset.id.as_bytes());
        }
        let digest = hash
            .finish()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self {
            manifest,
            digest,
            build_error,
            files: first,
            assets,
            stamp,
        })
    }

    /// Source and output digests of a Bend generation that built cleanly.
    pub fn build_receipt(&self) -> Option<crate::BuildReceipt> {
        self.manifest.bend.as_ref()?;
        self.build_error.is_none().then(|| bend::receipt(&self.files))?
    }

    /// A policy generation compiled into the host: the same manifest, Bend
    /// receipt and digest rules as a directory capture, from embedded bytes.
    pub fn from_files(id: &str, embedded: &[(&str, &str)]) -> Result<Self, String> {
        let mut files: BTreeMap<String, String> = embedded
            .iter()
            .map(|(name, body)| (name.to_string(), body.to_string()))
            .collect();
        if files.len() > MAX_FILES || files.values().map(String::len).sum::<usize>() > MAX_BYTES {
            return Err("module generation exceeds its file or byte budget".into());
        }
        let raw = files.get("module.json").ok_or("missing module.json")?;
        let manifest: Manifest = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if manifest.api_version != 1 || manifest.id != id || manifest.kind != "policy" {
            return Err("an embedded generation must be an API-1 policy module with its own id".into());
        }
        if !manifest.assets.is_empty() {
            return Err("an embedded policy declares no assets".into());
        }
        if !files.contains_key(&manifest.entry) || !manifest.entry.ends_with(".js") {
            return Err("entry must name a captured JavaScript file".into());
        }
        if let Some(build) = &manifest.bend {
            if build.output == manifest.entry {
                return Err("Bend output must be separate from the adapter entry".into());
            }
            bend::prepare(build, &mut files)?;
        }
        let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        for (name, source) in &files {
            hash.update(&(name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update(&(source.len() as u64).to_le_bytes());
            hash.update(source.as_bytes());
        }
        let digest = hash.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            manifest,
            digest,
            build_error: None,
            files,
            assets: BTreeMap::new(),
            stamp: SourceStamp(BTreeMap::new()),
        })
    }

    /// Metadata is a polling hint, never the generation identity. Explicit
    /// reload still captures and hashes bytes; changed inputs get two matching
    /// content reads and stable metadata before entering the debounce window.
    pub fn fresh(&self, root: &Path) -> bool {
        SourceStamp::read(root, &self.manifest.assets).is_ok_and(|stamp| {
            stamp.0.values().all(|file| file.modified.is_some()) && stamp == self.stamp
        })
    }

    fn bootstrap(&self, resource_json: &str) -> String {
        // JSON strings, rather than source interpolation, prevent a dependency
        // from breaking out of its own factory while the bundle is constructed.
        format!(
            r#"const files = {files};
const assets = {assets};
const dependencies = {resource_json};
function resource(module, name) {{
  if(typeof module!=='string'||typeof name!=='string') throw Error('resource requires module and name');
  const key=module+'/'+name;
  if(!Object.hasOwn(dependencies,key)) throw Error('resource is not imported: '+key);
  return Object.freeze(dependencies[key]);
}}
function asset(name) {{
  if(!Object.hasOwn(assets,name)) throw Error('asset not declared: '+name);
  return Object.freeze(assets[name]);
}}
const cache = Object.create(null);
function load(name) {{
  if (cache[name]) return cache[name].exports;
  if (!Object.hasOwn(files, name) || !name.endsWith('.js')) throw Error('missing dependency: '+name);
  const module = {{exports: {{}}}};
  cache[name] = module;
  const require = (specifier) => {{
    if (typeof specifier !== 'string' || !specifier.startsWith('.')) throw Error('use local relative dependencies');
    const parts = name.split('/').slice(0,-1);
    for (const part of specifier.split('/')) {{
      if (part === '' || part === '.') continue;
      if (part === '..') {{if (!parts.length) throw Error('dependency leaves module'); parts.pop();}}
      else parts.push(part);
    }}
    const path = parts.join('/');
    const resolved = [path, path+'.js', path+'/index.js'].find(p => Object.hasOwn(files,p));
    if (!resolved) throw Error('missing dependency: '+specifier);
    return load(resolved);
  }};
  new Function('module','exports','require','asset','resource','"use strict";\n'+files[name])(module,module.exports,require,asset,resource);
  return module.exports;
}}
const feature = load({entry});
"#,
            assets = serde_json::to_string(
                &self
                    .assets
                    .iter()
                    .map(|(name, asset)| (
                        name,
                        serde_json::json!({"id":asset.id,"bytes":asset.bytes.len()})
                    ))
                    .collect::<BTreeMap<_, _>>()
            )
            .unwrap(),
            files = serde_json::to_string(&self.files).expect("source JSON"),
            entry = serde_json::to_string(&self.manifest.entry).expect("entry JSON"),
        )
    }

    /// The module setup, a `CALL_MARKER` naming this generation's digest, then
    /// one render. An evaluator may keep the setup's context per digest and run
    /// only the call; run whole, the marker is an ordinary comment.
    pub fn script(&self, input: &serde_json::Value) -> String {
        format!(
            "{}\n{}{}*/\nif(feature.apiVersion!==1 || typeof feature.render!=='function') throw Error('expected apiVersion 1 and render(input)'); return {{value:await feature.render({input})}};",
            self.bootstrap("{}"),
            crate::CALL_MARKER,
            self.digest
        )
    }

    /// A system's module setup for one imported-resource set, ending in a
    /// `CALL_MARKER` that names this generation and that set. An evaluator may
    /// keep the setup's context per key and run only `system_call`; run whole,
    /// the marker is an ordinary comment. `resource_key` is
    /// `resource_identity(resource_json)`.
    pub fn system_setup(&self, resource_json: &str, resource_key: &str) -> String {
        format!(
            "{}\n{}{}:{}*/\n",
            self.bootstrap(resource_json),
            crate::CALL_MARKER,
            self.digest,
            resource_key
        )
    }

    /// One system method (`instantiate`, `prepare`, `rebind`, `instance`), run
    /// after `system_setup` in a scope of its own.
    pub fn system_call(
        &self,
        method: &str,
        previous: &serde_json::Value,
        event: &serde_json::Value,
    ) -> String {
        let schema = self.manifest.state_schema;
        let method = serde_json::to_string(method).unwrap();
        format!(
            r#"if(feature.apiVersion!==2 || typeof feature.reduce!=='function') throw Error('expected apiVersion 2 and reduce(state,event)');
const method={method}, previous={previous}, event={event};
let state;
if(method==='instantiate') {{
  if(typeof feature.instantiate!=='function') throw Error('system instantiate(context) is required');
  state=await feature.instantiate(event);
  return {{value:{{schema:{schema},state}}}};
}}
if(method==='prepare'||method==='rebind') {{
  if(method==='rebind') {{
    if(!previous||previous.schema!=={schema}) throw Error('resource rebind cannot change state schema');
    state=typeof feature.rebind==='function' ? await feature.rebind(previous.state,{{schema:{schema}}}) : previous.state;
  }} else if(previous===null) {{
    if(typeof feature.initialize!=='function') throw Error('system initialize(context) is required');
    state=await feature.initialize({{schema:{schema}}});
  }} else if(typeof feature.migrate==='function') {{
    state=await feature.migrate(previous,{{schema:{schema}}});
  }} else {{
    if(previous.schema!=={schema}) throw Error('state schema changed without migrate(checkpoint,context)');
    state=previous.state;
  }}
  const surfaces=typeof feature.view==='function' ? await feature.view(state) : {{}};
  const resources=typeof feature.resources==='function' ? await feature.resources(state) : {{}};
  return {{value:{{checkpoint:{{schema:{schema},state}},surfaces,resources}}}};
}}
const reducer=method==='instance' ? feature.reduceInstance : feature.reduce;
if(typeof reducer!=='function') throw Error('instance reducer is required');
const result=await reducer(previous.state,event);
if(!result || !Object.hasOwn(result,'state')) throw Error('reduce must return state, output and optional effects');
if(method==='instance') {{
  if(Object.keys(result).some(key=>key!=='state'&&key!=='output')) throw Error('instance returns state and adapter output only');
  return {{value:{{checkpoint:{{schema:{schema},state:result.state}},output:result.output??null}}}};
}}
const surfaces=typeof feature.view==='function' ? await feature.view(result.state) : {{}};
const resources=typeof feature.resources==='function' ? await feature.resources(result.state) : {{}};
return {{value:{{checkpoint:{{schema:{schema},state:result.state}},output:result.output??null,surfaces,resources,effects:result.effects??[]}}}};
"#
        )
    }
}

/// What an imported-resource set contributes to a setup's cache key.
pub fn resource_identity(resource_json: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, resource_json.as_bytes())
        .as_ref()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Manifest {
    pub fn system_manifest(&self) -> crate::kernel::Manifest {
        crate::kernel::Manifest {
            id: self.id.clone(),
            state_schema: self.state_schema,
            exports: self.exports.clone(),
            imports: self.imports.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64, u32),
}
impl FileStamp {
    fn read(path: &Path) -> Result<Self, String> {
        let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            created: meta.created().ok(),
            #[cfg(unix)]
            identity: (
                meta.dev(),
                meta.ino(),
                meta.ctime(),
                meta.ctime_nsec(),
                meta.mode(),
            ),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceStamp(BTreeMap<String, FileStamp>);
impl SourceStamp {
    fn read(root: &Path, assets: &[String]) -> Result<Self, String> {
        let mut stamp = BTreeMap::new();
        for (name, path) in inputs(root)? {
            stamp.insert(format!("source/{name}"), FileStamp::read(&path)?);
        }
        for (name, path) in asset_paths(root, assets)? {
            stamp.insert(format!("asset/{name}"), FileStamp::read(&path)?);
        }
        Ok(Self(stamp))
    }
}
#[cfg(test)]
thread_local! { static CAPTURE_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn captured_bytes() -> usize {
    CAPTURE_BYTES.with(|bytes| bytes.get())
}

fn capture(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    let mut bytes = 0;
    for (name, path) in inputs(root)? {
        let mut body = String::new();
        std::fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take((MAX_BYTES.saturating_sub(bytes) + 1) as u64)
            .read_to_string(&mut body)
            .map_err(|e| e.to_string())?;
        bytes += body.len();
        #[cfg(test)]
        CAPTURE_BYTES.with(|total| total.set(total.get() + body.len()));
        if bytes > MAX_BYTES {
            return Err(format!("module generation exceeds {} KiB", MAX_BYTES / 1024));
        }
        files.insert(name, body);
    }
    Ok(files)
}
fn inputs(root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let metadata = std::fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if !metadata.is_dir() {
        return Err("module must be a directory".into());
    }
    let mut files = BTreeMap::new();
    let mut entries = 0;
    visit(root, root, 0, &mut files, &mut entries)?;
    Ok(files)
}
fn visit(
    root: &Path,
    dir: &Path,
    depth: usize,
    files: &mut BTreeMap<String, PathBuf>,
    entries: &mut usize,
) -> Result<(), String> {
    if depth > 8 {
        return Err("module dependency directory is too deep".into());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        *entries += 1;
        if *entries > 256 {
            return Err("module dependency tree exceeds 256 entries".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_dir() {
            visit(root, &path, depth + 1, files, entries)?;
        } else if path.extension().is_some_and(|v| v == "js" || v == "bend")
            || path == root.join("module.json")
            || path == root.join("bend-build.json")
        {
            if !kind.is_file() {
                return Err("module inputs must be regular files".into());
            }
            if files.len() >= MAX_FILES {
                return Err("too many module files".into());
            }
            let name = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("module path is not UTF-8")?
                .replace('\\', "/");
            files.insert(name, path);
        }
    }
    Ok(())
}
fn asset_paths<'a>(root: &Path, names: &'a [String]) -> Result<Vec<(&'a str, PathBuf)>, String> {
    if names.len() > 64 {
        return Err("at most 64 assets per module".into());
    }
    let mut paths = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        let path = Path::new(name);
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || !seen.insert(name)
        {
            return Err("asset paths must be unique local relative files".into());
        }
        let mut full = root.to_path_buf();
        for component in path.components() {
            full.push(component);
            if std::fs::symlink_metadata(&full)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("asset inputs must not be symlinks".into());
            }
        }
        let meta = std::fs::metadata(&full).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > 2 * 1024 * 1024 {
            return Err("asset must be a regular file of at most 2 MiB".into());
        }
        paths.push((name.as_str(), full));
    }
    Ok(paths)
}

fn capture_assets(
    root: &Path,
    names: &[String],
) -> Result<BTreeMap<String, std::sync::Arc<crate::Asset>>, String> {
    let mut assets = BTreeMap::new();
    let mut total = 0;
    for (name, full) in asset_paths(root, names)? {
        let mut bytes = Vec::new();
        std::fs::File::open(full)
            .map_err(|e| e.to_string())?
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        total += bytes.len();
        #[cfg(test)]
        CAPTURE_BYTES.with(|total| total.set(total.get() + bytes.len()));
        if bytes.len() > 2 * 1024 * 1024 || total > 16 * 1024 * 1024 {
            return Err("module assets exceed their byte budget".into());
        }
        let id = ring::digest::digest(&ring::digest::SHA256, &bytes)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assets.insert(
            name.to_owned(),
            std::sync::Arc::new(crate::Asset { id, bytes }),
        );
    }
    Ok(assets)
}
