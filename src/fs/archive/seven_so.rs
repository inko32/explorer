//! 7z.so backend — native archive listing via the shared library's COM
//! interface (`IInArchive`), dlopened at first use (D16). Compatible with
//! upstream 7-Zip and 7zip-zstd builds (both export CreateObject /
//! GetNumberOfFormats / GetHandlerProperty2 / SysFreeString).
//!
//! Format handlers are discovered at runtime by enumerating
//! `GetNumberOfFormats` and matching the file extension against each
//! handler's `Extension` property — no hard-coded format GUIDs. Item
//! property IDs (Path/Size/IsDir/MTime) are likewise discovered through
//! `GetPropertyInfo` with fixed-ID fallback.
//!
//! All FFI is `unsafe` and concentrated in this file; the rest of the app
//! only sees `ArchiveBackend`.

use super::{ext_of, ArchiveBackend, ArchiveItem, FsError};
use std::path::Path;
use std::sync::OnceLock;

// ---------------- COM ABI ----------------

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
struct Guid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const IID_IUNKNOWN: Guid = Guid {
    d1: 0x0000_0000,
    d2: 0x0000,
    d3: 0x0000,
    d4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
/// 7-Zip interface GUID family: {23170F69-40C1-278A-0000-000300010000}
const IID_IIN_ARCHIVE: Guid = Guid {
    d1: 0x2317_0F69,
    d2: 0x40C1,
    d3: 0x278A,
    d4: [0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x00],
};

const S_OK: i32 = 0;
const E_FAIL: i32 = -2_147_467_259; // 0x80004005 as HRESULT
const E_NOINTERFACE: i32 = -2_147_462_262; // 0x80004002

const VT_EMPTY: u16 = 0;
const VT_BSTR: u16 = 8;
const VT_BOOL: u16 = 11;
const VT_I8: u16 = 20;
const VT_UI8: u16 = 21;
const VT_LPWSTR: u16 = 31;
const VT_FILETIME: u16 = 64;

#[repr(C)]
union PvVal {
    ll: i64,
    ul: u64,
    b: i16,
    bstr: *mut u16,
}

/// Windows PROPVARIANT (16 bytes, 8-byte-aligned union).
#[repr(C)]
struct PropVariant {
    vt: u16,
    _r1: u16,
    _r2: u16,
    _r3: u16,
    val: PvVal,
}

impl PropVariant {
    fn empty() -> Self {
        // zeroed = VT_EMPTY with null payload
        Self {
            vt: VT_EMPTY,
            _r1: 0,
            _r2: 0,
            _r3: 0,
            val: PvVal { ll: 0 },
        }
    }
}

type Void = core::ffi::c_void;

#[repr(C)]
struct InStream {
    vtbl: *const InStreamVtable,
    file: std::fs::File,
    pos: u64,
    len: u64,
}

#[repr(C)]
struct InStreamVtable {
    query_interface: extern "C" fn(*mut InStream, *const Guid, *mut *mut Void) -> i32,
    add_ref: extern "C" fn(*mut InStream) -> u32,
    release: extern "C" fn(*mut InStream) -> u32,
    read: extern "C" fn(*mut InStream, *mut u8, u32, *mut u32) -> i32,
    seek: extern "C" fn(*mut InStream, i64, u32, *mut u64) -> i32,
}

extern "C" fn s_qi(_: *mut InStream, _: *const Guid, out: *mut *mut Void) -> i32 {
    // 7z never QI's our stream; report E_NOINTERFACE, but don't dangle.
    unsafe { *out = std::ptr::null_mut() };
    E_NOINTERFACE
}
extern "C" fn s_add_ref(_: *mut InStream) -> u32 {
    1
}
extern "C" fn s_release(_: *mut InStream) -> u32 {
    1
}
extern "C" fn s_read(s: *mut InStream, data: *mut u8, size: u32, processed: *mut u32) -> i32 {
    use std::io::Read;
    let s = unsafe { &mut *s };
    let mut n = 0u32;
    while n < size {
        let buf = unsafe { std::slice::from_raw_parts_mut(data.add(n as usize), (size - n) as usize) };
        match s.file.read(buf) {
            Ok(0) => break,
            Ok(got) => {
                n += got as u32;
                s.pos += got as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return E_FAIL,
        }
    }
    unsafe { *processed = n };
    S_OK
}
extern "C" fn s_seek(s: *mut InStream, offset: i64, origin: u32, new_pos: *mut u64) -> i32 {
    use std::io::{Seek, SeekFrom};
    let s = unsafe { &mut *s };
    let (base, rel): (u64, i64) = match origin {
        0 => (0, offset),
        1 => (s.pos, offset),
        2 => (s.len, offset),
        _ => return E_FAIL,
    };
    let target = if rel >= 0 {
        base.checked_add(rel as u64)
    } else {
        base.checked_sub(rel.unsigned_abs())
    };
    let Some(target) = target else { return E_FAIL };
    if s.file.seek(SeekFrom::Start(target)).is_err() {
        return E_FAIL;
    }
    s.pos = target;
    unsafe { *new_pos = target };
    S_OK
}

static STREAM_VTBL: InStreamVtable = InStreamVtable {
    query_interface: s_qi,
    add_ref: s_add_ref,
    release: s_release,
    read: s_read,
    seek: s_seek,
};

// IInArchive vtable (7zip/IArchive.h): IUnknown, then Open, Close,
// GetNumberOfItems, GetProperty, Extract, GetArchiveProperty,
// GetNumberOfProperties, GetPropertyInfo, GetNumberOfArchiveProperties,
// GetArchivePropertyInfo.
#[repr(C)]
struct InArchiveVtable {
    query_interface: extern "C" fn(*mut Void, *const Guid, *mut *mut Void) -> i32,
    add_ref: extern "C" fn(*mut Void) -> u32,
    release: extern "C" fn(*mut Void) -> u32,
    open: extern "C" fn(*mut Void, *mut InStream, *const u64, *mut Void) -> i32,
    close: extern "C" fn(*mut Void) -> i32,
    get_number_of_items: extern "C" fn(*mut Void, *mut u32) -> i32,
    get_property: extern "C" fn(*mut Void, u32, u32, *mut PropVariant) -> i32,
    _extract: extern "C" fn(*mut Void, *const u32, u32, i32, *mut Void) -> i32,
    _get_archive_property: extern "C" fn(*mut Void, u32, *mut PropVariant) -> i32,
    get_number_of_properties: extern "C" fn(*mut Void, *mut u32) -> i32,
    get_property_info: extern "C" fn(*mut Void, u32, *mut *mut u16, *mut u32, *mut u16) -> i32,
}

// ---------------- BSTR helpers ----------------

struct BStr<'lib> {
    ptr: *mut u16,
    free: Option<&'lib libloading::Symbol<'lib, extern "C" fn(*mut u16)>>,
}

impl<'lib> BStr<'lib> {
    fn new(ptr: *mut u16, free: Option<&'lib libloading::Symbol<'lib, extern "C" fn(*mut u16)>>) -> Self {
        Self { ptr, free }
    }
    /// Decode to String (None on null). Length prefix is the BYTE count.
    fn to_string(&self) -> Option<String> {
        if self.ptr.is_null() {
            return None;
        }
        let bytes = unsafe { *(self.ptr as *const u32).sub(1) } as usize;
        let units = unsafe { std::slice::from_raw_parts(self.ptr, bytes / 2) };
        String::from_utf16(units).ok()
    }
}

impl Drop for BStr<'_> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            if let Some(free) = self.free {
                free(self.ptr);
            }
            // else: SysFreeString unavailable — small leak, bounded by use
        }
    }
}

