use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering::{Relaxed, Release, Acquire}};
use std::thread;
use std::thread::Thread;


// message - holds some data we may want to use
// ready - lets us know whether or not it is ready
pub struct OneshotChannel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
    in_use: AtomicBool
}

// this impl tells the compiler our new channel is (reasonably) safe to share as long as T is Send
unsafe impl<T> Sync for OneshotChannel<T> where T: Send {}

impl<T> OneshotChannel<T> {
    pub const fn new() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false),
            in_use: AtomicBool::new(false),
        }
    }

    // Get method here gets a pointer to the MaybeUninit<T>, and unsafely dereferences it
    // Safety: Only call this once, otherwise it breaks as it's dereferenced and will therefore leak memory
    // if called more than once
    pub fn send(&self, message: T) {
        // if self.in_use can be swapped with this value, panic and send a message
        if self.in_use.swap(true, Relaxed) {
            panic!("Can't send more than one message!");
        }
        unsafe {(*self.message.get()).write(message)};
        self.ready.store(true, Release);
    }

    // if Receive doesn't check the status of self.ready.load, this would be in Acquire memory ordering
    // however because this fn is now for indicative purposes, we can keep it as Relaxed as there is
    pub fn is_ready(&self) -> bool {
        self.ready.load(Relaxed)
    }


    pub fn receive(&self) -> T {
        // if is_ready wasn't called, panic and produce a message - this makes it safe to use
        if !self.ready.swap(false, Acquire) {
            panic!("No message available!");
        }
        unsafe { (*self.message.get()).assume_init_read() }
    }
}

impl<T> Drop for OneshotChannel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            unsafe { self.message.get_mut().assume_init_drop()}
        }
    }
}

pub fn simulate_oneshot_channel() {
    let channel = OneshotChannel::new();
    let t = thread::current();
    thread::scope(|s| {
        s.spawn(|| {
            channel.send("hello world!");
            t.unpark();
        });
        while !channel.is_ready() {
            thread::park();
        }
        assert_eq!(channel.receive(), "hello world!");
    })
}

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
    receiving_thread: Thread,
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
// PhantomData allows zero-sized to "act like" they own a <generic type>.
// This is useful for implementing things like thread blocking, which we're doing here
    _no_send: PhantomData<*const ()>
}

struct Channel<T> { // no longer `pub`
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false)
        }
    }

    pub fn split<'a>(&mut self) -> (Sender<T>, Receiver<T>) {
        // By overwriting *self with a new empty channel (where Self is a Channel<T>), we make sure it's in the 
        // expected state before we return the sender and receiver
        *self = Self::new();
        (
            Sender {
                channel: self,
                receiving_thread: thread::current()
            },
            Receiver {
                channel: self,
                _no_send: PhantomData
            }
        )
    }
}

impl<T> Sender<'_, T> {
    pub fn send(self, message: T) {
        unsafe { (*self.channel.message.get()).write(message)};
        self.channel.ready.store(true, Release);
        self.receiving_thread.unpark();
    }
}

impl<T> Receiver<'_, T> {
    pub fn receive(&self) -> T { 
        while !self.channel.ready.swap(false, Acquire) {
            thread::park();
        }
        unsafe { (*self.channel.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            unsafe {
                self.message.get_mut().assume_init_drop()
            }
        }
    }
}

pub fn simulate_oneshot_channel_with_sender_and_receiver() {
    let mut channel = Channel::new();
    thread::scope(|s| {
        let (sender, receiver) = channel.split();
        s.spawn(move || {
            sender.send("hello world!");
        });
        assert_eq!(receiver.receive(), "hello world!");
    })
}