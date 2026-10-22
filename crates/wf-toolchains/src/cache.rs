//! The build cache: a fingerprint decides whether a module's toolchain must run at all.
//!
//! What a fingerprint covers is the plan's list (`plan.md` #68): the source bytes, the
//! toolchain's invocation contract, the manifest as a whole, the Host ABI label and the
//! WasmFoundry version. It is content hashing, never mtime: a touched-but-identical
//! file must not rebuild, and an edited file must.
//!
//! The store is one TOML file per project (`target/wf/cache.toml`). It is best effort:
//! a corrupt or unwritable store degrades to "no cache" (rebuild everything) and never
//! fails a build.

use std::collections::BTreeMap;
use std::path::Path;

use blake3::Hasher;

/// Hashes a sequence of parts into a hex fingerprint, length-prefixing each part so
/// `["ab", "c"]` and `["a", "bc"]` can never collide.
pub fn hash_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_string()
}

/// One module's last built state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// The fingerprint this module was built under.
    pub fingerprint: String,
    /// Modules this module binds to, each with the fingerprint it had at build time.
    /// A module whose own fingerprint is unchanged still rebuilds when one of these
    /// moved: the artifact it consumed came from code that changed.
    pub deps: Vec<(String, String)>,
}

impl Entry {
    /// Whether this entry's recorded dependencies are still fresh.
    pub fn deps_are_fresh(&self, current: &BTreeMap<String, String>) -> bool {
        self.deps
            .iter()
            .all(|(module, fingerprint)| current.get(module) == Some(fingerprint))
    }
}

/// The whole project's cache, keyed by module name.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Cache {
    pub entries: BTreeMap<String, Entry>,
}

impl Cache {
    /// The store file for a project root.
    fn store_path(root: &Path) -> std::path::PathBuf {
        root.join("target").join("wf").join("cache.toml")
    }

    /// Loads the store, returning an empty cache on any problem: a corrupt store is
    /// indistinguishable from a missing one (rebuild everything), and panicking in a
    /// build cache would be the tool failing the project it exists to build.
    pub fn load(root: &Path) -> Cache {
        let Ok(text) = std::fs::read_to_string(Self::store_path(root)) else {
            return Cache::default();
        };
        let Ok(value) = toml::from_str::<toml::Value>(&text) else {
            return Cache::default();
        };

        let mut entries = BTreeMap::new();
        if let Some(list) = value.get("entry").and_then(toml::Value::as_array) {
            for item in list {
                let Some(module) = item.get("module").and_then(toml::Value::as_str) else {
                    continue;
                };
                let Some(fingerprint) = item.get("fingerprint").and_then(toml::Value::as_str)
                else {
                    continue;
                };
                // No deps section means an empty list; a section with one bad record
                // makes the whole entry untrustworthy (see below).
                let deps = match item.get("deps") {
                    None => Some(Vec::new()),
                    Some(toml::Value::Array(records)) => {
                        let mut out = Vec::with_capacity(records.len());
                        for record in records {
                            let Some((name, fp)) = (match (
                                record.get("module").and_then(toml::Value::as_str),
                                record.get("fingerprint").and_then(toml::Value::as_str),
                            ) {
                                (Some(name), Some(fp)) => Some((name.to_owned(), fp.to_owned())),
                                _ => None,
                            }) else {
                                break;
                            };
                            out.push((name, fp));
                        }
                        (out.len() == records.len()).then_some(out)
                    }
                    Some(_) => None,
                };
                // An incomplete dep record cannot be verified on the next build, so the
                // artifact it was built from cannot be trusted to be fresh: drop the
                // whole entry rather than skip with unknown dependencies.
                let Some(deps) = deps else { continue };
                entries.insert(
                    module.to_owned(),
                    Entry {
                        fingerprint: fingerprint.to_owned(),
                        deps,
                    },
                );
            }
        }
        Cache { entries }
    }

