## Rust Atomics & Locks Learning
This repo is for following my exploration of the book, Rust Atomics & Locks. This book is about concurrency in Rust, and features many indepth explanations of concurrency, how memory handling works in Rust, and how it can be used to effectively handle concurrency.

## Contents of this repo
- A minimal implementation of a spinlock (an element that can hold a value and be locked up in a thread which allows the value to be accessed - generally, not useful in Rust but implemented for the sake of learning what it is)
- A minimal implementation of a simple Mutex channel (a channel that uses a VecDeque which is mutually exclusive and uses a conditional variable to check when to notify the thread, waking it up and progressing the task)
- A minimal implementation of a Oneshot channel (a channel that uses a Sender and Receiver for messages, which abstracts a lot of code away from the main function at the cost of some flexibility due to borrowing)

