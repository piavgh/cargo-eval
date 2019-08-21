/*!
This module is for platform-specific stuff.
*/

pub use self::inner::{
    get_cache_dir, get_config_dir,
    write_path, read_path,
    force_cargo_color,
};


use std::fs;

use std::time::{SystemTime, UNIX_EPOCH};

// Last-modified time of a file, in milliseconds since the UNIX epoch.
pub fn file_last_modified(file: &fs::File) -> u128 {
  file.metadata().and_then(|md| md.modified().map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_millis())).unwrap_or(0)
}


// Current system time, in milliseconds since the UNIX epoch.
pub fn current_time() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
}

#[cfg(unix)]
mod inner {
    extern crate atty;

    pub use super::*;

    use std::path::{Path, PathBuf};
    use std::{env, io};
    use std::os::unix::ffi::OsStrExt;
    use crate::error::{MainError, Blame};

    /**
    Get a directory suitable for storing user- and machine-specific data which may or may not be persisted across sessions.

    This is chosen to match the location where Cargo places its cache data.
    */
    pub fn get_cache_dir() -> Result<PathBuf, MainError> {
        // try $CARGO_HOME then fall back to $HOME
        if let Some(home) = env::var_os("CARGO_HOME") {
            let home = Path::new(&home);
            let old_home = home.join(".cargo");
            if old_home.exists() {
                // Keep using the old directory in preference to the new one, but only if it still contains `script-cache` and/or `binary-cache`.
                if old_home.join("script-cache").exists() || old_home.join("binary-cache").exists() {
                    // Yup; use this one.
                    return Ok(old_home);
                }
            }

            // Just use `$CARGO_HOME` directly.
            return Ok(home.into());
        }

        if let Some(home) = env::var_os("HOME") {
            return Ok(Path::new(&home).join(".cargo"));
        }

        Err((Blame::Human, "neither $CARGO_HOME nor $HOME is defined").into())
    }

    /**
    Get a directory suitable for storing user-specific configuration data.

    This is chosen to match the location where Cargo places its configuration data.
    */
    pub fn get_config_dir() -> Result<PathBuf, MainError> {
        // Currently, this appears to be the same as the cache directory.
        get_cache_dir()
    }

    pub fn write_path<W>(w: &mut W, path: &Path) -> io::Result<()>
    where W: io::Write {
        w.write_all(path.as_os_str().as_bytes())
    }

    pub fn read_path<R>(r: &mut R) -> io::Result<PathBuf>
    where R: io::Read {
        use std::ffi::OsStr;
        let mut buf = vec![];
        r.read_to_end(&mut buf)?;
        Ok(OsStr::from_bytes(&buf).into())
    }

    /**
    Returns `true` if `cargo-eval` should force Cargo to use coloured output.

    This depends on whether `cargo-eval`'s STDERR is connected to a TTY or not.
    */
    pub fn force_cargo_color() -> bool {
        atty::is(atty::Stream::Stderr)
    }
}

#[cfg(windows)]
pub mod inner {
    #![allow(non_snake_case)]

    pub use super::*;

    use std::ffi::OsString;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::mem;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use crate::error::MainError;

    use winapi::{
      shared::{
        minwindef::DWORD,
        ntdef::{HANDLE, PWSTR},
        winerror::{HRESULT, S_OK},
      },
      um::{
        combaseapi::CoTaskMemFree,
        shlobj::SHGetKnownFolderPath,
        shtypes::KNOWNFOLDERID,
        knownfolders::{FOLDERID_LocalAppData, FOLDERID_RoamingAppData},
      },
    };

    /**
    Get a directory suitable for storing user- and machine-specific data which may or may not be persisted across sessions.

    This is *not* chosen to match the location where Cargo places its cache data, because Cargo is *wrong*.  This is at least *less wrong*.

    On Windows, LocalAppData is where user- and machine- specific data should go, but it *might* be more appropriate to use whatever the official name for "Program Data" is, though.
    */
    pub fn get_cache_dir() -> Result<PathBuf, MainError> {
        let rfid = &FOLDERID_LocalAppData;
        let dir = sh_get_known_folder_path(rfid, 0, ::std::ptr::null_mut())
            .map_err(|e| e.to_string())?;
        Ok(Path::new(&dir).to_path_buf().join("Cargo"))
    }

    /**
    Get a directory suitable for storing user-specific configuration data.

    This is *not* chosen to match the location where Cargo places its cache data, because Cargo is *wrong*.  This is at least *less wrong*.
    */
    pub fn get_config_dir() -> Result<PathBuf, MainError> {
        let rfid = &FOLDERID_RoamingAppData;
        let dir = sh_get_known_folder_path(rfid, 0, ::std::ptr::null_mut())
            .map_err(|e| e.to_string())?;
        Ok(Path::new(&dir).to_path_buf().join("Cargo"))
    }

    fn sh_get_known_folder_path(rfid: &KNOWNFOLDERID, dwFlags: DWORD, hToken: HANDLE) -> Result<OsString, HRESULT> {
        let mut psz_path: PWSTR = unsafe { mem::uninitialized() };
        let hresult = unsafe {
            SHGetKnownFolderPath(
                rfid,
                dwFlags,
                hToken,
                mem::transmute(&mut psz_path as &mut PWSTR as *mut PWSTR)
            )
        };

        if hresult == S_OK {
            let r = unsafe { pwstr_to_os_string(psz_path) };
            unsafe { CoTaskMemFree(psz_path as *mut _) };
            Ok(r)
        } else {
            Err(hresult)
        }
    }

    unsafe fn pwstr_to_os_string(ptr: PWSTR) -> OsString {
        OsStringExt::from_wide(::std::slice::from_raw_parts(ptr, pwstr_len(ptr)))
    }

    unsafe fn pwstr_len(mut ptr: PWSTR) -> usize {
        let mut len = 0;
        while *ptr != 0 {
            len += 1;
            ptr = ptr.offset(1);
        }
        len
    }

    pub fn write_path<W>(w: &mut W, path: &Path) -> io::Result<()>
    where W: io::Write {
        for word in path.as_os_str().encode_wide() {
            let lo = (word & 0xff) as u8;
            let hi = (word >> 8) as u8;
            w.write_all(&[lo, hi])?;
        }
        Ok(())
    }

    pub fn read_path<R>(r: &mut R) -> io::Result<PathBuf>
    where R: io::Read {
        let mut buf = vec![];
        r.read_to_end(&mut buf)?;

        let mut words = Vec::with_capacity(buf.len() / 2);
        let mut it = buf.iter().cloned();
        while let Some(lo) = it.next() {
            let hi = it.next().unwrap();
            words.push(lo as u16 | ((hi as u16) << 8));
        }

        return Ok(OsString::from_wide(&words).into())
    }

    /**
    Returns `true` if `cargo-eval` should force Cargo to use coloured output.

    Always returns `false` on Windows because colour is communicated over a side-channel.
    */
    pub fn force_cargo_color() -> bool {
        false
    }
}
