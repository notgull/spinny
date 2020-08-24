// MIT/Apache2 License

//! Implementation of a basic spin-based RwLock

#![no_std]
#![warn(clippy::pedantic)]

use core::sync::atomic::{spin_loop_hint, AtomicUsize, Ordering};
use lock_api::{GuardSend, RawRwLock, RawRwLockDowngrade, RawRwLockUpgrade, RwLock as LARwLock, RwLockReadGuard as LARwLockReadGuard, RwLockWriteGuard as LARwLockWriteGuard, RwLockUpgradableReadGuard as LARwLockUpgradableReadGuard};

/// Raw spinlock rwlock, wrapped in the lock_api RwLock struct.
pub struct RawRwSpinlock(AtomicUsize);

// flags stored in the usize struct
const READER: usize = 1 << 2;
const UPGRADED: usize = 1 << 1;
const WRITER: usize = 1 << 0;

unsafe impl RawRwLock for RawRwSpinlock {
    const INIT: RawRwSpinlock = RawRwSpinlock(AtomicUsize::new(0));

    type GuardMarker = GuardSend;

    fn lock_shared(&self) {
        while !self.try_lock_shared() {
            spin_loop_hint()
        }
    }

    fn try_lock_shared(&self) -> bool {
        let value = self.0.fetch_add(READER, Ordering::Acquire);

        if value & (WRITER | UPGRADED) != 0 {
            self.0.fetch_sub(READER, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    fn try_lock_exclusive(&self) -> bool {
        self.0
            .compare_exchange(0, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    fn lock_exclusive(&self) {
        loop {
            match self
                .0
                .compare_exchange_weak(0, WRITER, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return,
                Err(_) => spin_loop_hint(),
            }
        }
    }

    unsafe fn unlock_shared(&self) {
        self.0.fetch_sub(READER, Ordering::Release);
    }

    unsafe fn unlock_exclusive(&self) {
        self.0.fetch_and(!(WRITER | UPGRADED), Ordering::Release);
    }
}

unsafe impl RawRwLockUpgrade for RawRwSpinlock {
    fn lock_upgradable(&self) {
        while !self.try_lock_upgradable() {
            spin_loop_hint()
        }
    }

    fn try_lock_upgradable(&self) -> bool {
        self.0.fetch_or(UPGRADED, Ordering::Acquire) & (WRITER | UPGRADED) == 0
    }

    unsafe fn try_upgrade(&self) -> bool {
        self.0
            .compare_exchange(UPGRADED, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn upgrade(&self) {
        loop {
            match self.0.compare_exchange_weak(
                UPGRADED,
                WRITER,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(_) => spin_loop_hint(),
            }
        }
    }

    unsafe fn unlock_upgradable(&self) {
        self.0.fetch_sub(UPGRADED, Ordering::AcqRel);
    }
}

unsafe impl RawRwLockDowngrade for RawRwSpinlock {
    unsafe fn downgrade(&self) {
        self.0.fetch_add(READER, Ordering::Acquire);
        self.unlock_exclusive();
    }
}

/// A read-write lock that uses a spinlock internally.
pub type RwLock<T> = LARwLock<T, RawRwSpinlock>;
/// A read guard for the read-write lock.
pub type RwLockReadGuard<'a, T> = LARwLockReadGuard<'a, T>;
/// A write guard fo the read-write lock.
pub type RwLockWriteGuard<'a, T> = LARwLockWriteGuard<'a, T>;
/// An upgradable read guard for the read-write lock.
pub type RwLockUpgradableReadGuard<'a, T> = LARwLockUpgradableReadGuard<'a, T>; 