    /// Saves the store, best effort: an unwritable `target/` will already have failed
    /// the build with a proper diagnostic before this runs.
    pub fn save(&self, root: &Path) {
        let mut text = String::new();
        for (module, entry) in &self.entries {
            // Module names come from the manifest and reach a filename; only quoting
            // makes an arbitrary name embeddable, and a name that does not fit a TOML
            // string simply does not get an entry (the next build recomputes it).
            let Some(name) = toml_str(module) else {
                continue;
            };
            let Some(fp) = toml_str(&entry.fingerprint) else {
                continue;
            };
            text.push_str("[[entry]]\n");
            text.push_str(&format!("module = {name}\n"));
            text.push_str(&format!("fingerprint = {fp}\n"));
            for (dep, dep_fp) in &entry.deps {
                if let (Some(dep), Some(dep_fp)) = (toml_str(dep), toml_str(dep_fp)) {
                    text.push_str(&format!("[[entry.deps]]\nmodule = {dep}\n"));
                    text.push_str(&format!("fingerprint = {dep_fp}\n"));
                }
            }
        }
        let path = Self::store_path(root);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("toml.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Renders a value as a TOML basic string, or `None` when it contains characters a
/// hand-written TOML writer cannot represent safely.
fn toml_str(value: &str) -> Option<String> {
    if value.is_empty()
        || value
            .bytes()
            .any(|b| b < 0x20 || b == 0x7f || b == b'"' || b == b'\\')
    {
        return None;
    }
    Some(format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_is_deterministic_and_input_sensitive() {
        let a = hash_hex(&[b"source", b"toolchain"]);
        let b = hash_hex(&[b"source", b"toolchain"]);
        assert_eq!(a, b, "the same inputs must produce the same fingerprint");
        // Length-prefixed parts do not merge across boundaries.
        assert_ne!(hash_hex(&[b"ab", b"c"]), hash_hex(&[b"a", b"bc"]));
        assert_ne!(hash_hex(&[b"source"]), hash_hex(&[b"toolchain"]));
        assert_ne!(hash_hex(&[]), hash_hex(&[b""]));
    }

    fn write_cache(dir: &Path, text: &str) {
        let path = Cache::store_path(dir);
        std::fs::create_dir_all(path.parent().unwrap()).expect("dirs");
        std::fs::write(path, text).expect("writable");
    }

    #[test]
    fn a_store_round_trips_with_its_dependency_list() {
        let dir = std::env::temp_dir().join(format!("wf-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = Cache::default();
        cache.entries.insert(
            "hello".to_owned(),
            Entry {
                fingerprint: "abc123".to_owned(),
                deps: vec![("helper".to_owned(), "def456".to_owned())],
            },
        );
        cache.save(&dir);

        let loaded = Cache::load(&dir);
        assert_eq!(loaded, cache);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An entry that does not declare deps must load: the dep section is optional,
    /// and losing such an entry would keep recompiling leaf modules forever (this
    /// exact shape had that bug once).
    #[test]
    fn an_entry_without_deps_section_loads() {
        let dir = std::env::temp_dir().join(format!("wf-cache-nodeps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_cache(
            &dir,
            "[[entry]]\nmodule = \"app\"\nfingerprint = \"1\"\n\
             [[entry.deps]]\nmodule = \"engine\"\nfingerprint = \"2\"\n\
             [[entry]]\nmodule = \"engine\"\nfingerprint = \"2\"\n",
        );
        let loaded = Cache::load(&dir);
        assert_eq!(loaded.entries.len(), 2, "{loaded:?}");
        assert_eq!(loaded.entries.get("engine").map(|e| e.deps.len()), Some(0));
        assert_eq!(loaded.entries.get("app").map(|e| e.deps.len()), Some(1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every kind of damage a store can suffer reads as "no cache", never as a panic.
    #[test]
    fn a_corrupt_store_is_an_empty_cache() {
        let dir = std::env::temp_dir().join(format!("wf-cache-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for text in [
            "",
            "[[entry]]",
            "[[entry]]\nmodule = \"hello\"",
            "[[entry]]\nfingerprint = \"abc\"",
            "not toml at all {{{",
            "[[entry]]\nmodule = \"a\"\nfingerprint = \"x\"\n[[entry.deps]]\nmodule = \"a\"",
        ] {
            write_cache(&dir, text);
            assert_eq!(Cache::load(&dir), Cache::default(), "corrupt: {text}");
        }
        // A missing file is the same thing.
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(Cache::load(&dir), Cache::default());
    }

    #[test]
    fn a_dep_is_fresh_only_while_its_fingerprint_is_unchanged() {
        let entry = Entry {
            fingerprint: "own".to_owned(),
            deps: vec![("helper".to_owned(), "def456".to_owned())],
        };
        let mut current = BTreeMap::new();
        current.insert("helper".to_owned(), "def456".to_owned());
        assert!(entry.deps_are_fresh(&current));

        current.insert("helper".to_owned(), "changed".to_owned());
        assert!(
            !entry.deps_are_fresh(&current),
            "a moved dependency must rebuild"
        );

        current.remove("helper");
        assert!(
            !entry.deps_are_fresh(&current),
            "a disappeared dependency must rebuild"
        );
    }

    #[test]
    fn unsafe_names_do_not_reach_the_store() {
        let dir = std::env::temp_dir().join(format!("wf-cache-unsafe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cache = Cache::default();
        cache.entries.insert(
            "has \"quote\"".to_owned(),
            Entry {
                fingerprint: "x".to_owned(),
                deps: Vec::new(),
            },
        );
        cache.save(&dir);
        let loaded = Cache::load(&dir);
        assert!(loaded.entries.is_empty(), "{loaded:?}");
        // The saved text must still parse as TOML.
        let text = std::fs::read_to_string(Cache::store_path(&dir)).expect("file was written");
        assert!(toml::from_str::<toml::Value>(&text).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
