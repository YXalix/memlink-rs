use obmm_rs::*;

#[test]
fn test_obmm_export_unexport() {
    let mut lengths = vec![0; MAX_NUMA_NODES];
    lengths[1] = 1024 * 1024 * 128; // 128MB on NUMA node 1
    let flags = ObmmExportFlags::ALLOWMMAP;
    match mem_export::<UbPrivData>(&lengths, flags) {
        Ok((memid, desc)) => {
            println!("Exported MemID: {}", memid);
            println!("Memory Descriptor: {:?}", desc);
            assert!(memid != OBMM_INVALID_MEMID);
            assert!(desc.length == 1024 * 1024 * 128);

            // Now unexport the memory
            match mem_unexport(memid, ObmmUnexportFlags::FORCE) {
                Ok(_) => {
                    println!("Successfully unexported MemID: {}", memid);
                }
                Err(code) => {
                    panic!("mem_unexport failed with code {}", code);
                }
            }
        }
        Err(code) => {
            panic!("mem_export failed with code {}", code);
        }
    }
}
