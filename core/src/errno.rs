// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux errno values as carried in USB/IP status fields (always Linux numbering on the wire,
//! independent of the host the tests run on).

pub const ENOENT: i32 = 2;
pub const EIO: i32 = 5;
pub const EAGAIN: i32 = 11;
pub const ENOMEM: i32 = 12;
pub const EBUSY: i32 = 16;
pub const EXDEV: i32 = 18;
pub const ENODEV: i32 = 19;
pub const EINVAL: i32 = 22;
pub const EPIPE: i32 = 32;
pub const EPROTO: i32 = 71;
pub const EOVERFLOW: i32 = 75;
pub const ECONNRESET: i32 = 104;
pub const ESHUTDOWN: i32 = 108;
pub const ETIMEDOUT: i32 = 110;
