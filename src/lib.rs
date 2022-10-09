// MIT/Apache2 License

//! Implementation of a basic spin-based RwLock.
//! 
//! ## This is now deprecated in favor of [`spin-rs`].
//! 
//! [`spin-rs`]: https://crates.io/crates/spin

#![no_std]
#![warn(clippy::pedantic)]
#![allow(clippy::same_item_push)]
#![deprecated(since = "0.2.4", note = "Use spin-rs instead")]

#[cfg(any(test, loom))]
extern crate std;

use lock_api::{
    GuardSend, RawRwLock, RawRwLockDowngrade, RawRwLockUpgrade, RwLock as LARwLock,
    RwLockReadGuard as LARwLockReadGuard, RwLockUpgradableReadGuard as LARwLockUpgradableReadGuard,
    RwLockWriteGuard as LARwLockWriteGuard,
};

#[cfg(not(loom))]
use core::{
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
};
#[cfg(loom)]
use loom::{
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
};
#[cfg(loom)]
use once_cell::sync::OnceCell;

#[cfg(not(loom))]
/// Raw spinlock rwlock, wrapped in the `lock_api` RwLock struct.
pub struct RawRwSpinlock(AtomicUsize);

#[cfg(loom)]
/// Raw spinlock rwlock, wrapped in the `lock_api` RwLock struct.
pub struct RawRwSpinlock(OnceCell<AtomicUsize>);

#[cfg(not(loom))]
impl RawRwSpinlock {
    #[inline]
    fn ulock(&self) -> &AtomicUsize {
        &self.0
    }
}

#[cfg(loom)]
impl RawRwSpinlock {
    #[inline]
    fn ulock(&self) -> &AtomicUsize {
        self.0.get_or_init(|| AtomicUsize::new(0))
    }
}

// flags stored in the usize struct
const READER: usize = 1 << 2;
const UPGRADED: usize = 1 << 1;
const WRITER: usize = 1 << 0;

unsafe impl RawRwLock for RawRwSpinlock {
    #[cfg(not(loom))]
    const INIT: RawRwSpinlock = RawRwSpinlock(AtomicUsize::new(0));
    #[cfg(loom)]
    const INIT: RawRwSpinlock = RawRwSpinlock(OnceCell::new());

    type GuardMarker = GuardSend;

    fn lock_shared(&self) {
        while !self.try_lock_shared() {
            spin_loop()
        }
    }

