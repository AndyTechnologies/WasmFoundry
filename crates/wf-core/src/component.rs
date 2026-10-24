//! Component Model domain types.
//!
//! The Component Model is an expansion of the system, not a replacement of Core
//! Module support. These types live in `wf-core` so they can be shared across
//! `wf-wasm` (analysis), `wf-runtime` (execution), `wf-toolchains` (compilation)
//! and `wf-cli` (commands) without introducing cycles.

use crate::NameError;

/// The name of a WIT interface or world.
///
/// Validated to be a valid WIT identifier: ASCII letters, digits, `_` and `-`,
/// not starting with a digit, not empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WitName(String);

impl WitName {
    /// Creates a new `WitName`, validating it against WIT identifier rules.
    pub fn new(name: impl Into<String>) -> Result<Self, NameError> {
        let name = name.into();
        if name.is_empty() {
            return Err(NameError::new("WIT name"));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(NameError::new("WIT name"));
        }
        if name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            return Err(NameError::new("WIT name"));
        }
        Ok(WitName(name))
    }

    /// Returns the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WitName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for WitName {
    type Err = NameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WitName::new(s)
    }
}

/// A namespace-qualified identifier: `namespace:name`.
///
/// Used for interfaces and worlds (e.g., `wasi:cli`, `myorg:myworld`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WitQualifiedName {
    namespace: WitName,
    name: WitName,
}

impl WitQualifiedName {
    /// Creates a new qualified name from namespace and name.
    pub fn new(namespace: WitName, name: WitName) -> Self {
        WitQualifiedName { namespace, name }
    }

    /// The namespace part (e.g., `wasi` in `wasi:cli`).
    pub fn namespace(&self) -> &WitName {
        &self.namespace
    }

    /// The name part (e.g., `cli` in `wasi:cli`).
    pub fn name(&self) -> &WitName {
        &self.name
    }

    /// Returns the string representation as `namespace:name`.
    pub fn as_str(&self) -> String {
        format!("{}:{}", self.namespace, self.name)
    }
}

impl std::fmt::Display for WitQualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// A WIT package (a collection of interfaces and worlds).
///
/// This corresponds to a `package` in WIT terminology: a versioned collection
/// of interfaces and a single world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitPackage {
    name: WitQualifiedName,
    version: String, // semver-like, not strictly validated here
    interfaces: Vec<Interface>,
    world: Option<World>,
}

impl WitPackage {
    /// Creates a new WIT package.
    pub fn new(name: WitQualifiedName, version: String) -> Self {
        WitPackage {
            name,
            version,
            interfaces: Vec::new(),
            world: None,
        }
    }

    /// The package's qualified name (namespace:name).
    pub fn name(&self) -> &WitQualifiedName {
        &self.name
    }

    /// The package's version string.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The interfaces declared in this package.
    pub fn interfaces(&self) -> &[Interface] {
        &self.interfaces
    }

    /// The world declared in this package (at most one).
    pub fn world(&self) -> Option<&World> {
        self.world.as_ref()
    }

    /// Adds an interface to the package.
    pub fn add_interface(&mut self, interface: Interface) {
        self.interfaces.push(interface);
    }

    /// Sets the package's world.
    pub fn set_world(&mut self, world: World) {
        self.world = Some(world);
    }
}

/// A WIT interface: a collection of functions and types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    name: WitQualifiedName,
    functions: Vec<Function>,
    types: Vec<Type>,
}

impl Interface {
    /// Creates a new interface.
    pub fn new(name: WitQualifiedName) -> Self {
        Interface {
            name,
            functions: Vec::new(),
            types: Vec::new(),
        }
    }

    /// The interface's qualified name.
    pub fn name(&self) -> &WitQualifiedName {
        &self.name
    }

    /// The functions declared in this interface.
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    /// The types declared in this interface.
    pub fn types(&self) -> &[Type] {
        &self.types
    }

    /// Adds a function to the interface.
    pub fn add_function(&mut self, function: Function) {
        self.functions.push(function);
    }

    /// Adds a type to the interface.
    pub fn add_type(&mut self, ty: Type) {
        self.types.push(ty);
    }
}

/// A function signature in an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    name: WitName,
    params: Vec<(WitName, Type)>,
    results: Vec<Type>,
}

impl Function {
    /// Creates a new function.
    pub fn new(name: WitName) -> Self {
        Function {
            name,
            params: Vec::new(),
            results: Vec::new(),
        }
    }

    /// The function's name.
    pub fn name(&self) -> &WitName {
        &self.name
    }

    /// The function's parameters as (name, type) pairs.
    pub fn params(&self) -> &[(WitName, Type)] {
        &self.params
    }

    /// The function's result types.
    pub fn results(&self) -> &[Type] {
        &self.results
    }

    /// Adds a parameter.
    pub fn add_param(&mut self, name: WitName, ty: Type) {
        self.params.push((name, ty));
    }

    /// Adds a result type.
    pub fn add_result(&mut self, ty: Type) {
        self.results.push(ty);
    }
}

/// A WIT type reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in type (e.g., `u32`, `string`).
    Builtin(BuiltinType),
    /// A user-defined type from an interface.
    Defined(WitQualifiedName),
    /// A list of elements.
    List(Box<Type>),
    /// An option type.
    Option(Box<Type>),
    /// A result type.
    Result(Box<Type>, Box<Type>),
    /// A tuple of types.
    Tuple(Vec<Type>),
    /// A record type (named fields).
    Record(Vec<(WitName, Type)>),
    /// A variant type (tagged union).
    Variant(Vec<(WitName, Option<Type>)>),
    /// An enum type (simple tags).
    Enum(Vec<WitName>),
    /// A flags type (bitflags).
    Flags(Vec<WitName>),
    /// A handle (resource).
    Handle(WitQualifiedName),
}

