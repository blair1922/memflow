/*!
Connector Plugin interface.
*/

use crate::error::{Error, Result};
use crate::mem::PhysicalMemory;

use super::ConnectorArgs;

use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use libloading::Library;

/// Exported memflow plugin version
pub const MEMFLOW_CONNECTOR_VERSION: i32 = 1;

/// Describes a connector plugin
pub struct ConnectorDescriptor {
    /// The connector plugin api version for when the connector was built.
    /// This has to be set to `MEMFLOW_CONNECTOR_VERSION` of memflow_core.
    ///
    /// If the versions mismatch the plugin will refuse to load.
    pub connector_version: i32,

    /// The name of the connector plugin.
    /// This name will be used when loading a plugin from a plugin inventory.
    pub name: &'static str,

    /// The factory function for the connector.
    /// Calling this function will produce new connector instances.
    pub factory: extern "C" fn(args: &ConnectorArgs) -> Result<Box<dyn PhysicalMemory>>,
}

/// Holds an inventory of available connector plugins.
pub struct ConnectorInventory {
    connectors: Vec<Connector>,
}

impl ConnectorInventory {
    /// Creates a new inventory of connectors from the provided path.
    /// The path has to be a valid directory or the function will fail with an `Error::IO` error.
    ///
    /// # Safety
    ///
    /// Loading third party libraries is inherently unsafe and the compiler
    /// cannot guarantee that the implementation of the library
    /// matches the one specified here. This is especially true if
    /// the loaded library implements the necessary interface manually.
    ///
    /// # Examples
    ///
    /// Creating a inventory:
    /// ```
    /// use memflow_core::connector::ConnectorInventory;
    ///
    /// let inventory = unsafe {
    ///     ConnectorInventory::new("./")
    /// }.unwrap();
    /// ```
    pub unsafe fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut dir = PathBuf::default();
        dir.push(path);
        if !dir.is_dir() {
            return Err(Error::IO("inventory requires a valid path as argument"));
        }

        let mut connectors = Vec::new();

        // TODO: handle sub directories
        for entry in read_dir(dir).map_err(|_| Error::IO("unable to read directory"))? {
            let entry = entry.map_err(|_| Error::IO("unable to read directory entry"))?;
            if let Ok(connector) = Connector::try_with(entry.path()) {
                println!("connector loaded: {:?}", entry.path());
                connectors.push(connector);
            }
        }

        Ok(Self { connectors })
    }

    /// Tries to create a new connector instance for the connector with the given name.
    /// The connector will be initialized with the args provided to this call.
    ///
    /// In case no connector could be found this will throw an `Error::Connector`.
    ///
    /// # Safety
    ///
    /// Loading third party libraries is inherently unsafe and the compiler
    /// cannot guarantee that the implementation of the library
    /// matches the one specified here. This is especially true if
    /// the loaded library implements the necessary interface manually.
    ///
    /// It is adviced to use a proc macro for defining a connector plugin.
    ///
    /// # Examples
    ///
    /// Creating a connector instance:
    /// ```no_run
    /// use memflow_core::connector::{ConnectorInventory, ConnectorArgs};
    ///
    /// let inventory = unsafe {
    ///     ConnectorInventory::new("./")
    /// }.unwrap();
    /// let connector = unsafe {
    ///     inventory.create_connector("coredump", &ConnectorArgs::new())
    /// }.unwrap();
    /// ```
    ///
    /// Defining a dynamic plugin:
    /// ```
    /// use memflow_core::error::Result;
    /// use memflow_core::types::size;
    /// use memflow_core::mem::dummy::DummyMemory;
    /// use memflow_core::connector::ConnectorArgs;
    /// use memflow_derive::connector;
    ///
    /// #[connector(name = "dummy")]
    /// pub fn create_connector(_args: &ConnectorArgs) -> Result<DummyMemory> {
    ///     Ok(DummyMemory::new(size::mb(16)))
    /// }
    /// ```
    pub unsafe fn create_connector(
        &self,
        name: &str,
        args: &ConnectorArgs,
    ) -> Result<Box<dyn PhysicalMemory>> {
        let connector = self
            .connectors
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| Error::Connector("connector not found"))?;
        connector.create(args)
    }

    /// Creates a connector in the same way `create_connector` does but without any arguments provided.
    ///
    /// # Safety
    ///
    /// See the above safety section.
    /// This function essentially just wraps the above function.
    ///
    /// # Examples
    ///
    /// Creating a connector instance:
    /// ```no_run
    /// use memflow_core::connector::{ConnectorInventory, ConnectorArgs};
    ///
    /// let inventory = unsafe {
    ///     ConnectorInventory::new("./")
    /// }.unwrap();
    /// let connector = unsafe {
    ///     inventory.create_connector_default("coredump")
    /// }.unwrap();
    /// ```
    pub unsafe fn create_connector_default(&self, name: &str) -> Result<Box<dyn PhysicalMemory>> {
        self.create_connector(name, &ConnectorArgs::default())
    }
}

/// Stores a connector plugin library instance.
///
/// # Examples
///
/// Creating a connector instance:
/// ```no_run
/// use memflow_core::connector::{Connector, ConnectorArgs};
///
/// let connector_lib = unsafe {
///     Connector::try_with("./connector.so")
/// }.unwrap();
///
/// let connector = unsafe {
///     connector_lib.create(&ConnectorArgs::new())
/// }.unwrap();
/// ```
pub struct Connector {
    _library: Rc<Library>,
    name: String,
    factory: extern "C" fn(args: &ConnectorArgs) -> Result<Box<dyn PhysicalMemory>>,
}

impl Connector {
    /// Tries to initialize a connector from a `Path`.
    /// The path must point to a valid dynamic library that implements
    /// the memflow plugin interface.
    ///
    /// If the plugin doesn ot contain the necessary exports or the version does
    /// not match the current api version this function will return an `Error::Connector`.
    ///
    /// # Safety
    ///
    /// Loading third party libraries is inherently unsafe and the compiler
    /// cannot guarantee that the implementation of the library
    /// matches the one specified here. This is especially true if
    /// the loaded library implements the necessary interface manually.
    pub unsafe fn try_with<P: AsRef<Path>>(path: P) -> Result<Self> {
        let library =
            Library::new(path.as_ref()).map_err(|_| Error::Connector("unable to load library"))?;

        let desc = library
            .get::<*mut ConnectorDescriptor>(b"MEMFLOW_CONNECTOR\0")
            .map_err(|_| Error::Connector("connector descriptor not found"))?
            .read();

        if desc.connector_version != MEMFLOW_CONNECTOR_VERSION {
            return Err(Error::Connector("connector version mismatch"));
        }

        Ok(Self {
            _library: Rc::new(library),
            name: desc.name.to_string(),
            factory: desc.factory,
        })
    }

    /// Creates a new connector instance from this library.
    /// The connector is initialized with the arguments provided to this function.
    ///
    /// # Safety
    ///
    /// Loading third party libraries is inherently unsafe and the compiler
    /// cannot guarantee that the implementation of the library
    /// matches the one specified here. This is especially true if
    /// the loaded library implements the necessary interface manually.
    ///
    /// It is adviced to use a proc macro for defining a connector plugin.
    pub unsafe fn create(&self, args: &ConnectorArgs) -> Result<Box<dyn PhysicalMemory>> {
        (self.factory)(args)
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory() {
        let inventory = unsafe { ConnectorInventory::new("../target/release/") }.unwrap();
        unsafe {
            inventory.create_connector(
                "coredump",
                "/home/patrick/Documents/coredumps/coredump_win10_64bit.raw",
            )
        }
        .unwrap();
    }
}
*/