fn prop_bstr(v: &PropVariant, free: Option<&libloading::Symbol<'_, extern "C" fn(*mut u16)>>) -> Option<String> {
    match v.vt {
        VT_BSTR | VT_LPWSTR => BStr::new(unsafe { v.val.bstr }, free).to_string(),
        _ => None,
    }
}

fn prop_u64(v: &PropVariant) -> Option<u64> {
    match v.vt {
        VT_UI8 | VT_I8 | VT_FILETIME => Some(unsafe { v.val.ul }),
        _ => None,
    }
}

/// FILETIME (100ns ticks since 1601-01-01) -> epoch seconds.
fn filetime_to_epoch(ft: u64) -> Option<i64> {
    Some((ft / 10_000_000) as i64 - 11_644_473_600)
}

// ---------------- library handle ----------------

struct SevenLib {
    create_object:
        libloading::Symbol<'static, unsafe extern "C" fn(*const Guid, *const Guid, *mut *mut Void) -> i32>,
    get_number_of_formats: libloading::Symbol<'static, unsafe extern "C" fn(*mut u32) -> i32>,
    get_handler_property2:
        libloading::Symbol<'static, unsafe extern "C" fn(u32, u32, *mut PropVariant) -> i32>,
    sys_free_string: Option<libloading::Symbol<'static, extern "C" fn(*mut u16)>>,
}

