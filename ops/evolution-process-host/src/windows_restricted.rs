use std::ffi::OsStr;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT,
    SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
};
use windows_sys::Win32::Security::{
    ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl,
    OBJECT_INHERIT_ACE,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_DELETE_CHILD, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub(super) struct AppContainerProfile {
    name: Vec<u16>,
    sid: *mut core::ffi::c_void,
    deleted: bool,
}

impl AppContainerProfile {
    pub(super) fn create() -> Result<Self, ()> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let ticks = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ())?
            .as_nanos();
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!("maia-tier1-{}-{ticks:x}-{serial:x}", unsafe {
            GetCurrentProcessId()
        })
        .encode_utf16()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
        let mut sid = std::ptr::null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                std::ptr::null(),
                0,
                &mut sid,
            )
        };
        if hr != 0 || sid.is_null() {
            if !sid.is_null() {
                unsafe { LocalFree(sid) };
            }
            return Err(());
        }
        Ok(Self {
            name,
            sid,
            deleted: false,
        })
    }

    pub(super) fn sid(&self) -> *mut core::ffi::c_void {
        self.sid
    }

    pub(super) fn delete(&mut self) -> Result<(), ()> {
        if self.deleted {
            return Ok(());
        }
        if unsafe { DeleteAppContainerProfile(self.name.as_ptr()) } < 0 {
            return Err(());
        }
        self.deleted = true;
        if !self.sid.is_null() {
            unsafe { LocalFree(self.sid) };
            self.sid = std::ptr::null_mut();
        }
        Ok(())
    }
}

impl Drop for AppContainerProfile {
    fn drop(&mut self) {
        let _ = self.delete();
    }
}

struct AclSnapshot {
    path: Vec<u16>,
    original_acl: Vec<usize>,
    original_acl_bytes: usize,
}

pub(super) struct TemporaryAclGrants {
    snapshots: Vec<AclSnapshot>,
}

impl TemporaryAclGrants {
    pub(super) fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    pub(super) fn grant_read(
        &mut self,
        path: &Path,
        sid: *mut core::ffi::c_void,
    ) -> Result<(), ()> {
        self.grant(path, sid, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)
    }

    pub(super) fn grant_traverse(
        &mut self,
        path: &Path,
        sid: *mut core::ffi::c_void,
    ) -> Result<(), ()> {
        if !path.is_dir() {
            return Err(());
        }
        self.grant(path, sid, FILE_GENERIC_EXECUTE)
    }

    pub(super) fn grant_read_file(
        &mut self,
        path: &Path,
        sid: *mut core::ffi::c_void,
    ) -> Result<(), ()> {
        self.grant(path, sid, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)
    }

    pub(super) fn grant_modify(
        &mut self,
        path: &Path,
        sid: *mut core::ffi::c_void,
    ) -> Result<(), ()> {
        self.grant(
            path,
            sid,
            FILE_GENERIC_READ
                | FILE_GENERIC_WRITE
                | FILE_GENERIC_EXECUTE
                | DELETE
                | FILE_DELETE_CHILD,
        )
    }

