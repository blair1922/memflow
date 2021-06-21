//! Describes process context

use super::{
    ExportCallback, ExportInfo, ImportCallback, ImportInfo, ModuleAddressInfo, ModuleInfo,
    ModuleInfoCallback, SectionCallback, SectionInfo,
};
use crate::cglue::*;
use crate::mem::virt_mem::*;
use crate::prelude::v1::{Result, *};
use std::prelude::v1::*;

// ProcessInstance group for Process and AsVirtualMemory
#[cfg(feature = "plugins")]
cglue_trait_group!(ProcessInstance, { Process, AsVirtualMemory }, {});
#[cfg(feature = "plugins")]
cglue_trait_group!(IntoProcessInstance, { Process, AsVirtualMemory, Clone }, {});

/// Type meant for process IDs
///
/// If there is a case where Pid can be over 32-bit limit, or negative, please open an issue, we
/// would love to see that.
pub type Pid = u32;

/// Exit code of a process
pub type ExitCode = i32;

/// The state of a process
///
/// # Remarks
///
/// In case the exit code isn't known ProcessState::Unknown is set.
#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum ProcessState {
    Unknown,
    Alive,
    Dead(ExitCode),
}

impl ProcessState {
    pub fn is_alive(&self) -> bool {
        matches!(*self, ProcessState::Alive)
    }

    pub fn is_dead(&self) -> bool {
        matches!(*self, ProcessState::Dead(_))
    }

    pub fn is_unknown(&self) -> bool {
        matches!(*self, ProcessState::Unknown)
    }
}

/// Provides all actions on processes
///
/// This trait provides a lot of typical functionality for processes, such as memory access, module lists, and basic information.
///
/// Future expansions could include threads, keyboard input, and more.
#[cfg_attr(feature = "plugins", cglue_trait)]
#[cfg_attr(feature = "plugins", int_result)]
pub trait Process: Send {
    /// Retrieves the state of the process
    fn state(&mut self) -> ProcessState;

    /// Walks the process' module list and calls the provided callback for each module structure
    /// address
    ///
    /// # Arguments
    /// * `target_arch` - sets which architecture to retrieve the modules for (if emulated). Choose
    /// between `Some(ProcessInfo::sys_arch())`, and `Some(ProcessInfo::proc_arch())`. `None` for all.
    /// * `callback` - where to pass each matching module to. This is an opaque callback.
    fn module_address_list_callback(
        &mut self,
        target_arch: Option<&ArchitectureIdent>,
        callback: ModuleAddressCallback,
    ) -> Result<()>;

    /// Walks the process' module list and calls the provided callback for each module
    ///
    /// # Arguments
    /// * `target_arch` - sets which architecture to retrieve the modules for (if emulated). Choose
    /// between `Some(ProcessInfo::sys_arch())`, and `Some(ProcessInfo::proc_arch())`. `None` for all.
    /// * `callback` - where to pass each matching module to. This is an opaque callback.
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_list_callback(
        &mut self,
        target_arch: Option<&ArchitectureIdent>,
        mut callback: ModuleInfoCallback,
    ) -> Result<()> {
        // This is safe, because control will flow back to the callback.
        let sptr = self as *mut Self;
        let inner_callback = &mut |ModuleAddressInfo { address, arch }| match unsafe { &mut *sptr }
            .module_by_address(address, arch)
        {
            Ok(info) => callback.call(info),
            Err(Error(_, ErrorKind::PartialData)) => {
                log::trace!(
                    "Partial error when reading module {:x}, skipping entry",
                    address
                );
                true
            }
            Err(e) => {
                log::trace!("Error when reading module {:x} {:?}", address, e);
                false
            }
        };
        unsafe { sptr.as_mut().unwrap() }
            .module_address_list_callback(target_arch, inner_callback.into())
    }

    /// Retrieves a module by its structure address and architecture
    ///
    /// # Arguments
    /// * `address` - address where module's information resides in
    /// * `architecture` - architecture of the module. Should be either `ProcessInfo::proc_arch`, or `ProcessInfo::sys_arch`.
    fn module_by_address(
        &mut self,
        address: Address,
        architecture: ArchitectureIdent,
    ) -> Result<ModuleInfo>;

    /// Finds a process module by its name under specified architecture
    ///
    /// This function can be useful for quickly accessing a specific module
    ///
    /// # Arguments
    /// * `name` - name of the module to find
    /// * `architecture` - architecture of the module. Should be either `ProcessInfo::proc_arch`, or `ProcessInfo::sys_arch`, or None for both.
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_by_name_arch(
        &mut self,
        name: &str,
        architecture: Option<&ArchitectureIdent>,
    ) -> Result<ModuleInfo> {
        let mut ret = Err(Error(ErrorOrigin::OsLayer, ErrorKind::ModuleNotFound));
        let callback = &mut |data: ModuleInfo| {
            if data.name.as_ref() == name {
                ret = Ok(data);
                false
            } else {
                true
            }
        };
        self.module_list_callback(architecture, callback.into())?;
        ret
    }

    /// Finds any architecture process module by its name
    ///
    /// This function can be useful for quickly accessing a specific module
    ///
    /// # Arguments
    /// * `name` - name of the module to find
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_by_name(&mut self, name: &str) -> Result<ModuleInfo> {
        self.module_by_name_arch(name, None)
    }