unsafe impl Send for SevenLib {}
unsafe impl Sync for SevenLib {}

/// One format handler offered by the library.
struct FormatHandler {
    clsid: Guid,
    extensions: Vec<String>,
}

fn lib() -> Result<&'static SevenLib, FsError> {
    static LIB: OnceLock<Result<SevenLib, String>> = OnceLock::new();
    let r = LIB.get_or_init(load);
    r.as_ref().map_err(|e| FsError(e.clone()))
}

fn load() -> Result<SevenLib, String> {
    let candidates: Vec<std::path::PathBuf> = {
        let mut v: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(p) = std::env::var("EXPLORER_7Z_SO") {
            v.push(p.into());
        }
        for p in [
            "/usr/lib/7zip/7z.so",
            "/usr/lib/p7zip/7z.so",
            "/usr/local/lib/7z.so",
            "/usr/lib/7z.so",
        ] {
            v.push(p.into());
        }
        v
    };
    let mut tried = String::new();
    for path in candidates {
        match unsafe { libloading::Library::new(&path) } {
            Ok(l) => {
                return SevenLib::new(l).map_err(|e| format!("{}: {e}", path.display()));
            }
            Err(e) => {
                tried.push_str(&format!("{}: {e}; ", path.display()));
            }
        }
    }
    Err(format!("7z.so not loadable ({tried})"))
}

impl SevenLib {
    fn new(lib: libloading::Library) -> Result<Self, String> {
        // Leak the library into 'static so the Symbols stay valid for the
        // process lifetime (the Library handle itself is never dropped).
        let leaked: &'static libloading::Library = Box::leak(Box::new(lib));
        unsafe {
            Ok(Self {
                create_object: leaked
                    .get(b"CreateObject\0")
                    .map_err(|e| format!("CreateObject: {e}"))?,
                get_number_of_formats: leaked
                    .get(b"GetNumberOfFormats\0")
                    .map_err(|e| format!("GetNumberOfFormats: {e}"))?,
                get_handler_property2: leaked
                    .get(b"GetHandlerProperty2\0")
                    .map_err(|e| format!("GetHandlerProperty2: {e}"))?,
                sys_free_string: leaked.get(b"SysFreeString\0").ok(),
            })
        }
    }

