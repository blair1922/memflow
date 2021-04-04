use crate::error::*;

use super::{
    ArcPluginKeyboard, ArcPluginProcess, Keyboard, MuArcPluginKeyboard, MuPluginKeyboard,
    OsKeyboardFunctionTable, OsLayerFunctionTable, PluginKeyboard, PluginOsKeyboard, PluginProcess,
};
use crate::os::{
    AddressCallback, ModuleInfo, OsInfo, OsInner, OsKeyboardInner, Process, ProcessInfo,
};
use crate::types::Address;
use std::ffi::c_void;

use super::super::COptArc;
use super::PluginOs;
use super::{MuArcPluginProcess, MuModuleInfo, MuPluginProcess, MuProcessInfo};

use libloading::Library;

pub type OpaqueOsFunctionTable = OsFunctionTable<'static, c_void, c_void>;

impl Copy for OpaqueOsFunctionTable {}

impl Clone for OpaqueOsFunctionTable {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(C)]
pub struct OsFunctionTable<'a, P, T> {
    pub process_address_list_callback: extern "C" fn(os: &mut T, callback: AddressCallback) -> i32,
    pub process_info_by_address:
        extern "C" fn(os: &mut T, address: Address, out: &mut MuProcessInfo) -> i32,
    pub process_by_info:
        extern "C" fn(os: &'a mut T, info: ProcessInfo, out: &mut MuPluginProcess<'a>) -> i32,
    pub into_process_by_info: extern "C" fn(
        os: &mut T,
        info: ProcessInfo,
        lib: COptArc<Library>,
        out: &mut MuArcPluginProcess,
    ) -> i32,
    pub module_address_list_callback: extern "C" fn(os: &mut T, callback: AddressCallback) -> i32,
    pub module_by_address:
        extern "C" fn(os: &mut T, address: Address, out: &mut MuModuleInfo) -> i32,
    pub info: extern "C" fn(os: &T) -> &OsInfo,
    phantom: std::marker::PhantomData<P>,
}

impl<'a, P: 'static + Process + Clone, T: PluginOs<P>> Default for &'a OsFunctionTable<'a, P, T> {
    fn default() -> Self {
        &OsFunctionTable {
            process_address_list_callback: c_process_address_list_callback,
            process_info_by_address: c_process_info_by_address,
            process_by_info: c_process_by_info,
            into_process_by_info: c_into_process_by_info,
            module_address_list_callback: c_module_address_list_callback,
            module_by_address: c_module_by_address,
            info: c_os_info,
            phantom: std::marker::PhantomData {},
        }
    }
}

impl<'a, P: Process + Clone, T: PluginOs<P>> OsFunctionTable<'a, P, T> {
    pub fn as_opaque(&self) -> &OpaqueOsFunctionTable {
        unsafe { &*(self as *const Self as *const OpaqueOsFunctionTable) }
    }
}

extern "C" fn c_process_address_list_callback<'a, T: OsInner<'a>>(
    os: &mut T,
    callback: AddressCallback,
) -> i32 {
    os.process_address_list_callback(callback).into_int_result()
}

extern "C" fn c_process_info_by_address<'a, T: OsInner<'a>>(
    os: &mut T,
    address: Address,
    out: &mut MuProcessInfo,
) -> i32 {
    os.process_info_by_address(address).into_int_out_result(out)
}

extern "C" fn c_process_by_info<'a, T: 'a + OsInner<'a>>(
    os: &'a mut T,
    info: ProcessInfo,
    out: &mut MuPluginProcess<'a>,
) -> i32 {
    os.process_by_info(info)
        .map(PluginProcess::new)
        .into_int_out_result(out)
}

extern "C" fn c_into_process_by_info<P: 'static + Process + Clone, T: 'static + PluginOs<P>>(
    os: &mut T,
    info: ProcessInfo,
    lib: COptArc<Library>,
    out: &mut MuArcPluginProcess,
) -> i32 {
    let os = unsafe { Box::from_raw(os) };
    os.into_process_by_info(info)
        .map(|p| ArcPluginProcess::new(p, lib))
        .into_int_out_result(out)
}

extern "C" fn c_module_address_list_callback<'a, T: OsInner<'a>>(
    os: &mut T,
    callback: AddressCallback,
) -> i32 {
    os.module_address_list_callback(callback).into_int_result()
}

extern "C" fn c_module_by_address<'a, T: OsInner<'a>>(
    os: &mut T,
    address: Address,
    out: &mut MuModuleInfo,
) -> i32 {
    os.module_by_address(address).into_int_out_result(out)
}

extern "C" fn c_os_info<'a, T: OsInner<'a>>(os: &T) -> &OsInfo {
    os.info()
}

/// Describes initialized os instance
///
/// This structure is returned by `OS`. It is needed to maintain reference
/// counts to the loaded plugin library.
#[repr(C)]
pub struct OsInstance {
    instance: &'static mut c_void,
    vtable: OsLayerFunctionTable,

    /// Internal library arc.
    ///
    /// This will keep the library loaded in memory as long as the os instance is alive.
    /// This has to be the last member of the struct so the library will be unloaded _after_
    /// the instance is destroyed.
    ///
    /// If the library is unloaded prior to the instance this will lead to a SIGSEGV.
    pub(super) library: COptArc<Library>,
}

impl OsInstance {
    pub fn builder<P: 'static + Process + Clone, T: PluginOs<P>>(
        instance: T,
    ) -> OsInstanceBuilder<T> {
        OsInstanceBuilder {
            instance,
            vtable: OsLayerFunctionTable::new::<P, T>(),
        }
    }
}

