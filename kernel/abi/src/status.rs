// SPDX-License-Identifier: GPL-3.0-only

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(i32);

impl Error {
    pub const ENOSYS: Self = Self(-1);
    pub const EINVAL: Self = Self(-2);
    pub const EFAULT: Self = Self(-3);
    pub const EACCES: Self = Self(-4);
    pub const ENOENT: Self = Self(-5);
    pub const EEXIST: Self = Self(-6);
    pub const ENOMEM: Self = Self(-7);
    pub const ENOSPC: Self = Self(-8);
    pub const EBADF: Self = Self(-9);
    pub const EAGAIN: Self = Self(-10);
    pub const ETIMEDOUT: Self = Self(-11);
    pub const EBUSY: Self = Self(-12);
    pub const EIO: Self = Self(-13);
    pub const EINTR: Self = Self(-14);
    pub const ENOTSUP: Self = Self(-15);
    pub const EMSGSIZE: Self = Self(-16);
    pub const EPIPE: Self = Self(-17);
    pub const ESTALE: Self = Self(-18);
    pub const ENOTDIR: Self = Self(-19);
    pub const EISDIR: Self = Self(-20);
    pub const ENOTEMPTY: Self = Self(-21);
    pub const EROFS: Self = Self(-22);
    pub const EOVERFLOW: Self = Self(-23);
    pub const EABI: Self = Self(-24);
    pub const ELIMIT: Self = Self(-25);

    pub const fn code(self) -> i32 {
        self.0
    }

    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }
}
