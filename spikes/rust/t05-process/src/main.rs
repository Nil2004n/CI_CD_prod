//! D3-1 compile target only: links `libc` so the approved dependency is built.
//! The P00-T05 process-supervision spike is not implemented here and this
//! binary is not executed under D3-1.

use libc as _;

fn main() {}
