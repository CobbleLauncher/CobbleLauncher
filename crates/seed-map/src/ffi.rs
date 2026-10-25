// SPDX-License-Identifier: MIT
//! The only unsafe boundary: fixed vendored C functions and owned generator lifetime.
#![allow(unsafe_code)]
use std::{
    ffi::{CStr, c_char, c_void},
    ptr::NonNull,
};

unsafe extern "C" {
    fn cobble_seed_new(seed: u64, version: i32, dim: i32, large: i32) -> *mut c_void;
    fn cobble_seed_free(g: *mut c_void);
    fn cobble_seed_biome(g: *const c_void, x: i32, y: i32, z: i32) -> i32;
    fn cobble_seed_biome_name(id: i32, version: i32) -> *const c_char;
    fn cobble_seed_biome_dimension(id: i32, version: i32) -> i32;
    fn cobble_seed_biome_color(id: i32) -> u32;
    fn cobble_seed_structure_config(kind: i32, version: i32, dim: *mut i32) -> i32;
    fn cobble_seed_structure(
        g: *mut c_void,
        kind: i32,
        rx: i32,
        rz: i32,
        x: *mut i32,
        z: *mut i32,
    ) -> i32;
    #[cfg(test)]
    fn getBiomeAt(g: *const c_void, scale: i32, x: i32, y: i32, z: i32) -> i32;
}

pub struct Generator(NonNull<c_void>);
impl Generator {
    pub fn new(seed: i64, version: i32, dim: i32, large: bool) -> Option<Self> {
        // SAFETY: all scalar arguments are validated; C returns a fresh owned allocation.
        NonNull::new(unsafe { cobble_seed_new(seed as u64, version, dim, i32::from(large)) })
            .map(Self)
    }
    pub fn biome(&self, x: i32, y: i32, z: i32) -> i32 {
        // SAFETY: allocation remains alive; coordinates are bounded by caller.
        unsafe { cobble_seed_biome(self.0.as_ptr(), x, y, z) }
    }
    #[cfg(test)]
    pub fn upstream_biome4(&self, x: i32, y: i32, z: i32) -> i32 {
        // SAFETY: test reproduces pinned upstream's bounded reference-vector
        // coordinates at the explicitly supported scale4; generator is alive.
        unsafe { getBiomeAt(self.0.as_ptr(), 4, x, y, z) }
    }
    pub fn structure(&mut self, kind: i32, rx: i32, rz: i32) -> Option<(i32, i32)> {
        let (mut x, mut z) = (0, 0);
        // SAFETY: exclusive generator, supported kind, valid output pointers.
        let found = unsafe { cobble_seed_structure(self.0.as_ptr(), kind, rx, rz, &mut x, &mut z) };
        (found != 0).then_some((x, z))
    }
}
impl Drop for Generator {
    fn drop(&mut self) {
        // SAFETY: exactly the owned allocation returned by new; freed once.
        unsafe { cobble_seed_free(self.0.as_ptr()) }
    }
}
pub fn biome_info(id: i32, version: i32) -> Option<(String, i32, u32)> {
    // SAFETY: supported ID checked by C; returned strings have static storage.
    unsafe {
        let dim = cobble_seed_biome_dimension(id, version);
        if dim == 1000 {
            return None;
        }
        let name = cobble_seed_biome_name(id, version);
        if name.is_null() {
            return None;
        }
        Some((
            CStr::from_ptr(name).to_string_lossy().into_owned(),
            dim,
            cobble_seed_biome_color(id),
        ))
    }
}
pub fn structure_config(kind: i32, version: i32) -> Option<(i32, i32)> {
    let mut dim = 1000;
    // SAFETY: C validates kind; output points to a live scalar.
    let span = unsafe { cobble_seed_structure_config(kind, version, &mut dim) };
    (span > 0).then_some((span, dim))
}