/// The built-in WIT types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuiltinType {
    Bool,
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    Float32,
    Float64,
    Char,
    String,
}

impl std::fmt::Display for BuiltinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinType::Bool => f.write_str("bool"),
            BuiltinType::U8 => f.write_str("u8"),
            BuiltinType::U16 => f.write_str("u16"),
            BuiltinType::U32 => f.write_str("u32"),
            BuiltinType::U64 => f.write_str("u64"),
            BuiltinType::S8 => f.write_str("s8"),
            BuiltinType::S16 => f.write_str("s16"),
            BuiltinType::S32 => f.write_str("s32"),
            BuiltinType::S64 => f.write_str("s64"),
            BuiltinType::Float32 => f.write_str("float32"),
            BuiltinType::Float64 => f.write_str("float64"),
            BuiltinType::Char => f.write_str("char"),
            BuiltinType::String => f.write_str("string"),
        }
    }
}

/// A WIT world: the exports and imports of a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    name: WitQualifiedName,
    exports: Vec<(WitName, WitQualifiedName)>, // (local name, interface)
    imports: Vec<(WitName, WitQualifiedName)>,
}

impl World {
    /// Creates a new world.
    pub fn new(name: WitQualifiedName) -> Self {
        World {
            name,
            exports: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// The world's qualified name.
    pub fn name(&self) -> &WitQualifiedName {
        &self.name
    }

    /// The exports of this world as (local name, interface) pairs.
    pub fn exports(&self) -> &[(WitName, WitQualifiedName)] {
        &self.exports
    }

    /// The imports of this world as (local name, interface) pairs.
    pub fn imports(&self) -> &[(WitName, WitQualifiedName)] {
        &self.imports
    }

    /// Adds an export.
    pub fn add_export(&mut self, local: WitName, interface: WitQualifiedName) {
        self.exports.push((local, interface));
    }

    /// Adds an import.
    pub fn add_import(&mut self, local: WitName, interface: WitQualifiedName) {
        self.imports.push((local, interface));
    }
}

/// A compiled WebAssembly component with its WIT package.
///
/// This is what the toolchain produces and the runtime consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitComponent {
    /// The WIT package describing the component's interfaces and world.
    pub wit_package: WitPackage,
    /// The raw component bytes.
    pub bytes: Vec<u8>,
}

impl WitComponent {
    /// Creates a new component from a WIT package and bytes.
    pub fn new(wit_package: WitPackage, bytes: Vec<u8>) -> Self {
        WitComponent { wit_package, bytes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wit_name_validates_basic_rules() {
        assert!(WitName::new("valid").is_ok());
        assert!(WitName::new("valid_name").is_ok());
        assert!(WitName::new("valid-name").is_ok());
        assert!(WitName::new("valid123").is_ok());
        assert!(WitName::new("").is_err());
        assert!(WitName::new("123invalid").is_err());
        assert!(WitName::new("with space").is_err());
        assert!(WitName::new("with.dot").is_err());
    }

    #[test]
    fn wit_qualified_name_formats_correctly() {
        let qn = WitQualifiedName::new(WitName::new("wasi").unwrap(), WitName::new("cli").unwrap());
        assert_eq!(qn.as_str(), "wasi:cli");
    }

    #[test]
    fn package_holds_interfaces_and_world() {
        let mut pkg = WitPackage::new(
            WitQualifiedName::new(
                WitName::new("example").unwrap(),
                WitName::new("pkg").unwrap(),
            ),
            "1.0.0".to_string(),
        );

        let iface = Interface::new(WitQualifiedName::new(
            WitName::new("example").unwrap(),
            WitName::new("iface").unwrap(),
        ));
        pkg.add_interface(iface);

        let world = World::new(WitQualifiedName::new(
            WitName::new("example").unwrap(),
            WitName::new("world").unwrap(),
        ));
        pkg.set_world(world);

        assert_eq!(pkg.interfaces().len(), 1);
        assert!(pkg.world().is_some());
    }

    #[test]
    fn world_tracks_exports_and_imports() {
        let mut world = World::new(WitQualifiedName::new(
            WitName::new("example").unwrap(),
            WitName::new("world").unwrap(),
        ));

        world.add_export(
            WitName::new("run").unwrap(),
            WitQualifiedName::new(WitName::new("wasi").unwrap(), WitName::new("cli").unwrap()),
        );
        world.add_import(
            WitName::new("stdin").unwrap(),
            WitQualifiedName::new(WitName::new("wasi").unwrap(), WitName::new("io").unwrap()),
        );

        assert_eq!(world.exports().len(), 1);
        assert_eq!(world.imports().len(), 1);
    }

    #[test]
    fn type_display_for_builtins() {
        assert_eq!(BuiltinType::U32.to_string(), "u32");
        assert_eq!(BuiltinType::String.to_string(), "string");
    }

    #[test]
    fn wit_component_roundtrip() {
        let pkg = WitPackage::new(
            WitQualifiedName::new(
                WitName::new("example").unwrap(),
                WitName::new("pkg").unwrap(),
            ),
            "1.0.0".to_string(),
        );
        let component = WitComponent::new(pkg, vec![0, 1, 2, 3]);
        assert_eq!(component.bytes, vec![0, 1, 2, 3]);
        assert_eq!(component.wit_package.version(), "1.0.0");
    }
}