    fn try_lock_shared(&self) -> bool {
        let value = self.ulock().fetch_add(READER, Ordering::Acquire);

        if value & (WRITER | UPGRADED) != 0 {
            self.ulock().fetch_sub(READER, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    fn try_lock_exclusive(&self) -> bool {
        self.ulock()
            .compare_exchange(0, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    fn lock_exclusive(&self) {
        loop {
            match self.ulock().compare_exchange_weak(
                0,
                WRITER,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(_) => spin_loop(),
            }
        }
    }

    unsafe fn unlock_shared(&self) {
        self.ulock().fetch_sub(READER, Ordering::Release);
    }

    unsafe fn unlock_exclusive(&self) {
        self.ulock()
            .fetch_and(!(WRITER | UPGRADED), Ordering::Release);
    }
}

unsafe impl RawRwLockUpgrade for RawRwSpinlock {
    fn lock_upgradable(&self) {
        while !self.try_lock_upgradable() {
            spin_loop()
        }
    }

    fn try_lock_upgradable(&self) -> bool {
        self.ulock().fetch_or(UPGRADED, Ordering::Acquire) & (WRITER | UPGRADED) == 0
    }

    unsafe fn try_upgrade(&self) -> bool {
        self.ulock()
            .compare_exchange(UPGRADED, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn upgrade(&self) {
        loop {
            match self.ulock().compare_exchange_weak(
                UPGRADED,
                WRITER,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(_) => spin_loop(),
            }
        }
    }

    unsafe fn unlock_upgradable(&self) {
        self.ulock().fetch_sub(UPGRADED, Ordering::AcqRel);
    }
}

unsafe impl RawRwLockDowngrade for RawRwSpinlock {
    unsafe fn downgrade(&self) {
        self.ulock().fetch_add(READER, Ordering::Acquire);
        self.unlock_exclusive();
    }
}

/// A read-write lock that uses a spinlock internally.
pub type RwLock<T> = LARwLock<RawRwSpinlock, T>;
/// A read guard for the read-write lock.
pub type RwLockReadGuard<'a, T> = LARwLockReadGuard<'a, RawRwSpinlock, T>;
/// A write guard fo the read-write lock.
pub type RwLockWriteGuard<'a, T> = LARwLockWriteGuard<'a, RawRwSpinlock, T>;
/// An upgradable read guard for the read-write lock.
pub type RwLockUpgradableReadGuard<'a, T> = LARwLockUpgradableReadGuard<'a, RawRwSpinlock, T>;

#[test]
fn basics() {
    let rwlock = RwLock::new(8);
    assert_eq!(*rwlock.read(), 8);
    *rwlock.write() = 7;
    assert_eq!(*rwlock.read(), 7);
}

#[cfg(test)]
mod tests {
    use super::{RwLock, RwLockUpgradableReadGuard};

    #[cfg(loom)]
    use loom::thread;
    #[cfg(not(loom))]
    use std::thread;

    use std::{sync::Arc, vec::Vec};

    // test multiple reads
    fn multiread_kernel() {
        let rwlock = Arc::new(RwLock::new(7));
        let mut joiners = Vec::new();
        for _ in 0..1 {
            let rclone = rwlock.clone();
            joiners.push(thread::spawn(move || {
                let lock = rclone.read();
                assert_eq!(*lock, 7);
            }));
        }

        joiners.into_iter().for_each(|j| j.join().unwrap());
    }

    #[cfg(loom)]
    #[test]
    fn multiread() {
        loom::model(|| multiread_kernel());
    }

    #[cfg(not(loom))]
    #[test]
    fn multiread() {
        multiread_kernel();
    }

    // test multiple writes
    fn multiwrite_kernel() {
        let rwlock = Arc::new(RwLock::new(0));
        let mut joiners = Vec::new();
        for _ in 0..2 {
            let rclone = rwlock.clone();
            joiners.push(thread::spawn(move || {
                let mut lock = rclone.write();
                *lock += 1;
            }));
        }

        joiners.into_iter().for_each(|j| j.join().unwrap());
        assert_eq!(*rwlock.read(), 2);
    }

    #[cfg(loom)]
    #[test]
    fn multiwrite() {
        loom::model(|| multiwrite_kernel());
    }

    #[cfg(not(loom))]
    #[test]
    fn multiwrite() {
        multiwrite_kernel();
    }

    // test upgrading
    fn upgrade_kernel() {
        let rwlock = Arc::new(RwLock::new((false, 0)));
        let mut joiners = Vec::new();
        for i in 0..2 {
            let rclone = rwlock.clone();
            joiners.push(thread::spawn(move || {
                let lock = RwLock::upgradable_read(&rclone);

                // even numbers just read the lock, determine the first element is false, then return
                if i & 1 == 0 {
                    assert_eq!(lock.0, false);
                } else {
                    // odd numbers increment the number
                    let mut lock = RwLockUpgradableReadGuard::upgrade(lock);
                    lock.1 += 1;
                }
            }));
        }

        joiners.into_iter().for_each(|j| j.join().unwrap());
        assert_eq!(rwlock.read().1, 1);
    }

    #[cfg(loom)]
    #[test]
    fn upgrade() {
        loom::model(|| upgrade_kernel());
    }

    #[cfg(not(loom))]
    #[test]
    fn upgrade() {
        upgrade_kernel();
    }
}