    /// Retrieves a module list for the process
    ///
    /// # Arguments
    /// * `target_arch` - sets which architecture to retrieve the modules for (if emulated). Choose
    /// between `Some(ProcessInfo::sys_arch())`, and `Some(ProcessInfo::proc_arch())`. `None` for all.
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_list_arch(
        &mut self,
        target_arch: Option<&ArchitectureIdent>,
    ) -> Result<Vec<ModuleInfo>> {
        let mut ret = vec![];
        self.module_list_callback(target_arch, (&mut ret).into())?;
        Ok(ret)
    }

    /// Retrieves a module list for the process
    ///
    /// This is equivalent to `Process::module_list_arch(None)`
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_list(&mut self) -> Result<Vec<ModuleInfo>> {
        self.module_list_arch(None)
    }

    /// Retrieves address of the primary module structure of the process
    ///
    /// This will generally be for the initial executable that was run
    fn primary_module_address(&mut self) -> Result<Address>;

    /// Retrieves information for the primary module of the process
    ///
    /// This will generally be the initial executable that was run
    #[cfg_attr(feature = "plugins", skip_func)]
    fn primary_module(&mut self) -> Result<ModuleInfo> {
        let addr = self.primary_module_address()?;
        self.module_by_address(addr, self.info().proc_arch)
    }

    /// Retrieves a list of all imports of a given module
    fn module_import_list_callback(
        &mut self,
        info: &ModuleInfo,
        callback: ImportCallback,
    ) -> Result<()>;

    /// Retrieves a list of all exports of a given module
    fn module_export_list_callback(
        &mut self,
        info: &ModuleInfo,
        callback: ExportCallback,
    ) -> Result<()>;

    /// Retrieves a list of all sections of a given module
    fn module_section_list_callback(
        &mut self,
        info: &ModuleInfo,
        callback: SectionCallback,
    ) -> Result<()>;

    /// Retrieves a list of all imports of a given module
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_import_list(&mut self, info: &ModuleInfo) -> Result<Vec<ImportInfo>> {
        let mut ret = vec![];
        self.module_import_list_callback(info, (&mut ret).into())?;
        Ok(ret)
    }

    /// Retrieves a list of all exports of a given module
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_export_list(&mut self, info: &ModuleInfo) -> Result<Vec<ExportInfo>> {
        let mut ret = vec![];
        self.module_export_list_callback(info, (&mut ret).into())?;
        Ok(ret)
    }

    /// Retrieves a list of all sections of a given module
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_section_list(&mut self, info: &ModuleInfo) -> Result<Vec<SectionInfo>> {
        let mut ret = vec![];
        self.module_section_list_callback(info, (&mut ret).into())?;
        Ok(ret)
    }

    /// Finds a single import of a given module by its name
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_import_by_name(&mut self, info: &ModuleInfo, name: &str) -> Result<ImportInfo> {
        let mut ret = Err(Error(ErrorOrigin::OsLayer, ErrorKind::ImportNotFound));
        let callback = &mut |data: ImportInfo| {
            if data.name.as_ref() == name {
                ret = Ok(data);
                false
            } else {
                true
            }
        };
        self.module_import_list_callback(info, callback.into())?;
        ret
    }

    /// Finds a single export of a given module by its name
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_export_by_name(&mut self, info: &ModuleInfo, name: &str) -> Result<ExportInfo> {
        let mut ret = Err(Error(ErrorOrigin::OsLayer, ErrorKind::ImportNotFound));
        let callback = &mut |data: ExportInfo| {
            if data.name.as_ref() == name {
                ret = Ok(data);
                false
            } else {
                true
            }
        };
        self.module_export_list_callback(info, callback.into())?;
        ret
    }

    /// Finds a single section of a given module by its name
    #[cfg_attr(feature = "plugins", skip_func)]
    fn module_section_by_name(&mut self, info: &ModuleInfo, name: &str) -> Result<SectionInfo> {
        let mut ret = Err(Error(ErrorOrigin::OsLayer, ErrorKind::ImportNotFound));
        let callback = &mut |data: SectionInfo| {
            if data.name.as_ref() == name {
                ret = Ok(data);
                false
            } else {
                true
            }
        };
        self.module_section_list_callback(info, callback.into())?;
        ret
    }

    /// Retrieves the process info
    fn info(&self) -> &ProcessInfo;
}

/// Process information structure
///
/// This structure implements basic process information. Architectures are provided both of the
/// system, and of the process.
#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct ProcessInfo {
    /// The base address of this process.
    ///
    /// # Remarks
    ///
    /// On Windows this will be the address of the [`_EPROCESS`](https://www.nirsoft.net/kernel_struct/vista/EPROCESS.html) structure.
    pub address: Address,
    /// ID of this process.
    pub pid: Pid,
    /// Name of the process.
    pub name: ReprCString,
    /// Path of the process binary
    pub path: ReprCString,
    /// Command line the process was started with.
    pub command_line: ReprCString,
    /// System architecture of the target system.
    pub sys_arch: ArchitectureIdent,
    /// Process architecture
    ///
    /// # Remarks
    ///
    /// Specifically on 64-bit systems this could be different
    /// to the `sys_arch` in case the process is an emulated 32-bit process.
    ///
    /// On windows this technique is called [`WOW64`](https://docs.microsoft.com/en-us/windows/win32/winprog64/wow64-implementation-details).
    pub proc_arch: ArchitectureIdent,
}

pub type ProcessInfoCallback<'a> = OpaqueCallback<'a, ProcessInfo>;