/// Builder for the os instance structure.
pub struct OsInstanceBuilder<T> {
    instance: T,
    vtable: OsLayerFunctionTable,
}

impl<T> OsInstanceBuilder<T> {
    /// Enables the optional Keyboard feature for the OsInstance.
    pub fn enable_keyboard<K>(mut self) -> Self
    where
        K: 'static + Keyboard + Clone,
        T: PluginOsKeyboard<K>,
    {
        self.vtable.keyboard = Some(<&OsKeyboardFunctionTable<K, T>>::default().as_opaque());
        self
    }

    /// Build the OsInstance
    pub fn build(self) -> OsInstance {
        OsInstance {
            instance: unsafe {
                Box::into_raw(Box::new(self.instance))
                    .cast::<c_void>()
                    .as_mut()
            }
            .unwrap(),
            vtable: self.vtable,
            library: None.into(),
        }
    }
}

impl OsInstance {
    pub fn has_phys_mem(&self) -> bool {
        self.vtable.phys.is_some()
    }

    pub fn has_virt_mem(&self) -> bool {
        self.vtable.virt.is_some()
    }

    pub fn has_keyboard(&self) -> bool {
        self.vtable.keyboard.is_some()
    }
}

impl<'a> OsInner<'a> for OsInstance {
    type ProcessType = PluginProcess<'a>;
    type IntoProcessType = ArcPluginProcess;

    /// Walks a process list and calls a callback for each process structure address
    ///
    /// The callback is fully opaque. We need this style so that C FFI can work seamlessly.
    fn process_address_list_callback(&mut self, callback: AddressCallback) -> Result<()> {
        result_from_int_void((self.vtable.os.process_address_list_callback)(
            self.instance,
            callback,
        ))
    }

    /// Find process information by its internal address
    fn process_info_by_address(&mut self, address: Address) -> Result<ProcessInfo> {
        let mut out = MuProcessInfo::uninit();
        let res = (self.vtable.os.process_info_by_address)(self.instance, address, &mut out);
        result_from_int(res, out)
    }

    /// Construct a process by its info, borrowing the os
    ///
    /// It will share the underlying memory resources
    fn process_by_info(&'a mut self, info: ProcessInfo) -> Result<Self::ProcessType> {
        let mut out = MuPluginProcess::uninit();
        // Shorten the lifetime of instance
        let instance = unsafe { (self.instance as *mut c_void).as_mut() }.unwrap();
        let res = (self.vtable.os.process_by_info)(instance, info, &mut out);
        result_from_int(res, out)
    }
    /// Construct a process by its info, consuming the os
    ///
    /// This function will consume the OS instance and move its resources into the process
    fn into_process_by_info(mut self, info: ProcessInfo) -> Result<Self::IntoProcessType> {
        let mut out = MuArcPluginProcess::uninit();
        let res = (self.vtable.os.into_process_by_info)(
            self.instance,
            info,
            self.library.take(),
            &mut out,
        );
        std::mem::forget(self);
        result_from_int(res, out)
    }

    /// Walks the os module list and calls the provided callback for each module structure
    /// address
    ///
    /// # Arguments
    /// * `callback` - where to pass each matching module to. This is an opaque callback.
    fn module_address_list_callback(&mut self, callback: AddressCallback) -> Result<()> {
        result_from_int_void((self.vtable.os.module_address_list_callback)(
            self.instance,
            callback,
        ))
    }

    /// Retrieves a module by its structure address
    ///
    /// # Arguments
    /// * `address` - address where module's information resides in
    fn module_by_address(&mut self, address: Address) -> Result<ModuleInfo> {
        let mut out = MuModuleInfo::uninit();
        let res = (self.vtable.os.module_by_address)(self.instance, address, &mut out);
        result_from_int(res, out)
    }

    /// Retrieves the os info
    fn info(&self) -> &OsInfo {
        (self.vtable.os.info)(self.instance)
    }
}

/// Optional Keyboard feature implementation
impl<'a> OsKeyboardInner<'a> for OsInstance {
    type KeyboardType = PluginKeyboard<'a>;
    type IntoKeyboardType = ArcPluginKeyboard;

    fn keyboard(&'a mut self) -> Result<Self::KeyboardType> {
        let kbd = self.vtable.keyboard.ok_or(Error(
            ErrorOrigin::OsLayer,
            ErrorKind::UnsupportedOptionalFeature,
        ))?;
        let mut out = MuPluginKeyboard::uninit();
        // Shorten the lifetime of instance
        let instance = unsafe { (self.instance as *mut c_void).as_mut() }.unwrap();
        let res = (kbd.keyboard)(instance, self.library.clone(), &mut out);
        result_from_int(res, out)
    }

    fn into_keyboard(mut self) -> Result<Self::IntoKeyboardType> {
        let kbd = self.vtable.keyboard.ok_or(Error(
            ErrorOrigin::OsLayer,
            ErrorKind::UnsupportedOptionalFeature,
        ))?;
        let mut out = MuArcPluginKeyboard::uninit();
        let res = (kbd.into_keyboard)(self.instance, self.library.take(), &mut out);
        std::mem::forget(self);
        result_from_int(res, out)
    }
}

impl Clone for OsInstance {
    fn clone(&self) -> Self {
        let instance =
            (self.vtable.base.clone.clone)(self.instance).expect("Unable to clone Connector");
        Self {
            instance,
            vtable: self.vtable,
            library: self.library.clone(),
        }
    }
}

impl Drop for OsInstance {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.base.drop)(self.instance);
        }
    }
}
