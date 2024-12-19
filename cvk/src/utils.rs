use crate::Adapter;
use ash::vk;

pub fn print_queues_pretty(adapter: &Adapter) {
    println!("Queue Families:");

    for (index, queue_family) in adapter.queue_properties.iter().enumerate() {
        println!("\tQueue Family {}:", index);
        println!("\t\tQueue Count: {}", queue_family.queue_count);

        // Print queue flags
        let flags = queue_family.queue_flags;
        println!("\t\tQueue Flags:");
        if flags.contains(vk::QueueFlags::GRAPHICS) {
            println!("\t\t\tGRAPHICS");
        }
        if flags.contains(vk::QueueFlags::COMPUTE) {
            println!("\t\t\tCOMPUTE");
        }
        if flags.contains(vk::QueueFlags::TRANSFER) {
            println!("\t\t\tTRANSFER");
        }
        if flags.contains(vk::QueueFlags::SPARSE_BINDING) {
            println!("\t\t\tSPARSE_BINDING");
        }
        if flags.contains(vk::QueueFlags::PROTECTED) {
            println!("\t\t\tPROTECTED");
        }

        println!(
            "\t\tTimestamp Valid Bits: {}",
            queue_family.timestamp_valid_bits
        );
        println!(
            "\t\tMin Image Transfer Granularity: {:?}",
            queue_family.min_image_transfer_granularity
        );
    }
}
