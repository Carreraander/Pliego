//! Enlace FFI mínimo con la API C de Tesseract (`capi.h`). Solo se declara lo que usa `pliego-ocr`.
#![allow(unsafe_code, non_camel_case_types)]

use std::os::raw::{c_char, c_float, c_int, c_uchar, c_void};

/// Manejadores opacos.
#[repr(C)]
pub struct TessBaseAPI {
    _private: [u8; 0],
}
#[repr(C)]
pub struct TessResultIterator {
    _private: [u8; 0],
}
#[repr(C)]
pub struct TessPageIterator {
    _private: [u8; 0],
}

/// `BOOL` de la API C es un `int`.
pub type BOOL = c_int;

pub type TessPageIteratorLevel = c_int;
pub const RIL_BLOCK: TessPageIteratorLevel = 0;
pub const RIL_PARA: TessPageIteratorLevel = 1;
pub const RIL_TEXTLINE: TessPageIteratorLevel = 2;
pub const RIL_WORD: TessPageIteratorLevel = 3;
pub const RIL_SYMBOL: TessPageIteratorLevel = 4;

pub type TessPageSegMode = c_int;
pub const PSM_OSD_ONLY: TessPageSegMode = 0;
pub const PSM_AUTO: TessPageSegMode = 3;
pub const PSM_SPARSE_TEXT: TessPageSegMode = 11;

unsafe extern "C" {
    pub fn TessVersion() -> *const c_char;
    pub fn TessDeleteText(text: *const c_char);
    pub fn TessDeleteTextArray(arr: *mut *mut c_char);

    pub fn TessBaseAPICreate() -> *mut TessBaseAPI;
    pub fn TessBaseAPIDelete(handle: *mut TessBaseAPI);
    pub fn TessBaseAPIInit3(handle: *mut TessBaseAPI, datapath: *const c_char, language: *const c_char) -> c_int;
    pub fn TessBaseAPISetVariable(handle: *mut TessBaseAPI, name: *const c_char, value: *const c_char) -> BOOL;
    pub fn TessBaseAPISetPageSegMode(handle: *mut TessBaseAPI, mode: TessPageSegMode);
    pub fn TessBaseAPISetImage(
        handle: *mut TessBaseAPI,
        imagedata: *const c_uchar,
        width: c_int,
        height: c_int,
        bytes_per_pixel: c_int,
        bytes_per_line: c_int,
    );
    pub fn TessBaseAPISetSourceResolution(handle: *mut TessBaseAPI, ppi: c_int);
    pub fn TessBaseAPIRecognize(handle: *mut TessBaseAPI, monitor: *mut c_void) -> c_int;
    pub fn TessBaseAPIGetIterator(handle: *mut TessBaseAPI) -> *mut TessResultIterator;
    pub fn TessBaseAPIClear(handle: *mut TessBaseAPI);
    pub fn TessBaseAPIEnd(handle: *mut TessBaseAPI);
    pub fn TessBaseAPIDetectOrientationScript(
        handle: *mut TessBaseAPI,
        orient_deg: *mut c_int,
        orient_conf: *mut c_float,
        script_name: *mut *const c_char,
        script_conf: *mut c_float,
    ) -> BOOL;

    pub fn TessResultIteratorDelete(handle: *mut TessResultIterator);
    pub fn TessResultIteratorGetPageIteratorConst(handle: *const TessResultIterator) -> *const TessPageIterator;
    pub fn TessResultIteratorNext(handle: *mut TessResultIterator, level: TessPageIteratorLevel) -> BOOL;
    pub fn TessResultIteratorGetUTF8Text(
        handle: *const TessResultIterator,
        level: TessPageIteratorLevel,
    ) -> *mut c_char;
    pub fn TessResultIteratorConfidence(handle: *const TessResultIterator, level: TessPageIteratorLevel) -> c_float;
    pub fn TessPageIteratorBoundingBox(
        handle: *const TessPageIterator,
        level: TessPageIteratorLevel,
        left: *mut c_int,
        top: *mut c_int,
        right: *mut c_int,
        bottom: *mut c_int,
    ) -> BOOL;
}
