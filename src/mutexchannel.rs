pub struct MutexChannel<T> {
    queue: Mutex<VecDeque<T>>,
    item_ready: Condvar,
}

impl<T> MutexChannel<T> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            item_ready: Condvar::new()
        }
    }

    // when a message is sent, it's sent to the back of the queue and alerts a receiving thread that a message can be popped
    // this wakes the thread up and allows it to receive a message
    pub fn send(&self, message: T) {
        self.queue.lock().unwrap().push_back(message);
        self.item_ready.notify_one();
    }

    pub fn receive(&self) -> T {
        let mut b = self.queue.lock().unwrap();

        loop {
        // if there's a message that can be returned from the front of the VecDeque queue, return it
            if let Some(message) = b.pop_front() {
                return message;
            }
        // wait until this thread receives a notification to loop again - the mutex is unlocked while waiting
        // this means that the mutex can be used between several threads
            b = self.item_ready.wait(b).unwrap();
        }
    }
}