    /// All format handlers, cached.
    fn handlers(&self) -> Result<&'static [FormatHandler], FsError> {
        static CACHE: OnceLock<Result<Vec<FormatHandler>, String>> = OnceLock::new();
        let r = CACHE.get_or_init(|| self.enumerate_handlers().map_err(|e| e.0));
        r.as_deref().map_err(|e| FsError(e.clone()))
    }

    fn enumerate_handlers(&self) -> Result<Vec<FormatHandler>, FsError> {
        let mut n = 0u32;
        let hr = unsafe { (self.get_number_of_formats)(&mut n) };
        if hr != S_OK {
            return Err(FsError(format!("GetNumberOfFormats -> {hr:#x}")));
        }
        let mut out = Vec::new();
        for i in 0..n {
            let mut pv = PropVariant::empty();
            let mut clsid_s = None;
            if unsafe { (self.get_handler_property2)(i, 1, &mut pv) } == S_OK {
                clsid_s = prop_bstr(&pv, self.sys_free_string.as_ref());
            }
            let mut pv = PropVariant::empty();
            let mut exts = Vec::new();
            if unsafe { (self.get_handler_property2)(i, 2, &mut pv) } == S_OK {
                if let Some(s) = prop_bstr(&pv, self.sys_free_string.as_ref()) {
                    exts = s
                        .split_whitespace()
                        .map(|e| e.trim_start_matches('.').to_lowercase())
                        .collect();
                }
            }
            if let Some(g) = clsid_s.as_deref().and_then(parse_guid) {
                out.push(FormatHandler { clsid: g, extensions: exts });
            }
        }
        Ok(out)
    }

    /// Create an IInArchive instance for a handler; Release()d by the caller.
    fn create_in_archive(&self, clsid: &Guid) -> Result<*mut Void, FsError> {
        let mut obj: *mut Void = std::ptr::null_mut();
        let mut hr =
            unsafe { (self.create_object)(clsid, &IID_IIN_ARCHIVE, &mut obj) };
        if hr != S_OK || obj.is_null() {
            // Some builds only accept IID_IUnknown
            hr = unsafe { (self.create_object)(clsid, &IID_IUNKNOWN, &mut obj) };
        }
        if hr != S_OK || obj.is_null() {
            return Err(FsError(format!("CreateObject -> {hr:#x}")));
        }
        Ok(obj)
    }
}

/// "{23170F69-40C1-278A-1000-000110010000}" (braces optional) -> Guid.
fn parse_guid(s: &str) -> Option<Guid> {
    let h = |s: &str| u64::from_str_radix(s, 16).ok();
    let s = s.trim().trim_start_matches('{').trim_end_matches('}');
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return None;
    }
    let d1 = u32::from_str_radix(parts[0], 16).ok()?;
    let d2 = u16::from_str_radix(parts[1], 16).ok()?;
    let d3 = u16::from_str_radix(parts[2], 16).ok()?;
    let d4hi = h(parts[3])? as u16;
    let d4lo = h(parts[4])? as u64;
    Some(Guid {
        d1,
        d2,
        d3,
        d4: [
            (d4hi >> 8) as u8,
            d4hi as u8,
            (d4lo >> 56) as u8,
            (d4lo >> 48) as u8,
            (d4lo >> 40) as u8,
            (d4lo >> 32) as u8,
            (d4lo >> 24) as u8,
            (d4lo >> 16) as u8,
        ],
    })
}

// ---------------- backend ----------------

pub struct SevenZipSo;

