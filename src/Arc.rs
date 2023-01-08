use std::ptr::NonNull;
use std::mem::ManuallyDrop;

struct ArcData<T> {
    // Number of Arcs
    data_ref_count: AtomicUsize,
    // Number of Arcs and Weaks combined
    alloc_ref_count: AtomicUsize,
    // The data. Should be "none" if there's only weak pointers left
    data: UnsafeCell<ManuallyDrop<T>>,
}

pub struct Arc<T> {
    weak: NonNull<ArcData<T>>
}

unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}

pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>
}

unsafe impl<T: Sync + Send> Send for Weak<T> {}
unsafe impl<T: Sync + Send> Sync for Weak<T> {}

impl<T> Arc<T> {
    // to be able to create a new Arc, we have to create a new allocation with an ArcData<T> with a ref count of 1.
    // Box is used to create a new heap allocation, then it's leaked to give up exclusive ownership and NonNull::from
    // is used to turn it into a pointer that can be referenced
    pub fn new(data: T) -> Arc<T> {
        Arc {
            weak: Weak {
                ptr: NonNull::from(Box::leak(Box::new(ArcData {
                    alloc_ref_count: AtomicUsize::new(1),
                    data_ref_count: AtomicUsize::new(1),
                    data: UnsafeCell::new(ManuallyDrop::new(data))
                })))
            }
        }
    }

    // As long as Arc exists, the pointer will always ref a valid ArcData<T>
    // However, the compiler can't know this so we have to wrap this in an unsafe 
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref()}
    }

    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        // Acquire matches Weak::drop's Release decrement, to make sure any
        // upgraded pointers are visible in the next data_ref_count.load.
        if arc.data().alloc_ref_count.compare_exchange(
            1, usize::MAX, Acquire, Relaxed
        ).is_err() {
            return None;
        }
        let is_unique = arc.data().data_ref_count.load(Relaxed) == 1;
        // Release matches Acquire increment in `downgrade`, to make sure any
        // changes to the data_ref_count that come after `downgrade` don't
        // change the is_unique result above.
        arc.data().alloc_ref_count.store(1, Release);
        if !is_unique {
            return None;
        }
        // Acquire to match Arc::drop's Release decrement, to make sure nothing
        // else is accessing the data.
        fence(Acquire);
        unsafe { Some(&mut *arc.data().data.get()) }
    }

    pub fn downgrade(arc: &Self) -> Weak<T> {
        let mut n = arc.data().alloc_ref_count.load(Relaxed);
        loop {
            if n == usize::MAX {
                std::hint::spin_loop();
                n = arc.data().alloc_ref_count.load(Relaxed);
                continue;
            }
            assert!(n < usize::MAX - 1);
            // Acquire synchronises with get_mut's release-store.
            if let Err(e) =
                arc.data()
                    .alloc_ref_count
                    .compare_exchange_weak(n, n + 1, Acquire, Relaxed)
            {
                n = e;
                continue;
            }
            return Weak { ptr: arc.ptr };
        }
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe {self.ptr.as_ref()}
    }

    fn upgrade(&self) -> Option<Arc<T>> {
        let mut n = self.data().data_ref_count.load(Relaxed);
        // If there's no arcs, return Nothing
        loop {
            if n == 0 {
                return None;
            }
            assert!(n < Usize::MAX);
            // if there's an error with trying to store the value (ie internal error), return an error
            // Setting n to e means that n == 0 will automatically trip
            if let Err(e) = self.data().data_ref_count.compare_exchange_weak(n, n+1, Relaxed, Relaxed) {
                n = e;
                continue
            }
            return Some(Arc { weak: self.clone()})
        }
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    // deref allows Arc<T> to transparently behave as reference to T
    // Because Arc<T> represents shared ownership, DerefMut cannot be implemented
    fn deref(&self) -> &T {
        // Since there's an Arc to the data, it exists and can therefore be shared safely
        unsafe { (*ptr).as_ref().unwrap()}
    }
}


impl<T> Clone for Arc<T> {
    fn clone (&self) -> Self {
        if self.data().ref_count.fetch_add(1, Relaxed) > usize::MAX / 2 {
            std::process::abort()
        }
        Arc {
            ptr: self_ptr,
        }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        let weak = self.weak.clone();
        // If the reference counter is 0, abort
        if weak.data().data_ref_count.fetch_add(1, Release) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc {weak}
    }
}

impl Drop for Weak<T> {
    fn drop(&mut self) {
        // Decrement the Arc counter and de-allocate the ArcData when the counter hits 0
        if self.data().alloc_ref_count.fetch_sub(1, Release) == 1 {
            fence(Acquire);
            unsafe {
                // This converts the raw heap allocation to a box, then immediately drops the box.
                drop(Box::from_raw(self.ptr.as_ptr()))
            }
        }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        // If an Arc is dropped, drop a Weak as well as every Arc contains a Weak
        if self.data().ref_count.fetch_sub(1, Release) == 1 {
            fence(Acquire);
            let ptr = self.weak.data().data.get();
            // The reference counter is 0, so nothing is going to access the data and it's therefore safe
            unsafe {
                ManuallyDrop::drop(&mut *self.data().data.get());
            }
            drop(Weak {ptr: self.ptr});
        }
    }
}

#[test]
fn test() {
    static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);

    struct DetectDrop;

    impl Drop for DetectDrop {
        fn drop(&mut self) {
            NUM_DROPS.fetch_add(1, Relaxed);
        }
    }

    let x = Arc::new(("hello", DetectDrop));
    let y = Arc::downgrade(&x);
    let z = Arc::downgrade(&x);

    let t = std::thread::spawn(move || {
        let y = y.upgrade().unwrap();
        assert_eq!(y.0, "hello");
    });

    assert_eq!(x.0, "hello");

    // Wait for thread to finish
    t.join().unwrap();

    assert_eq!(NUM_DROPS.load(Relaxed), 0);
    assert!(z.upgrade().is_some());

    drop(x);

    assert_eq!(NUM_DROPS.load(Relaxed), 1);
    assert!(z.upgrade().is_none());
}

