use std::{thread, ops::DerefMut};
use core::sync::atomic::{AtomicBool, Ordering::{Acquire, Release}};
use core::cell::UnsafeCell;
use std::ops::Deref;

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    // Value in spinlock is accessed here. The data is locked until it's unlocked
    pub fn lock<'a>(&'a self) -> Guard<T> {
        while self.locked.swap(true, Acquire) {
            std::hint::spin_loop();
        }
        Guard {lock: self}
    }
}

// This has to be called because otherwise, we cannot 
unsafe impl<T> Sync for SpinLock<T> where T: Send {}

pub struct Guard<'a, T> {
    lock: &'a SpinLock<T>
}


impl<T> Deref for Guard<'_, T> {
    type Target = T;
    // Safety: The very existence of this guard means we've exclusively locked the lock,
    // essentially meaning that the spinlock is safe to use
    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get()}
    }
}

impl<T> DerefMut for Guard<'_, T> {
    // Safety: The very existence of this guard means we've exclusively locked the lock,
    // essentially meaning that the spinlock is safe to use
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get()}
    }
}

// Drop automatically gets rid of the value once it's out of scope - this doesn't need to be called explicitly
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Release);
    }
}

pub fn simulate_spinlock() {
    // create a new Spinlock with a vec inside of the spinlock
    let x = SpinLock::new(Vec::new());
    thread::scope(|s| {
        // create a new thread that will lock the spinlock to that thread
        // after done, the spinlock is free so it can be locked again in another thread
        s.spawn(|| x.lock().push(1));
        s.spawn(|| {
            let mut g = x.lock();
            g.push(2);
            g.push(2);
        });
    });
    let g = x.lock();
    assert!(g.as_slice() == [1, 2, 2] || g.as_slice() == [2, 2, 1]);
}