impl ArchiveBackend for SevenZipSo {
    fn name(&self) -> &'static str {
        "so"
    }

    fn can_open(&self, path: &Path) -> bool {
        lib().is_ok() && super::is_archive_path(path)
    }

    fn list(&self, archive: &Path) -> Result<Vec<ArchiveItem>, FsError> {
        let l = lib()?;
        let Some(ext) = ext_of(archive) else {
            return Err(FsError("no extension".into()));
        };
        let mut last_err = FsError(format!("no handler for .{ext}"));
        for h in l.handlers()? {
            if !h.extensions.iter().any(|e| *e == ext) {
                continue;
            }
            match list_with_handler(l, h, archive) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}

fn vtbl(obj: *mut Void) -> *const InArchiveVtable {
    unsafe { *(obj as *const *const InArchiveVtable) }
}

/// Item property IDs, discovered per-handler via GetPropertyInfo (index into
/// `GetProperty`), with the classic fixed IDs as fallback.
struct PropIds {
    path: u32,
    size: u32,
    is_dir: u32,
    mtime: u32,
}

fn prop_ids(l: &SevenLib, obj: *mut Void) -> PropIds {
    let vt = unsafe { &*vtbl(obj) };
    let mut ids = PropIds {
        path: 3,
        size: 7,
        is_dir: 6,
        mtime: 12,
    };
    let mut n = 0u32;
    if (vt.get_number_of_properties)(obj, &mut n) != S_OK {
        return ids;
    }
    for i in 0..n {
        let mut name: *mut u16 = std::ptr::null_mut();
        let mut id = 0u32;
        let mut ty = 0u16;
        let hr = (vt.get_property_info)(obj, i, &mut name, &mut id, &mut ty);
        let _ = ty;
        if hr != S_OK {
            continue;
        }
        let s = BStr::new(name, l.sys_free_string.as_ref()).to_string();
        match s.as_deref() {
            Some("Path") => ids.path = id,
            Some("Size") => ids.size = id,
            Some("IsDir") => ids.is_dir = id,
            Some("MTime") => ids.mtime = id,
            _ => {}
        }
    }
    ids
}

fn list_with_handler(
    l: &SevenLib,
    h: &FormatHandler,
    archive: &Path,
) -> Result<Vec<ArchiveItem>, FsError> {
    let obj = l.create_in_archive(&h.clsid)?;
    let result = (|| -> Result<Vec<ArchiveItem>, FsError> {
        let vt = unsafe { &*vtbl(obj) };
        let file = std::fs::File::open(archive)
            .map_err(|e| FsError(format!("open {}: {e}", archive.display())))?;
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut stream = Box::new(InStream {
            vtbl: &STREAM_VTBL,
            file,
            pos: 0,
            len,
        });
        let stream_ptr: *mut InStream = &mut *stream;
        let hr = (vt.open)(obj, stream_ptr, std::ptr::null(), std::ptr::null_mut());
        if hr != S_OK {
            return Err(FsError(format!("Open -> {hr:#x}")));
        }
        let mut count = 0u32;
        let hr = (vt.get_number_of_items)(obj, &mut count);
        if hr != S_OK {
            return Err(FsError(format!("GetNumberOfItems -> {hr:#x}")));
        }
        let ids = prop_ids(l, obj);
        let free = l.sys_free_string.as_ref();
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut path = None;
            let mut size = 0u64;
            let mut is_dir = false;
            let mut modified = None;

            let mut pv = PropVariant::empty();
            if (vt.get_property)(obj, i, ids.path, &mut pv) == S_OK {
                path = prop_bstr(&pv, free);
            }
            let mut pv = PropVariant::empty();
            if (vt.get_property)(obj, i, ids.size, &mut pv) == S_OK {
                size = prop_u64(&pv).unwrap_or(0);
            }
            let mut pv = PropVariant::empty();
            if (vt.get_property)(obj, i, ids.is_dir, &mut pv) == S_OK {
                is_dir = match pv.vt {
                    VT_BOOL => (unsafe { pv.val.b } != 0),
                    VT_UI8 | VT_I8 => (unsafe { pv.val.ul } != 0),
                    _ => false,
                };
            }
            let mut pv = PropVariant::empty();
            if (vt.get_property)(obj, i, ids.mtime, &mut pv) == S_OK {
                if pv.vt == VT_FILETIME {
                    modified = prop_u64(&pv).and_then(filetime_to_epoch);
                }
            }
            let Some(path) = path else { continue };
            out.push(ArchiveItem {
                path,
                size,
                is_dir,
                modified,
            });
        }
        Ok(out)
    })();
    let vt = unsafe { &*vtbl(obj) };
    let _ = (vt.close)(obj);
    let _ = (vt.release)(obj);
    result
}
