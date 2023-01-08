mod spinlock;
use spinlock::simulate_spinlock;

mod oneshotchannel;
use oneshotchannel::simulate_oneshot_channel;

use crate::oneshotchannel::simulate_oneshot_channel_with_sender_and_receiver;

fn main() {    
    simulate_spinlock();
    simulate_oneshot_channel();
    simulate_oneshot_channel_with_sender_and_receiver();
    println!("Hello world");
}