    fn grant(&mut self, path: &Path, sid: *mut core::ffi::c_void, rights: u32) -> Result<(), ()> {
        let canonical = std::fs::canonicalize(path).map_err(|_| ())?;
        let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| ())?;
        if metadata.file_type().is_symlink()
            || (!metadata.is_dir() && rights & FILE_DELETE_CHILD != 0)
        {
            return Err(());
        }
        let name = wide(canonical.as_os_str());
        let mut dacl = std::ptr::null_mut();
        let mut descriptor = std::ptr::null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 || descriptor.is_null() {
            return Err(());
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut original_dacl = std::ptr::null_mut();
        let dacl_ok = unsafe {
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut original_dacl, &mut defaulted)
        };
        if dacl_ok == 0 || present == 0 || original_dacl.is_null() {
            unsafe { LocalFree(descriptor) };
            return Err(());
        }
        let acl_size = unsafe { (*original_dacl).AclSize as usize };
        if acl_size < std::mem::size_of::<ACL>() {
            unsafe { LocalFree(descriptor) };
            return Err(());
        }
        let mut original_acl = vec![0usize; acl_size.div_ceil(std::mem::size_of::<usize>())];
        unsafe {
            std::ptr::copy_nonoverlapping(
                original_dacl.cast::<u8>(),
                original_acl.as_mut_ptr().cast::<u8>(),
                acl_size,
            );
        }
        let trustee = TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        };
        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: rights,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: if metadata.is_dir() && rights != FILE_GENERIC_EXECUTE {
                OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
            } else {
                0
            },
            Trustee: trustee,
        };
        let mut new_acl = std::ptr::null_mut();
        let acl_status = unsafe { SetEntriesInAclW(1, &entry, original_dacl, &mut new_acl) };
        if acl_status != 0 || new_acl.is_null() {
            unsafe { LocalFree(descriptor) };
            if !new_acl.is_null() {
                unsafe { LocalFree(new_acl.cast()) };
            }
            return Err(());
        }
        let set_status = unsafe {
            SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                new_acl,
                std::ptr::null_mut(),
            )
        };
        unsafe {
            LocalFree(new_acl.cast());
            LocalFree(descriptor);
        }
        if set_status != 0 {
            return Err(());
        }
        self.snapshots.push(AclSnapshot {
            path: name,
            original_acl,
            original_acl_bytes: acl_size,
        });
        Ok(())
    }

    pub(super) fn restore_all(&mut self) -> Result<(), ()> {
        let mut failed = Vec::new();
        while let Some(snapshot) = self.snapshots.pop() {
            if restore_snapshot(&snapshot).is_err() {
                eprintln!(
                    "Tier-1 ACL restore failed for {}",
                    String::from_utf16_lossy(
                        &snapshot.path[..snapshot.path.len().saturating_sub(1)]
                    )
                );
                failed.push(snapshot);
            }
        }
        self.snapshots = failed;
        if self.snapshots.is_empty() {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Drop for TemporaryAclGrants {
    fn drop(&mut self) {
        if !self.snapshots.is_empty() {
            match self.restore_all() {
                Ok(()) => eprintln!("Tier-1 temporary ACL restoration after setup error: verified"),
                Err(()) => eprintln!("Tier-1 temporary ACL restoration after setup error: FAILED"),
            }
        }
    }
}

fn restore_snapshot(snapshot: &AclSnapshot) -> Result<(), ()> {
    let original_dacl = snapshot.original_acl.as_ptr().cast_mut().cast();
    let set_status = unsafe {
        SetNamedSecurityInfoW(
            snapshot.path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            original_dacl,
            std::ptr::null_mut(),
        )
    };
    if set_status != 0 {
        return Err(());
    }

    let mut actual_dacl = std::ptr::null_mut();
    let mut actual_descriptor = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            snapshot.path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut actual_dacl,
            std::ptr::null_mut(),
            &mut actual_descriptor,
        )
    };
    if status != 0 || actual_descriptor.is_null() || actual_dacl.is_null() {
        if !actual_descriptor.is_null() {
            unsafe { LocalFree(actual_descriptor) };
        }
        return Err(());
    }
    let size = unsafe { (*actual_dacl).AclSize as usize };
    let equal = size == snapshot.original_acl_bytes
        && unsafe {
            std::slice::from_raw_parts(actual_dacl.cast::<u8>(), size)
                == std::slice::from_raw_parts(
                    snapshot.original_acl.as_ptr().cast::<u8>(),
                    snapshot.original_acl_bytes,
                )
        };
    unsafe { LocalFree(actual_descriptor) };
    if equal { Ok(()) } else { Err(()) }
}

pub(super) fn probe_profile_creation() -> bool {
    let Ok(mut profile) = AppContainerProfile::create() else {
        return false;
    };
    profile.delete().is_ok()
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(iter::once(0)).collect()
}

pub(super) fn probe_directory_acl(path: &Path) -> bool {
    let Ok(mut profile) = AppContainerProfile::create() else {
        return false;
    };
    let mut grants = TemporaryAclGrants::new();
    let granted = grants.grant_modify(path, profile.sid()).is_ok();
    #[cfg(test)]
    eprintln!("M0.16.5 ACL probe: grant modify={granted}");
    let restored = grants.restore_all().is_ok();
    #[cfg(test)]
    eprintln!("M0.16.5 ACL probe: exact restore={restored}");
    if !restored {
        std::mem::forget(profile);
        return false;
    }
    let deleted = profile.delete().is_ok();
    #[cfg(test)]
    eprintln!("M0.16.5 ACL probe: profile delete={deleted}");
    granted && deleted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_temporary_acl_grants_restore_exact_descriptors() {
        let root = std::env::temp_dir().join(format!("maia-acl-restore-{}", unsafe {
            GetCurrentProcessId()
        }));
        std::fs::create_dir_all(root.join("child")).unwrap();
        std::fs::write(root.join("child/file.txt"), b"fixture").unwrap();
        let mut profile = AppContainerProfile::create().unwrap();
        let mut grants = TemporaryAclGrants::new();
        grants.grant_read(&root, profile.sid()).unwrap();
        grants
            .grant_modify(&root.join("child"), profile.sid())
            .unwrap();
        grants
            .grant_read_file(&root.join("child/file.txt"), profile.sid())
            .unwrap();
        grants.restore_all().expect("exact nested DACL restoration");
        profile.delete().expect("profile removal");
        std::fs::remove_dir_all(root).unwrap();
    }
}
